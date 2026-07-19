use crate::config_manager::ConfigManager;
use crate::db_manager::DbManager;
use crate::models::{
    AppSettings, ConnectionConfig, DbObject, ExportConfig, FilePreview, ImportConfig,
    MultiSheetImportConfig, QueryResult, RoutineDefinition, RowActionResult, TableDefinition,
    TableIndexInfo, ViewDefinition,
};
use crate::script_manager::ScriptManager;
use crate::settings_manager::SettingsManager;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectResult {
    pub server_version: Option<String>,
}

/// Connects to a database using the provided configuration.
/// Used by the "Add Connection" and "Connect" buttons in the sidebar.
#[tauri::command]
pub async fn connect(
    config: ConnectionConfig,
    db_manager: State<'_, DbManager>,
) -> Result<ConnectResult, String> {
    let server_version = db_manager.connect(config).await?;
    Ok(ConnectResult {
        server_version: Some(server_version),
    })
}

/// Disconnects from an active database connection.
/// Triggered when a connection is manually closed or when the app is shutting down.
#[tauri::command]
pub async fn disconnect(id: String, db_manager: State<'_, DbManager>) -> Result<(), String> {
    db_manager.disconnect(&id).await;
    Ok(())
}

/// Executes a raw SQL query on the specified connection.
/// Used by the SQL Editor to run user-provided queries.
/// Optionally accepts an `exec_id` so the query can be cancelled mid-flight via `cancel_query`.
#[tauri::command]
pub async fn execute_query(
    id: String,
    query: String,
    table_name: Option<String>,
    database: Option<String>,
    schema: Option<String>,
    exec_id: Option<String>,
    db_manager: State<'_, DbManager>,
) -> Result<QueryResult, String> {
    db_manager
        .execute_query(&id, &query, table_name, database, schema, exec_id)
        .await
}

/// Result of executing a multi-statement script: one `QueryResult` per statement, plus the
/// statement texts (in execution order) so the UI can label each result.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptResult {
    pub results: Vec<QueryResult>,
    pub statements: Vec<String>,
}

/// Executes a SQL script (one or more statements) on the specified connection.
///
/// Unlike calling `execute_query` per statement, the whole script runs on a single
/// connection, so session-scoped state — transactions (`BEGIN`/`COMMIT`), temporary tables,
/// session variables — is preserved across statements. Statements are split server-side with
/// a SQL-aware splitter (respecting quotes, comments and dollar-quoted bodies).
#[tauri::command]
pub async fn execute_script(
    id: String,
    query: String,
    table_name: Option<String>,
    database: Option<String>,
    schema: Option<String>,
    exec_id: Option<String>,
    db_manager: State<'_, DbManager>,
) -> Result<ScriptResult, String> {
    let statements = crate::db::utils::split_sql_statements(&query);
    let results = db_manager
        .execute_script(&id, statements.clone(), table_name, database, schema, exec_id)
        .await?;
    Ok(ScriptResult {
        results,
        statements,
    })
}

/// Cancels an in-flight query started via `execute_query` with the given `exec_id`.
/// Returns true if an active query was found and aborted.
#[tauri::command]
pub async fn cancel_query(
    exec_id: String,
    db_manager: State<'_, DbManager>,
) -> Result<bool, String> {
    Ok(db_manager.cancel_query(&exec_id).await)
}

/// Updates a specific cell value in a table.
/// Used by the TableView's inline editing feature.
#[tauri::command]
pub async fn update_row(
    id: String,
    table_name: String,
    pks: std::collections::HashMap<String, serde_json::Value>,
    column: String,
    value: serde_json::Value,
    catalog: Option<String>,
    schema: Option<String>,
    db_manager: State<'_, DbManager>,
) -> Result<RowActionResult, String> {
    db_manager
        .update_row(&id, &table_name, pks, &column, value, catalog, schema)
        .await
}

/// Inserts a new row into a table.
/// Used by the TableView's "Add Row" feature.
#[tauri::command]
pub async fn insert_row(
    id: String,
    table_name: String,
    data: std::collections::HashMap<String, serde_json::Value>,
    catalog: Option<String>,
    schema: Option<String>,
    db_manager: State<'_, DbManager>,
) -> Result<RowActionResult, String> {
    db_manager
        .insert_row(&id, &table_name, data, catalog, schema)
        .await
}

