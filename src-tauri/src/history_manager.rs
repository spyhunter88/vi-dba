use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QueryHistoryEntry {
    pub id: String,
    pub query: String,
    pub timestamp: String,
    pub connection_id: String,
    pub duration_ms: u128,
    pub status: String, // "success" or "error"
    pub affected_rows: u64,
    pub script_id: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TabState {
    pub id: String,
    pub title: String,
    pub tab_type: String, // TabType as string
    pub connection_id: String,
    pub content: Option<String>,
    pub file_path: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub cursor_position: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SessionState {
    pub tabs: Vec<TabState>,
    pub active_tab_id: Option<String>,
}

pub fn get_history_dir(base_path: &Path) -> Result<PathBuf, String> {
    let app_dir = base_path.join("history");

    if !app_dir.exists() {
        fs::create_dir_all(&app_dir).map_err(|e| e.to_string())?;
    }

    Ok(app_dir)
}

pub fn get_sessions_dir(base_path: &Path) -> Result<PathBuf, String> {
    let app_dir = base_path.join("sessions");

    if !app_dir.exists() {
        fs::create_dir_all(&app_dir).map_err(|e| e.to_string())?;
    }

    Ok(app_dir)
}

pub fn get_snapshots_dir(base_path: &Path) -> Result<PathBuf, String> {
    let dir = base_path.join("snapshots");
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    Ok(dir)
}

fn get_specific_snapshot_dir(
    base_path: &Path,
    connection_id: &str,
    database: Option<&str>,
    schema: Option<&str>,
    tab_id: &str,
) -> Result<PathBuf, String> {
    let mut dir = get_snapshots_dir(base_path)?;
    dir = dir.join(connection_id);
    if let Some(db) = database {
        if !db.is_empty() {
            dir = dir.join(db);
            if let Some(sch) = schema {
                if !sch.is_empty() {
                    dir = dir.join(sch);
                }
            }
        }
    }
    dir = dir.join(tab_id);

    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    }

    Ok(dir)
}

#[tauri::command]
pub async fn save_session(app_handle: AppHandle, state: SessionState) -> Result<(), String> {
    let settings_manager = app_handle.state::<crate::settings_manager::SettingsManager>();
    let base_path = settings_manager.get_app_data_path();
    let sessions_dir = get_sessions_dir(&base_path)?;
    let session_file = sessions_dir.join("auto-session.json");

    let json = serde_json::to_string_pretty(&state).map_err(|e| e.to_string())?;
    fs::write(session_file, json).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn load_session(app_handle: AppHandle) -> Result<Option<SessionState>, String> {
    let settings_manager = app_handle.state::<crate::settings_manager::SettingsManager>();
    let base_path = settings_manager.get_app_data_path();
    let sessions_dir = get_sessions_dir(&base_path)?;
    let session_file = sessions_dir.join("auto-session.json");

    if !session_file.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(session_file).map_err(|e| e.to_string())?;
    let state: SessionState = serde_json::from_str(&content).map_err(|e| e.to_string())?;

    Ok(Some(state))
}

#[tauri::command]
pub async fn add_query_history(
    app_handle: AppHandle,
    entry: QueryHistoryEntry,
) -> Result<(), String> {
    let settings_manager = app_handle.state::<crate::settings_manager::SettingsManager>();
    let settings = settings_manager.load_settings();
    let base_path = settings_manager.get_app_data_path();
    let history_dir = get_history_dir(&base_path)?;
    let history_file = history_dir.join("query-history.json");

    let mut history: Vec<QueryHistoryEntry> = if history_file.exists() {
        let content = fs::read_to_string(&history_file).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).unwrap_or_else(|_| Vec::new())
    } else {
        Vec::new()
    };

    history.push(entry.clone());

    // 1. Time-based retention
    if settings.enable_history_retention_lifetime.unwrap_or(false) {
        let days = settings.history_max_lifetime_days.unwrap_or(0) as i64;
        let hours = settings.history_max_lifetime_hours.unwrap_or(0) as i64;
        let minutes = settings.history_max_lifetime_minutes.unwrap_or(0) as i64;

        if days > 0 || hours > 0 || minutes > 0 {
            let total_minutes = days * 24 * 60 + hours * 60 + minutes;
            let cutoff = chrono::Local::now() - chrono::Duration::minutes(total_minutes);

            history.retain(|item| {
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&item.timestamp) {
                    return dt.with_timezone(&chrono::Local) >= cutoff;
                }
                true
            });
        }
    }

    // 2. Per-connection retention
    if settings
        .enable_history_retention_per_connection
        .unwrap_or(false)
    {
        if let Some(limit) = settings.history_max_per_connection {
            let mut conn_counts = std::collections::HashMap::new();
            // Iterate backwards to keep newest
            let mut new_history = Vec::new();
            for item in history.into_iter().rev() {
                let count = conn_counts.entry(item.connection_id.clone()).or_insert(0);
                if *count < limit {
                    new_history.push(item);
                    *count += 1;
                }
            }
            new_history.reverse();
            history = new_history;
        }
    }

    // 3. Global total retention
    let max_total = if settings.enable_history_retention_total.unwrap_or(false) {
        settings.history_max_total.unwrap_or(1000) as usize
    } else {
        1000 // Default fallback
    };

    if history.len() > max_total {
        let start = history.len() - max_total;
        history.drain(0..start);
    }

    let json = serde_json::to_string_pretty(&history).map_err(|e| e.to_string())?;
    fs::write(history_file, json).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn get_query_history(app_handle: AppHandle) -> Result<Vec<QueryHistoryEntry>, String> {
    let settings_manager = app_handle.state::<crate::settings_manager::SettingsManager>();
    let base_path = settings_manager.get_app_data_path();
    let history_dir = get_history_dir(&base_path)?;
    let history_file = history_dir.join("query-history.json");

    if !history_file.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(history_file).map_err(|e| e.to_string())?;
    let history: Vec<QueryHistoryEntry> =
        serde_json::from_str(&content).map_err(|e| e.to_string())?;

    Ok(history)
}

#[tauri::command]
pub async fn clear_query_history(app_handle: AppHandle) -> Result<(), String> {
    let settings_manager = app_handle.state::<crate::settings_manager::SettingsManager>();
    let base_path = settings_manager.get_app_data_path();
    let history_dir = get_history_dir(&base_path)?;
    let history_file = history_dir.join("query-history.json");

    if history_file.exists() {
        fs::remove_file(history_file).map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub async fn save_snapshot(
    app_handle: AppHandle,
    tab_id: String,
    connection_id: String,
    database: Option<String>,
    schema: Option<String>,
    content: String,
    limit: Option<usize>,
    limit_days: Option<u64>,
) -> Result<(), String> {
    let settings_manager = app_handle.state::<crate::settings_manager::SettingsManager>();
    let base_path = settings_manager.get_app_data_path();
    let snapshots_dir = get_specific_snapshot_dir(
        &base_path,
        &connection_id,
        database.as_deref(),
        schema.as_deref(),
        &tab_id,
    )?;

    if !snapshots_dir.exists() {
        fs::create_dir_all(&snapshots_dir).map_err(|e| e.to_string())?;
    }

    // Check last snapshot to avoid duplicates
    let mut entries: Vec<_> = fs::read_dir(&snapshots_dir)
        .map_err(|e| e.to_string())?
        .filter_map(|res| res.ok())
        .collect();

    entries.sort_by_key(|e| e.path());

    if let Some(last_entry) = entries.last() {
        if let Ok(last_content) = fs::read_to_string(last_entry.path()) {
            if last_content == content {
                // Same content, no need to create new snapshot
                return Ok(());
            }
        }
    }

    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
    let snapshot_file = snapshots_dir.join(format!("{}.sql", timestamp));

    fs::write(snapshot_file, content).map_err(|e| e.to_string())?;

    // Refresh entries after write
    let mut entries: Vec<_> = fs::read_dir(&snapshots_dir)
        .map_err(|e| e.to_string())?
        .filter_map(|res| res.ok())
        .collect();

    // 1. Time-based retention (if enabled)
    if let Some(days) = limit_days {
        // let cutoff = chrono::Local::now() - chrono::Duration::days(days as i64);
        // Clean up entries older than cutoff
        // We need to re-read directory or just filter 'entries' if we assume filenames match timestamps
        // Filename format: YYYYMMDD_HHMMSS

        // Let's filter in-place and delete files
        entries.retain(|entry| {
            if let Some(filename) = entry.file_name().to_str() {
                // extract timestamp part (remove extension)
                let stem = Path::new(filename)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("");
                // Parse
                if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(stem, "%Y%m%d_%H%M%S") {
                    // NaiveDateTime doesn't have timezone, but filename was created with Local::now().
                    // Comparing with naive local is okay.
                    let now_naive = chrono::Local::now().naive_local();
                    let age = now_naive - dt;
                    if age.num_days() > days as i64 {
                        fs::remove_file(entry.path()).ok();
                        return false; // Remove from list
                    }
                }
            }
            true // Keep in list
        });
    }

    // 2. Count-based retention (if enabled)
    // If limit is None, it means "unlimited count" (assuming user unchecked it)
    if let Some(retention_limit) = limit {
        if entries.len() > retention_limit {
            entries.sort_by_key(|e| e.path());
            // Remove oldest entries to fit within limit
            let num_to_remove = entries.len().saturating_sub(retention_limit);
            for entry in entries.iter().take(num_to_remove) {
                fs::remove_file(entry.path()).ok();
            }
        }
    }

    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SnapshotInfo {
    pub timestamp: String,
    pub path: String,
}

#[tauri::command]
pub async fn get_snapshots(
    app_handle: AppHandle,
    tab_id: String,
    connection_id: String,
    database: Option<String>,
    schema: Option<String>,
) -> Result<Vec<SnapshotInfo>, String> {
    let settings_manager = app_handle.state::<crate::settings_manager::SettingsManager>();
    let base_path = settings_manager.get_app_data_path();
    let snapshots_dir = get_specific_snapshot_dir(
        &base_path,
        &connection_id,
        database.as_deref(),
        schema.as_deref(),
        &tab_id,
    )?;

    if !snapshots_dir.exists() {
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(snapshots_dir).map_err(|e| e.to_string())?;
    let mut snapshots = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_file() {
            let filename = path.file_stem().unwrap().to_str().unwrap().to_string();
            snapshots.push(SnapshotInfo {
                timestamp: filename,
                path: path.to_str().unwrap().to_string(),
            });
        }
    }

    snapshots.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    Ok(snapshots)
}

#[tauri::command]
pub async fn read_snapshot(path: String) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| e.to_string())
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotSummary {
    pub tab_id: String,
    pub connection_id: String,
    pub database: Option<String>,
    pub schema: Option<String>,
    pub snapshot_count: usize,
    pub last_snapshot: Option<String>,
}

#[tauri::command]
pub async fn get_all_snapshots_summary(
    app_handle: AppHandle,
) -> Result<Vec<SnapshotSummary>, String> {
    let settings_manager = app_handle.state::<crate::settings_manager::SettingsManager>();
    let base_path = settings_manager.get_app_data_path();
    let snapshots_base_dir = base_path.join("snapshots");

    if !snapshots_base_dir.exists() {
        return Ok(Vec::new());
    }

    let mut summaries = Vec::new();

    // Recursive scan
    // snapshots/
    //   connection_id/
    //     database/ (optional)
    //       schema/ (optional)
    //         tab_id/
    //           timestamp.sql

    fn scan_snapshots(
        summaries: &mut Vec<SnapshotSummary>,
        current_dir: &Path,
        base_dir: &Path,
    ) -> Result<(), String> {
        if !current_dir.exists() {
            return Ok(());
        }

        let entries = fs::read_dir(current_dir).map_err(|e| e.to_string())?;
        let mut has_sql_files = false;
        let mut sql_files = Vec::new();

        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.is_dir() {
                scan_snapshots(summaries, &path, base_dir)?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("sql") {
                has_sql_files = true;
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    sql_files.push(stem.to_string());
                }
            }
        }

        if has_sql_files {
            // This is a tab_id directory
            let rel_path = current_dir
                .strip_prefix(base_dir)
                .map_err(|e| e.to_string())?;
            let components: Vec<_> = rel_path.components().collect();

            if components.len() >= 1 {
                let connection_id = components[0].as_os_str().to_str().unwrap_or("").to_string();
                let mut database = None;
                let mut schema = None;
                let mut tab_id = String::new();

                match components.len() {
                    2 => {
                        // connection/tab_id
                        tab_id = components[1].as_os_str().to_str().unwrap_or("").to_string();
                    }
                    3 => {
                        // connection/database/tab_id
                        database =
                            Some(components[1].as_os_str().to_str().unwrap_or("").to_string());
                        tab_id = components[2].as_os_str().to_str().unwrap_or("").to_string();
                    }
                    4 => {
                        // connection/database/schema/tab_id
                        database =
                            Some(components[1].as_os_str().to_str().unwrap_or("").to_string());
                        schema = Some(components[2].as_os_str().to_str().unwrap_or("").to_string());
                        tab_id = components[3].as_os_str().to_str().unwrap_or("").to_string();
                    }
                    _ => {
                        // Too deep or too shallow, but let's take the last one as tab_id
                        tab_id = components
                            .last()
                            .unwrap()
                            .as_os_str()
                            .to_str()
                            .unwrap_or("")
                            .to_string();
                    }
                }

                sql_files.sort();
                summaries.push(SnapshotSummary {
                    tab_id,
                    connection_id,
                    database,
                    schema,
                    snapshot_count: sql_files.len(),
                    last_snapshot: sql_files.last().cloned(),
                });
            }
        }

        Ok(())
    }

    scan_snapshots(&mut summaries, &snapshots_base_dir, &snapshots_base_dir)?;

    // Sort by last snapshot time descending
    summaries.sort_by(|a, b| b.last_snapshot.cmp(&a.last_snapshot));

    Ok(summaries)
}

#[tauri::command]
pub async fn get_query_history_for_tab(
    app_handle: AppHandle,
    tab_id: String,
) -> Result<Vec<QueryHistoryEntry>, String> {
    let history = get_query_history(app_handle).await?;
    Ok(history
        .into_iter()
        .filter(|e| e.script_id.as_deref() == Some(&tab_id))
        .collect())
}
