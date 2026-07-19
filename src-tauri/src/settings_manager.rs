use crate::models::AppSettings;
use serde_json;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::AppHandle;
use tauri::Manager;

pub struct SettingsManager {
    settings_file: PathBuf,
}

impl SettingsManager {
    pub fn new(app_handle: &AppHandle) -> Self {
        let base_dir = Self::resolve_base_dir(app_handle);
        let settings_file = base_dir.join("settings.json");

        let manager = Self { settings_file };
        // Make sure a valid settings file exists on disk from the very first run.
        manager.ensure_initialized();
        manager
    }

    /// Resolves the directory that holds the config/data by default.
    ///
    /// Preference order (portable-first):
    /// 1. The directory that contains the running executable ("thư mục đang chạy").
    /// 2. The current working directory.
    /// 3. The OS app config directory (guaranteed writable, used only as a last
    ///    resort when the app is installed in a read-only location, e.g. Program Files).
    fn resolve_base_dir(app_handle: &AppHandle) -> PathBuf {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                if Self::is_writable(dir) {
                    return dir.to_path_buf();
                }
            }
        }

        if let Ok(cwd) = std::env::current_dir() {
            if Self::is_writable(&cwd) {
                return cwd;
            }
        }

        let config_dir = app_handle
            .path()
            .app_config_dir()
            .expect("Could not find app config directory");
        let _ = fs::create_dir_all(&config_dir);
        config_dir
    }

    /// Probes whether we can actually create files inside `dir`.
    fn is_writable(dir: &Path) -> bool {
        if fs::create_dir_all(dir).is_err() {
            return false;
        }
        let probe = dir.join(".vidbconnect_write_test");
        match fs::write(&probe, b"") {
            Ok(_) => {
                let _ = fs::remove_file(&probe);
                true
            }
            Err(_) => false,
        }
    }

    /// Ensures the settings file exists and is readable. Creates it with defaults
    /// when missing, and re-initializes (backing up the broken file) when the
    /// existing content cannot be parsed.
    fn ensure_initialized(&self) {
        if let Some(parent) = self.settings_file.parent() {
            let _ = fs::create_dir_all(parent);
        }

        if !self.settings_file.exists() {
            let _ = self.write_settings(&AppSettings::default());
            return;
        }

        // Detect a corrupt/unreadable file and rebuild it from defaults.
        let needs_reinit = match fs::read_to_string(&self.settings_file) {
            Ok(content) => serde_json::from_str::<AppSettings>(&content).is_err(),
            Err(_) => true,
        };

        if needs_reinit {
            self.reinit();
        }
    }

    /// Backs up the broken settings file (best-effort) and writes fresh defaults.
    fn reinit(&self) {
        let backup = self.settings_file.with_extension("json.bak");
        let _ = fs::rename(&self.settings_file, &backup);
        let _ = self.write_settings(&AppSettings::default());
    }

    fn write_settings(&self, settings: &AppSettings) -> Result<(), String> {
        let content = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
        fs::write(&self.settings_file, content).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn load_settings(&self) -> AppSettings {
        if !self.settings_file.exists() {
            // File went missing after startup — recreate it so callers still get defaults.
            let _ = self.write_settings(&AppSettings::default());
            return AppSettings::default();
        }

        match fs::read_to_string(&self.settings_file) {
            Ok(content) => match serde_json::from_str(&content) {
                Ok(settings) => settings,
                Err(_) => {
                    // Corrupt content: repair on the fly and fall back to defaults.
                    self.reinit();
                    AppSettings::default()
                }
            },
            Err(_) => {
                self.reinit();
                AppSettings::default()
            }
        }
    }

    pub fn save_settings(&self, settings: &AppSettings) -> Result<(), String> {
        self.write_settings(settings)
    }

    pub fn get_app_data_path(&self) -> PathBuf {
        let settings = self.load_settings();
        if let Some(path) = settings.app_data_path {
            let p = PathBuf::from(path);
            if p.exists() {
                return p;
            }
        }

        // Default: keep data alongside the settings file (the running directory),
        // so the app is portable and configuration lives next to the executable.
        let base_dir = self
            .settings_file
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        if !base_dir.exists() {
            let _ = fs::create_dir_all(&base_dir);
        }
        base_dir
    }
}