/// Retrieves all database objects (tables, views, routines, etc.) for a connection.
/// Used to populate the sidebar tree view.
#[tauri::command]
pub async fn get_objects(
    id: String,
    db_manager: State<'_, DbManager>,
    config_manager: State<'_, ConfigManager>,
    settings_manager: State<'_, SettingsManager>,
) -> Result<Vec<DbObject>, String> {
    if !db_manager.is_connected(&id).await {
        let base_path = settings_manager.get_app_data_path();
        let connections = config_manager.load_connections(&base_path)?;
        if let Some(config) = connections.into_iter().find(|c| c.id == id) {
            let _ = db_manager.connect(config).await;
        }
    }
    db_manager.get_objects(&id).await
}

/// Returns a list of all tables in the specified database/schema.
/// Used by various migration and comparison tools.
#[tauri::command]
pub async fn get_table_list(
    id: String,
    catalog: Option<String>,
    schema: Option<String>,
    db_manager: State<'_, DbManager>,
) -> Result<QueryResult, String> {
    db_manager.get_table_list(&id, catalog, schema).await
}

/// Returns a list of all routines (functions/procedures) in the specified database/schema.
/// Used to populate the routines section of the sidebar.
#[tauri::command]
pub async fn get_routine_list(
    id: String,
    catalog: Option<String>,
    schema: Option<String>,
    db_manager: State<'_, DbManager>,
) -> Result<QueryResult, String> {
    db_manager.get_routine_list(&id, catalog, schema).await
}

/// Retrieves all saved database connections from the local configuration file.
/// Used to populate the connection list in the sidebar and settings.
#[tauri::command]
pub async fn get_connections(
    config_manager: State<'_, ConfigManager>,
    settings_manager: State<'_, SettingsManager>,
) -> Result<Vec<ConnectionConfig>, String> {
    config_manager.load_connections(&settings_manager.get_app_data_path())
}

/// Saves or updates a database connection configuration.
/// Triggered when the user clicks "Save" in the connection editor.
#[tauri::command]
pub async fn save_connection(
    config: ConnectionConfig,
    config_manager: State<'_, ConfigManager>,
    settings_manager: State<'_, SettingsManager>,
) -> Result<(), String> {
    let base_path = settings_manager.get_app_data_path();
    let mut connections = config_manager.load_connections(&base_path)?;
    // Update if exists, otherwise add
    if let Some(idx) = connections.iter().position(|c| c.id == config.id) {
        connections[idx] = config;
    } else {
        connections.push(config);
    }
    config_manager.save_connections(&base_path, &connections)
}

/// Deletes a database connection configuration by its ID.
/// Triggered when the user selects "Delete" from a connection's context menu.
#[tauri::command]
pub async fn delete_connection(
    id: String,
    config_manager: State<'_, ConfigManager>,
    settings_manager: State<'_, SettingsManager>,
) -> Result<(), String> {
    let base_path = settings_manager.get_app_data_path();
    let mut connections = config_manager.load_connections(&base_path)?;
    connections.retain(|c| c.id != id);
    config_manager.save_connections(&base_path, &connections)
}

/// Tests a database connection without saving it.
/// Used by the "Test Connection" button in the connection editor.
#[tauri::command]
pub async fn test_connection(config: ConnectionConfig) -> Result<ConnectResult, String> {
    let manager = DbManager::new();
    let server_version = manager.connect(config).await?;
    Ok(ConnectResult {
        server_version: Some(server_version),
    })
}
/// Retrieves the detailed definition of a table (columns, types, constraints).
/// Used by the Table Editor to reflect the current schema.
#[tauri::command]
pub async fn get_table_definition(
    id: String,
    table_name: String,
    catalog: Option<String>,
    schema: Option<String>,
    db_manager: State<'_, DbManager>,
) -> Result<TableDefinition, String> {
    db_manager
        .get_table_definition(&id, &table_name, catalog, schema)
        .await
}

/// Returns indexes and foreign keys for a specific table.
#[tauri::command]
pub async fn get_table_indexes(
    id: String,
    table_name: String,
    catalog: Option<String>,
    schema: Option<String>,
    db_manager: State<'_, DbManager>,
) -> Result<TableIndexInfo, String> {
    db_manager
        .get_table_indexes(&id, &table_name, catalog, schema)
        .await
}

