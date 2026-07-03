use std::fs;
use std::path::{PathBuf};
use crate::models::ScriptInfo;
use chrono::{DateTime, Utc};

pub struct ScriptManager {}

impl ScriptManager {
    pub fn new() -> Self {
        Self {}
    }

    fn get_script_dir(&self, base_path: &PathBuf, connection_id: &str, database: Option<String>, schema: Option<String>) -> PathBuf {
        let mut dir = base_path.join("scripts").join(connection_id);
        if let Some(db) = database {
            dir = dir.join(db);
            if let Some(sch) = schema {
                dir = dir.join(sch);
            }
        }
        if !dir.exists() {
            let _ = fs::create_dir_all(&dir);
        }
        dir
    }

    pub fn list_scripts(&self, base_path: &PathBuf, connection_id: &str, database: Option<String>, schema: Option<String>) -> Result<Vec<ScriptInfo>, String> {
        let mut scripts = Vec::new();
        let base_dir = base_path.join("scripts").join(connection_id);
        
        if !base_dir.exists() {
            return Ok(vec![]);
        }

        // If specific db/schema provided, just scan that dir
        if database.is_some() {
            let dir = self.get_script_dir(base_path, connection_id, database.clone(), schema.clone());
            self.scan_dir(&dir, &mut scripts, database, schema)?;
        } else {
            // Scan everything under connection_id recursively to find all .sql files
            self.scan_recursive(&base_dir, &mut scripts, &base_dir)?;
        }
        
        Ok(scripts)
    }

    fn scan_recursive(&self, dir: &PathBuf, scripts: &mut Vec<ScriptInfo>, base_conn_dir: &PathBuf) -> Result<(), String> {
        if !dir.exists() { return Ok(()); }
        let entries = fs::read_dir(dir).map_err(|e| e.to_string())?;
        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.is_dir() {
                self.scan_recursive(&path, scripts, base_conn_dir)?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("sql") {
                // Infer database and schema from path
                let rel_path = path.strip_prefix(base_conn_dir).map_err(|e| e.to_string())?;
                let components: Vec<_> = rel_path.components().collect();
                
                let mut db = None;
                let mut sch = None;
                
                if components.len() >= 3 {
                    // database/schema/filename.sql
                    db = components[0].as_os_str().to_str().map(|s| s.to_string());
                    sch = components[1].as_os_str().to_str().map(|s| s.to_string());
                } else if components.len() == 2 {
                    // database/filename.sql
                    db = components[0].as_os_str().to_str().map(|s| s.to_string());
                }

                self.add_script_info(&path, scripts, db, sch)?;
            }
        }
        Ok(())
    }

    fn scan_dir(&self, dir: &PathBuf, scripts: &mut Vec<ScriptInfo>, database: Option<String>, schema: Option<String>) -> Result<(), String> {
        if !dir.exists() { return Ok(()); }
        let entries = fs::read_dir(dir).map_err(|e| e.to_string())?;
        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.is_file() && (path.extension().and_then(|s| s.to_str()) == Some("sql")) {
                self.add_script_info(&path, scripts, database.clone(), schema.clone())?;
            }
        }
        Ok(())
    }

    fn add_script_info(&self, path: &PathBuf, scripts: &mut Vec<ScriptInfo>, database: Option<String>, schema: Option<String>) -> Result<(), String> {
        let metadata = fs::metadata(path).map_err(|e| e.to_string())?;
        let created_at: DateTime<Utc> = metadata.created().map_err(|e| e.to_string())?.into();
        let modified_at: DateTime<Utc> = metadata.modified().map_err(|e| e.to_string())?.into();
        
        scripts.push(ScriptInfo {
            name: path.file_name().and_then(|s| s.to_str()).unwrap_or_default().to_string(),
            path: path.to_string_lossy().to_string(),
            database,
            schema,
            created_at: created_at.to_rfc3339(),
            modified_at: modified_at.to_rfc3339(),
        });
        Ok(())
    }

    pub fn save_script(&self, base_path: &PathBuf, connection_id: &str, name: &str, content: &str, database: Option<String>, schema: Option<String>) -> Result<ScriptInfo, String> {
        let dir = self.get_script_dir(base_path, connection_id, database.clone(), schema.clone());
        // Ensure name ends with .sql
        let mut file_name = name.to_string();
        if !file_name.ends_with(".sql") {
            file_name.push_str(".sql");
        }
        
        let path = dir.join(&file_name);
        fs::write(&path, content).map_err(|e| e.to_string())?;
        
        let metadata = fs::metadata(&path).map_err(|e| e.to_string())?;
        let created_at: DateTime<Utc> = metadata.created().map_err(|e| e.to_string())?.into();
        let modified_at: DateTime<Utc> = metadata.modified().map_err(|e| e.to_string())?.into();
        
        Ok(ScriptInfo {
            name: file_name,
            path: path.to_string_lossy().to_string(),
            database,
            schema,
            created_at: created_at.to_rfc3339(),
            modified_at: modified_at.to_rfc3339(),
        })
    }

    pub fn read_script(&self, _base_path: &PathBuf, _connection_id: &str, name: &str) -> Result<String, String> {
        // Since list_scripts now returns absolute paths in ScriptInfo.path, we can just read it.
        // But the frontend might expect us to resolve it.
        // Actually, name is often the filename. If it's an absolute path, we use it.
        let path = PathBuf::from(name);
        if !path.exists() {
            return Err("Script not found".to_string());
        }
        
        fs::read_to_string(path).map_err(|e| e.to_string())
    }
}
