use std::fs;
use std::path::PathBuf;
use sqlx::{sqlite::SqliteConnectOptions, ConnectOptions, SqlitePool};
use tauri::{AppHandle, Manager};
use chrono::Utc;

pub struct SchemaCacheManager {
    pool: Option<SqlitePool>,
    cache_dir: PathBuf,
}

impl SchemaCacheManager {
    pub fn new(app_handle: &AppHandle) -> Self {
        let settings_manager = app_handle.state::<crate::settings_manager::SettingsManager>();
        let base_path = settings_manager.get_app_data_path();
        let cache_dir = base_path.join("cache");
        
        if !cache_dir.exists() {
            fs::create_dir_all(&cache_dir).expect("Could not create cache directory");
        }

        Self {
            pool: None,
            cache_dir,
        }
    }

    pub async fn init(&mut self) -> Result<(), String> {
        let db_path = self.cache_dir.join("schema_cache.db");
        let options = SqliteConnectOptions::new()
            .filename(db_path)
            .create_if_missing(true)
            .disable_statement_logging();

        let pool = SqlitePool::connect_with(options).await.map_err(|e| e.to_string())?;
        
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS schema_cache (
                connection_id TEXT PRIMARY KEY,
                schema_json   TEXT NOT NULL,
                fetched_at    INTEGER NOT NULL
            )"
        ).execute(&pool).await.map_err(|e| e.to_string())?;

        self.pool = Some(pool);
        Ok(())
    }

    pub async fn save_cache(&self, connection_id: &str, schema_json: &str) -> Result<(), String> {
        let pool = self.pool.as_ref().ok_or("SchemaCacheManager not initialized")?;
        let now = Utc::now().timestamp();

        sqlx::query(
            "INSERT OR REPLACE INTO schema_cache (connection_id, schema_json, fetched_at)
             VALUES (?, ?, ?)"
        )
        .bind(connection_id)
        .bind(schema_json)
        .bind(now)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub async fn get_cache(&self, connection_id: &str, ttl_minutes: Option<u32>) -> Result<Option<String>, String> {
        let pool = self.pool.as_ref().ok_or("SchemaCacheManager not initialized")?;

        let row: Option<(String, i64)> = sqlx::query_as(
            "SELECT schema_json, fetched_at FROM schema_cache WHERE connection_id = ?"
        )
        .bind(connection_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;

        if let Some((json, fetched_at)) = row {
            if let Some(ttl) = ttl_minutes {
                let now = Utc::now().timestamp();
                if now - fetched_at > (ttl as i64 * 60) {
                    return Ok(None); // Expired
                }
            }
            return Ok(Some(json));
        }

        Ok(None)
    }

    pub async fn clear_cache(&self, connection_id: &str) -> Result<(), String> {
        let pool = self.pool.as_ref().ok_or("SchemaCacheManager not initialized")?;

        sqlx::query("DELETE FROM schema_cache WHERE connection_id = ?")
            .bind(connection_id)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub async fn clear_all_cache(&self) -> Result<(), String> {
        let pool = self.pool.as_ref().ok_or("SchemaCacheManager not initialized")?;

        sqlx::query("DELETE FROM schema_cache")
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;

        Ok(())
    }
}
