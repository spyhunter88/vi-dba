use crate::models::AppSettings;
use serde_json;
use std::fs;
use std::path::PathBuf;
use tauri::AppHandle;
use tauri::Manager;

pub struct SettingsManager {
    settings_file: PathBuf,
}

impl SettingsManager {
    pub fn new(app_handle: &AppHandle) -> Self {
        let config_dir = app_handle
            .path()
            .app_config_dir()
            .expect("Could not find app config directory");

        if !config_dir.exists() {
            fs::create_dir_all(&config_dir).expect("Could not create config directory");
        }

        let settings_file = config_dir.join("settings.json");
        Self { settings_file }
    }

    pub fn load_settings(&self) -> AppSettings {
        if !self.settings_file.exists() {
            return AppSettings {
                app_data_path: None,
                schema_cache_ttl: None,
                ai_mode: None,
                ollama_url: None,
                ollama_model: None,
                cloud_provider: None,
                cloud_api_key: None,
                cloud_model: None,
                cloud_base_url: None,
                ai_exclude_patterns: None,
                history_max_total: Some(1000),
                history_max_per_connection: Some(100),
                history_max_lifetime_days: Some(30),
                history_max_lifetime_hours: Some(0),
                history_max_lifetime_minutes: Some(0),
                enable_history_retention_total: Some(true),
                enable_history_retention_per_connection: Some(false),
                enable_history_retention_lifetime: Some(false),
            };
        }

        match fs::read_to_string(&self.settings_file) {
            Ok(content) => serde_json::from_str(&content).unwrap_or(AppSettings {
                app_data_path: None,
                schema_cache_ttl: None,
                ai_mode: None,
                ollama_url: None,
                ollama_model: None,
                cloud_provider: None,
                cloud_api_key: None,
                cloud_model: None,
                cloud_base_url: None,
                ai_exclude_patterns: None,
                history_max_total: Some(1000),
                history_max_per_connection: Some(100),
                history_max_lifetime_days: Some(30),
                history_max_lifetime_hours: Some(0),
                history_max_lifetime_minutes: Some(0),
                enable_history_retention_total: Some(true),
                enable_history_retention_per_connection: Some(false),
                enable_history_retention_lifetime: Some(false),
            }),
            Err(_) => AppSettings {
                app_data_path: None,
                schema_cache_ttl: None,
                ai_mode: None,
                ollama_url: None,
                ollama_model: None,
                cloud_provider: None,
                cloud_api_key: None,
                cloud_model: None,
                cloud_base_url: None,
                ai_exclude_patterns: None,
                history_max_total: Some(1000),
                history_max_per_connection: Some(100),
                history_max_lifetime_days: Some(30),
                history_max_lifetime_hours: Some(0),
                history_max_lifetime_minutes: Some(0),
                enable_history_retention_total: Some(true),
                enable_history_retention_per_connection: Some(false),
                enable_history_retention_lifetime: Some(false),
            },
        }
    }

    pub fn save_settings(&self, settings: &AppSettings) -> Result<(), String> {
        let content = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
        fs::write(&self.settings_file, content).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn get_app_data_path(&self) -> PathBuf {
        let settings = self.load_settings();
        if let Some(path) = settings.app_data_path {
            let p = PathBuf::from(path);
            if p.exists() {
                return p;
            }
        }

        // Use a "data" subdirectory in the app config directory instead of current_dir()
        // This avoids saving files in the project root during development, which triggers hot-reloads.
        let data_dir = self.settings_file.parent().unwrap().join("data");
        if !data_dir.exists() {
            let _ = fs::create_dir_all(&data_dir);
        }
        data_dir
    }
}
