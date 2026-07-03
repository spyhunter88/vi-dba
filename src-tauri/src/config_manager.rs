use std::fs;
use std::path::PathBuf;
use crate::models::ConnectionConfig;
use serde_json;

pub struct ConfigManager {}

impl ConfigManager {
    pub fn new() -> Self {
        Self {}
    }

    fn get_connections_file(&self, base_path: &PathBuf) -> PathBuf {
        if !base_path.exists() {
            let _ = fs::create_dir_all(&base_path);
        }
        base_path.join("connections.json")
    }

    pub fn load_connections(&self, base_path: &PathBuf) -> Result<Vec<ConnectionConfig>, String> {
        let file_path = self.get_connections_file(base_path);
        if !file_path.exists() {
            return Ok(vec![]);
        }

        let content = fs::read_to_string(file_path).map_err(|e| e.to_string())?;
        let connections: Vec<ConnectionConfig> = serde_json::from_str(&content).map_err(|e| e.to_string())?;
        Ok(connections)
    }

    pub fn save_connections(&self, base_path: &PathBuf, connections: &[ConnectionConfig]) -> Result<(), String> {
        let file_path = self.get_connections_file(base_path);
        let content = serde_json::to_string_pretty(connections).map_err(|e| e.to_string())?;
        fs::write(file_path, content).map_err(|e| e.to_string())?;
        Ok(())
    }
}