/// Creates a new table based on the provided definition.
/// Triggered when saving a "New Table" tab.
#[tauri::command]
pub async fn create_table(
    id: String,
    definition: TableDefinition,
    db_manager: State<'_, DbManager>,
) -> Result<(), String> {
    db_manager.create_table(&id, definition).await
}

/// Alters an existing table by comparing old and new definitions.
/// Triggered when saving changes in the Table Editor.
#[tauri::command]
pub async fn alter_table(
    id: String,
    old_definition: TableDefinition,
    new_definition: TableDefinition,
    db_manager: State<'_, DbManager>,
) -> Result<(), String> {
    db_manager
        .alter_table(&id, old_definition, new_definition)
        .await
}

/// Generates the SQL script required to transform a table from its old definition to a new one.
/// Used to show a preview of changes before applying them in the Table Editor.
#[tauri::command]
pub async fn generate_table_sql(
    id: String,
    old_definition: Option<TableDefinition>,
    new_definition: TableDefinition,
    db_manager: State<'_, DbManager>,
) -> Result<String, String> {
    db_manager
        .generate_table_sql(&id, old_definition, new_definition)
        .await
}

/// Retrieves the source code of a specific routine (function or procedure).
/// Used to populate the Routine Editor.
#[tauri::command]
pub async fn get_routine_definition(
    id: String,
    name: String,
    routine_type: String,
    catalog: Option<String>,
    schema: Option<String>,
    db_manager: State<'_, DbManager>,
) -> Result<RoutineDefinition, String> {
    db_manager
        .get_routine_definition(&id, &name, &routine_type, catalog, schema)
        .await
}

/// Saves or updates a routine's definition in the database.
/// Triggered when saving changes in the Routine Editor.
#[tauri::command]
pub async fn save_routine(
    id: String,
    definition: RoutineDefinition,
    db_manager: State<'_, DbManager>,
) -> Result<(), String> {
    db_manager.save_routine(&id, definition).await
}

/// Retrieves the SQL definition of a view.
/// Used to populate the View Editor.
#[tauri::command]
pub async fn get_view_definition(
    id: String,
    name: String,
    catalog: Option<String>,
    schema: Option<String>,
    db_manager: State<'_, DbManager>,
) -> Result<ViewDefinition, String> {
    db_manager
        .get_view_definition(&id, &name, catalog, schema)
        .await
}

/// Saves or updates a view's definition in the database.
/// Triggered when saving changes in the View Editor.
#[tauri::command]
pub async fn save_view(
    id: String,
    definition: ViewDefinition,
    db_manager: State<'_, DbManager>,
) -> Result<(), String> {
    db_manager.save_view(&id, definition).await
}

/// Lists all saved SQL scripts for a particular connection/database context.
/// Used by the "Scripts" view to show saved files.
#[tauri::command]
pub async fn list_scripts(
    connection_id: String,
    database: Option<String>,
    schema: Option<String>,
    script_manager: State<'_, crate::script_manager::ScriptManager>,
    settings_manager: State<'_, SettingsManager>,
) -> Result<Vec<crate::models::ScriptInfo>, String> {
    script_manager.list_scripts(
        &settings_manager.get_app_data_path(),
        &connection_id,
        database,
        schema,
    )
}

/// Saves a SQL script to the local filesystem.
/// Triggered when the user saves a script from the SQL Editor.
#[tauri::command]
pub async fn save_script(
    connection_id: String,
    name: String,
    content: String,
    database: Option<String>,
    schema: Option<String>,
    script_manager: State<'_, crate::script_manager::ScriptManager>,
    settings_manager: State<'_, SettingsManager>,
) -> Result<crate::models::ScriptInfo, String> {
    script_manager.save_script(
        &settings_manager.get_app_data_path(),
        &connection_id,
        &name,
        &content,
        database,
        schema,
    )
}

/// Reads a SQL script from the local filesystem.
/// Triggered when the user opens a script from the Scripts list.
#[tauri::command]
pub async fn read_script(
    connection_id: String,
    name: String,
    script_manager: State<'_, crate::script_manager::ScriptManager>,
    settings_manager: State<'_, SettingsManager>,
) -> Result<String, String> {
    script_manager.read_script(&settings_manager.get_app_data_path(), &connection_id, &name)
}
/// Opens the Settings window.
/// Triggered by the settings icon in the sidebar.
#[tauri::command]
pub async fn open_settings(app: tauri::AppHandle) -> Result<(), String> {
    tauri::WebviewWindowBuilder::new(
        &app,
        "settings",
        tauri::WebviewUrl::App("/settings?window=true".into()),
    )
    .title("Settings")
    .inner_size(800.0, 600.0)
    .resizable(true)
    .build()
    .map_err(|e| e.to_string())?;

    Ok(())
}

/// Opens the History window.
/// Triggered by the history icon in the sidebar.
#[tauri::command]
pub async fn open_history(app: tauri::AppHandle) -> Result<(), String> {
    tauri::WebviewWindowBuilder::new(
        &app,
        "history",
        tauri::WebviewUrl::App("/history?window=true".into()),
    )
    .title("Query History & Sessions")
    .inner_size(1100.0, 750.0)
    .resizable(true)
    .build()
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportProgress {
    total: usize,
    current: usize,
    success_count: usize,
    error_count: usize,
    is_finished: bool,
    message: String,
    logs: Vec<String>,
}

/// Opens the Import Data window.
/// Triggered from the table context menu or the sidebar.
#[tauri::command]
pub async fn open_import(
    app: tauri::AppHandle,
    connection_id: String,
    catalog: Option<String>,
    schema: Option<String>,
    table_name: Option<String>,
) -> Result<(), String> {
    let mut url = format!("/import?connectionId={}", connection_id);
    if let Some(c) = catalog {
        url.push_str(&format!("&catalog={}", urlencoding::encode(&c)));
    }
    if let Some(s) = schema {
        url.push_str(&format!("&schema={}", urlencoding::encode(&s)));
    }
    if let Some(t) = table_name {
        url.push_str(&format!("&tableName={}", urlencoding::encode(&t)));
    }

    tauri::WebviewWindowBuilder::new(&app, "import", tauri::WebviewUrl::App(url.into()))
        .title("Import Data")
        .inner_size(900.0, 700.0)
        .resizable(true)
        .build()
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Stashes data to be exported in the DbManager's state.
/// Used before opening the Export window to pass large result sets.
#[tauri::command]
pub async fn set_export_data(
    data: QueryResult,
    db_manager: State<'_, DbManager>,
) -> Result<(), String> {
    let mut stash = db_manager.export_data.lock().await;
    *stash = Some(data);
    Ok(())
}

/// Retrieves column information for the data currently stashed for export.
/// Used by the Export window to configure column mapping.
#[tauri::command]
pub async fn get_stashed_export_info(
    db_manager: State<'_, DbManager>,
) -> Result<Option<Vec<String>>, String> {
    let stash = db_manager.export_data.lock().await;
    Ok(stash.as_ref().map(|s| s.columns.clone()))
}

/// Opens the Export Data window.
/// Triggered from the "Export" button in various views.
#[tauri::command]
pub async fn open_export(
    app: tauri::AppHandle,
    connection_id: String,
    source_type: String,
    source_name: Option<String>,
    catalog: Option<String>,
    schema: Option<String>,
    query: Option<String>,
    is_current: Option<bool>,
) -> Result<(), String> {
    let mut url = format!(
        "/export?connectionId={}&sourceType={}",
        connection_id, source_type
    );
    if let Some(n) = source_name {
        url.push_str(&format!("&sourceName={}", urlencoding::encode(&n)));
    }
    if let Some(c) = catalog {
        url.push_str(&format!("&catalog={}", urlencoding::encode(&c)));
    }
    if let Some(s) = schema {
        url.push_str(&format!("&schema={}", urlencoding::encode(&s)));
    }
    if let Some(q) = query {
        url.push_str(&format!("&query={}", urlencoding::encode(&q)));
    }
    if let Some(true) = is_current {
        url.push_str("&isCurrent=true");
    }

    tauri::WebviewWindowBuilder::new(&app, "export", tauri::WebviewUrl::App(url.into()))
        .title("Export Data")
        .inner_size(800.0, 700.0)
        .resizable(true)
        .build()
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Analyzes a file and returns a preview of its columns and first few rows.
/// Used by the Import wizard after a file is selected.
#[tauri::command]
pub async fn get_file_preview(
    path: String,
    sheet_name: Option<String>,
    delimiter: Option<String>,
) -> Result<FilePreview, String> {
    use calamine::{open_workbook_auto, Reader};
    use std::path::Path;

    let path_buf = Path::new(&path);
    let ext = path_buf
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    if ext == "csv" {
        let mut rdr = csv::ReaderBuilder::new()
            .delimiter(delimiter.and_then(|d| d.chars().next()).unwrap_or(',') as u8)
            .from_path(&path)
            .map_err(|e| e.to_string())?;

        let headers = rdr
            .headers()
            .map_err(|e| e.to_string())?
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        let mut rows = Vec::new();
        for result in rdr.records().take(10) {
            let record = result.map_err(|e| e.to_string())?;
            rows.push(record.iter().map(|s| s.to_string()).collect());
        }
        Ok(FilePreview {
            columns: headers,
            rows,
            sheets: None,
        })
    } else if ext == "xlsx" || ext == "xls" || ext == "ods" {
        let mut workbook = open_workbook_auto(&path).map_err(|e| e.to_string())?;
        let sheets = workbook.sheet_names().to_vec();
        let sheet = if let Some(name) = sheet_name {
            workbook.worksheet_range(&name).map_err(|e| e.to_string())?
        } else {
            let first_sheet = sheets
                .get(0)
                .ok_or_else(|| "Workbook is empty".to_string())?;
            workbook
                .worksheet_range(first_sheet)
                .map_err(|e| e.to_string())?
        };

        let mut columns = Vec::new();
        let mut rows = Vec::new();
        for (i, row) in sheet.rows().enumerate() {
            if i == 0 {
                columns = row.iter().map(|c| c.to_string()).collect();
            } else if i <= 10 {
                rows.push(row.iter().map(|c| c.to_string()).collect());
            } else {
                break;
            }
        }
        Ok(FilePreview {
            columns,
            rows,
            sheets: Some(sheets),
        })
    } else {
        Err("Unsupported file format".to_string())
    }
}

/// Starts an asynchronous data import process.
/// Progress updates are emitted via the "import-progress" event.
#[tauri::command]
pub async fn perform_import(
    config: ImportConfig,
    app: tauri::AppHandle,
    db_manager: State<'_, DbManager>,
) -> Result<(), String> {
    use tauri::Emitter;

    // Initial progress emit
    app.emit(
        "import-progress",
        ImportProgress {
            total: 0,
            current: 0,
            success_count: 0,
            error_count: 0,
            is_finished: false,
            message: "Starting import...".into(),
            logs: Vec::new(),
        },
    )
    .map_err(|e| e.to_string())?;

    let app_handle = app.clone();
    let manager = db_manager.inner().clone();
    tauri::async_runtime::spawn(async move {
        match manager
            .perform_import(&config, {
                let app_handle = app_handle.clone();
                move |current, total, success, error, message, logs| {
                    let _ = app_handle.emit(
                        "import-progress",
                        ImportProgress {
                            current,
                            total,
                            success_count: success,
                            error_count: error,
                            message: message.to_string(),
                            logs,
                            is_finished: message == "Finished"
                                || (total > 0 && current >= total && message != "Importing..."),
                        },
                    );
                }
            })
            .await
        {
            Ok(_) => {
                // Success is usually already emitted by the final progress call in perform_import
                // but we can ensure finished: true here if needed.
            }
            Err(e) => {
                let _ = app_handle.emit(
                    "import-progress",
                    ImportProgress {
                        current: 0,
                        total: 0,
                        success_count: 0,
                        error_count: 0,
                        message: format!("Error: {}", e),
                        logs: vec![format!("Fatal Error: {}", e)],
                        is_finished: true,
                    },
                );
            }
        }
    });

    Ok(())
}

/// Performs a data export based on the provided configuration.
/// Can export to CSV, JSON, or direct download (if applicable).
#[tauri::command]
pub async fn perform_export(
    config: ExportConfig,
    db_manager: State<'_, DbManager>,
) -> Result<(), String> {
    db_manager.perform_export(config).await
}

/// Opens an export window for exporting multiple tables into a single Excel file.
/// Each table becomes a separate worksheet named after the table.
#[tauri::command]
pub async fn open_export_multi(
    app: tauri::AppHandle,
    connection_id: String,
    source_tables: Vec<String>,
    catalog: Option<String>,
    schema: Option<String>,
) -> Result<(), String> {
    let tables_param = source_tables
        .iter()
        .map(|t| urlencoding::encode(t).to_string())
        .collect::<Vec<_>>()
        .join(",");

    let mut url = format!(
        "/export?connectionId={}&sourceType=multi&sourceTables={}",
        connection_id, tables_param
    );
    if let Some(c) = catalog {
        url.push_str(&format!("&catalog={}", urlencoding::encode(&c)));
    }
    if let Some(s) = schema {
        url.push_str(&format!("&schema={}", urlencoding::encode(&s)));
    }

    tauri::WebviewWindowBuilder::new(&app, "export", tauri::WebviewUrl::App(url.into()))
        .title("Export Data")
        .inner_size(800.0, 700.0)
        .resizable(true)
        .build()
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Imports multiple sheets from an Excel file, each sheet into its own table.
/// Progress updates are emitted via the "import-progress" event.
#[tauri::command]
pub async fn perform_multi_import(
    config: MultiSheetImportConfig,
    app: tauri::AppHandle,
    db_manager: State<'_, DbManager>,
) -> Result<(), String> {
    use tauri::Emitter;

    app.emit(
        "import-progress",
        ImportProgress {
            total: 0,
            current: 0,
            success_count: 0,
            error_count: 0,
            is_finished: false,
            message: "Starting multi-sheet import...".into(),
            logs: Vec::new(),
        },
    )
    .map_err(|e| e.to_string())?;

    let app_handle = app.clone();
    let manager = db_manager.inner().clone();
    tauri::async_runtime::spawn(async move {
        match manager
            .perform_multi_import(&config, {
                let app_handle = app_handle.clone();
                move |current, total, success, error, message, logs| {
                    let _ = app_handle.emit(
                        "import-progress",
                        ImportProgress {
                            current,
                            total,
                            success_count: success,
                            error_count: error,
                            message: message.to_string(),
                            logs,
                            is_finished: message == "Finished"
                                || (total > 0 && current >= total && message != "Importing..."),
                        },
                    );
                }
            })
            .await
        {
            Ok(_) => {}
            Err(e) => {
                let _ = app_handle.emit(
                    "import-progress",
                    ImportProgress {
                        current: 0,
                        total: 0,
                        success_count: 0,
                        error_count: 0,
                        message: format!("Error: {}", e),
                        logs: vec![format!("Fatal Error: {}", e)],
                        is_finished: true,
                    },
                );
            }
        }
    });

    Ok(())
}

/// Retrieves the current application settings.
/// Used to initialize the settings form.
#[tauri::command]
pub async fn get_app_settings(
    settings_manager: tauri::State<'_, SettingsManager>,
) -> Result<AppSettings, String> {
    Ok(settings_manager.load_settings())
}

/// Saves updated application settings to the filesystem.
/// Triggered when the user saves changes in the Settings window.
#[tauri::command]
pub async fn update_app_settings(
    settings_manager: tauri::State<'_, SettingsManager>,
    settings: AppSettings,
) -> Result<(), String> {
    settings_manager.save_settings(&settings)?;
    Ok(())
}

/// Refreshes the schema cache for a specific connection by fetching the latest objects.
/// Triggered manually by the user or after a schema-changing operation.
#[tauri::command]
pub async fn refresh_schema_cache(
    id: String,
    db_manager: State<'_, DbManager>,
    schema_cache_manager: State<'_, crate::db::schema_cache_manager::SchemaCacheManager>,
) -> Result<(), String> {
    let objects = db_manager.get_objects(&id).await?;
    let json = serde_json::to_string(&objects).map_err(|e| e.to_string())?;
    schema_cache_manager.save_cache(&id, &json).await
}

/// Retrieves the cached schema for a connection if it hasn't expired.
/// Used to speed up the loading of the sidebar tree.
#[tauri::command]
pub async fn get_schema_cache(
    id: String,
    settings_manager: State<'_, SettingsManager>,
    schema_cache_manager: State<'_, crate::db::schema_cache_manager::SchemaCacheManager>,
) -> Result<Option<String>, String> {
    let settings = settings_manager.load_settings();
    schema_cache_manager
        .get_cache(&id, settings.schema_cache_ttl)
        .await
}

/// Clears the schema cache for a specific connection.
#[tauri::command]
pub async fn clear_schema_cache(
    id: String,
    schema_cache_manager: State<'_, crate::db::schema_cache_manager::SchemaCacheManager>,
) -> Result<(), String> {
    schema_cache_manager.clear_cache(&id).await
}

/// Clears all cached schemas for all connections.
#[tauri::command]
pub async fn clear_all_schema_cache(
    schema_cache_manager: State<'_, crate::db::schema_cache_manager::SchemaCacheManager>,
) -> Result<(), String> {
    schema_cache_manager.clear_all_cache().await
}
/// Lists all locally available AI models.
#[tauri::command]
pub async fn list_local_models(
    ai_manager: State<'_, crate::ai::ai_manager::AiManager>,
) -> Result<Vec<String>, String> {
    ai_manager.list_local_models()
}

/// Cancels a currently running AI SQL generation task.
#[tauri::command]
pub async fn cancel_ai_generation(
    ai_manager: State<'_, crate::ai::ai_manager::AiManager>,
) -> Result<(), String> {
    ai_manager.cancel_generation();
    Ok(())
}

/// Opens the local directory where AI models are stored.
#[tauri::command]
pub async fn open_models_directory(
    ai_manager: State<'_, crate::ai::ai_manager::AiManager>,
) -> Result<(), String> {
    let path = ai_manager.get_models_directory()?;

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// Tests the connection to an Ollama server and lists available models.
#[tauri::command]
pub async fn test_ollama_connection(url: String) -> Result<Vec<String>, String> {
    let client = reqwest::Client::new();
    let ping_url = format!("{}/api/tags", url.trim_end_matches('/'));

    let res = client
        .get(&ping_url)
        .send()
        .await
        .map_err(|e| format!("Failed to connect to Ollama: {}", e))?;

    if !res.status().is_success() {
        return Err(format!("Ollama server returned error: {}", res.status()));
    }

    #[derive(serde::Deserialize)]
    struct OllamaModel {
        name: String,
    }
    #[derive(serde::Deserialize)]
    struct OllamaTags {
        models: Vec<OllamaModel>,
    }

    let tags: OllamaTags = res
        .json()
        .await
        .map_err(|e| format!("Failed to parse Ollama response: {}", e))?;
    Ok(tags.models.into_iter().map(|m| m.name).collect())
}

/// Generates SQL code using AI based on a natural language prompt and schema information.
#[tauri::command]
pub async fn generate_ai_sql(
    connection_id: String,
    human_input: String,
    model_name: Option<String>,
    database: Option<String>,
    schema: Option<String>,
    app_handle: tauri::AppHandle,
    ai_manager: State<'_, crate::ai::ai_manager::AiManager>,
    settings_manager: State<'_, SettingsManager>,
    config_manager: State<'_, ConfigManager>,
    db_manager: State<'_, DbManager>,
    schema_cache_manager: State<'_, crate::db::schema_cache_manager::SchemaCacheManager>,
) -> Result<crate::models::AiSqlResult, String> {
    let settings = settings_manager.load_settings();
    let ttl = settings.schema_cache_ttl;

    // 1. Get dialect
    let connections = config_manager.load_connections(&settings_manager.get_app_data_path())?;
    let conn = connections
        .iter()
        .find(|c| c.id == connection_id)
        .ok_or_else(|| format!("Connection {} not found", connection_id))?;

    let dialect = format!("{:?}", conn.db_type);

    // 2. Get schema from cache
    let schema_json = schema_cache_manager.get_cache(&connection_id, ttl).await?;
    let schema_json = match schema_json {
        Some(json) => json,
        None => {
            let objects = db_manager.get_objects(&connection_id).await?;
            let json = serde_json::to_string(&objects).map_err(|e| e.to_string())?;
            schema_cache_manager
                .save_cache(&connection_id, &json)
                .await?;
            json
        }
    };

    // 3. Filter by database/schema if provided (scope, not relevance)
    let mut filtered_objects: Vec<crate::models::DbObject> =
        serde_json::from_str(&schema_json).unwrap_or_default();
    if let Some(db) = &database {
        filtered_objects.retain(|obj| obj.catalog.as_ref() == Some(db));
    }
    if let Some(sch) = &schema {
        filtered_objects.retain(|obj| obj.schema.as_ref() == Some(sch));
    }

    let filtered_schema_json =
        serde_json::to_string(&filtered_objects).unwrap_or_else(|_| schema_json.clone());

    // User-configured exclude patterns (comma/newline separated) to trim noise tables.
    let exclude_patterns: Vec<String> = settings
        .ai_exclude_patterns
        .as_deref()
        .unwrap_or("")
        .split([',', '\n'])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    // 4. Generate SQL — single call with the full (non-excluded) schema.
    ai_manager
        .generate_sql(
            &app_handle,
            &settings,
            &filtered_schema_json,
            &dialect,
            &human_input,
            model_name,
            &exclude_patterns,
        )
        .await
}
