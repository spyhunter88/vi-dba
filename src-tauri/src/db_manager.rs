use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::AbortHandle;
use crate::models::{ConnectionConfig, DbType, QueryResult, DbObject, TableDefinition, RoutineDefinition, ViewDefinition, ImportConfig, ExportConfig, MultiSheetImportConfig, RowActionResult};
use crate::db::{Database, mysql::MySqlDatabase, postgres::PostgreSqlDatabase, sqlite::SqliteDatabase, sqlserver::SqlServerDatabase, mongo::MongoDatabase};
#[cfg(feature = "oracle")]
use crate::db::oracle::OracleDatabase;
use sqlx::{MySqlPool, PgPool, SqlitePool};
use tiberius::{AuthMethod, Config};

#[derive(Clone)]
pub struct DbManager {
    pools: Arc<Mutex<HashMap<String, Arc<dyn Database>>>>,
    pub export_data: Arc<Mutex<Option<QueryResult>>>,
    active_queries: Arc<Mutex<HashMap<String, AbortHandle>>>,
}

impl DbManager {
    pub fn new() -> Self {
        Self {
            pools: Arc::new(Mutex::new(HashMap::new())),
            export_data: Arc::new(Mutex::new(None)),
            active_queries: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn perform_import<F>(&self, config: &ImportConfig, progress: F) -> Result<(), String> 
    where F: Fn(usize, usize, usize, usize, &str, Vec<String>) + Send + 'static 
    {
        use calamine::{Reader, open_workbook_auto, Data};
        use std::path::Path;
        use crate::models::{TableDefinition, TableColumn};

        let db = {
            let pools = self.pools.lock().await;
            pools.get(&config.connection_id).ok_or("Not connected")?.clone()
        };

        let mut logs = Vec::new();
        let log = |logs: &mut Vec<String>, msg: String| {
            logs.push(msg);
            if logs.len() > 1000 { logs.remove(0); }
        };

        log(&mut logs, format!("Starting import to table: {}", config.target_table));
        progress(0, 0, 0, 0, "Initializing...", logs.clone());

        // 1. Check if table exists, if not create it
        log(&mut logs, format!("Checking if table `{}` exists...", config.target_table));
        progress(0, 0, 0, 0, "Checking table...", logs.clone());

        let table_def = db.get_table_definition(&config.target_table, config.catalog.clone(), config.schema.clone()).await;
        let table_exists = match &table_def {
            Ok(def) => !def.columns.is_empty(),
            Err(_) => false,
        };

        if !table_exists {
            log(&mut logs, format!("Table `{}` not found. Preparing to create it...", config.target_table));
            progress(0, 0, 0, 0, "Creating table...", logs.clone());
            
            let mut columns = Vec::new();
            for (source, target) in &config.column_mappings {
                if target.is_empty() { continue; }
                let data_type = config.column_types.get(source).cloned().unwrap_or_else(|| "TEXT".to_string());
                columns.push(TableColumn {
                    name: target.clone(),
                    data_type,
                    is_nullable: true,
                    is_primary_key: false,
                    is_auto_increment: false,
                    default_value: None,
                    comment: None,
                    length: None,
                    collation: None,
                });
            }

            if columns.is_empty() {
                let err = "No columns mapped for table creation".to_string();
                log(&mut logs, format!("Error: {}", err));
                progress(0, 0, 0, 0, "Error", logs.clone());
                return Err(err);
            }

            let def = TableDefinition {
                name: config.target_table.clone(),
                columns,
                catalog: config.catalog.clone(),
                schema: config.schema.clone(),
                comment: None,
                collation: None,
            };

            // Generate SQL for log
            match db.generate_table_sql(None, def.clone()).await {
                Ok(sql) => {
                    log(&mut logs, format!("Generated Create SQL: {}", sql));
                    progress(0, 0, 0, 0, "Executing SQL...", logs.clone());
                    
                    db.create_table(def).await.map_err(|e| {
                        let msg = format!("Failed to create table: {}", e);
                        log(&mut logs, msg.clone());
                        progress(0, 0, 0, 0, "Error", logs.clone());
                        msg
                    })?;
                    log(&mut logs, "Table created successfully.".to_string());
                }
                Err(e) => {
                    log(&mut logs, format!("SQL Generation Error: {}", e));
                    // Fallback to direct creation if SQL generation fails but driver might still work
                    db.create_table(def).await.map_err(|e| {
                        let msg = format!("Failed to create table: {}", e);
                        log(&mut logs, msg.clone());
                        progress(0, 0, 0, 0, "Error", logs.clone());
                        msg
                    })?;
                    log(&mut logs, "Table created successfully (via fallback).".to_string());
                }
            }
            progress(0, 0, 0, 0, "Table ready", logs.clone());
        } else {
            log(&mut logs, format!("Target table `{}` already exists.", config.target_table));
        }

        let path_buf = Path::new(&config.file_path);
        let ext = path_buf.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();

        log(&mut logs, "Parsing source file...".to_string());
        progress(0, 0, 0, 0, "Parsing file...", logs.clone());

        let mut all_data: Vec<HashMap<String, serde_json::Value>> = Vec::new();

        if ext == "csv" {
            let mut rdr = csv::ReaderBuilder::new()
                .delimiter(config.delimiter.as_ref().and_then(|d| d.chars().next()).unwrap_or(',') as u8)
                .from_path(&config.file_path)
                .map_err(|e| e.to_string())?;
            
            let headers = if config.has_header {
                rdr.headers().map_err(|e| e.to_string())?.clone()
            } else {
                csv::StringRecord::from(vec![""; 0]) // dummy
            };

            for result in rdr.records() {
                let record = result.map_err(|e| e.to_string())?;
                let mut row_data = HashMap::new();
                for (source_col, target_col) in &config.column_mappings {
                    if target_col.is_empty() { continue; }
                    
                    let val = if config.has_header {
                        // Find index of source_col in headers
                        headers.iter().position(|h| h == source_col)
                            .and_then(|idx| record.get(idx))
                    } else {
                        // source_col is index as string
                        source_col.parse::<usize>().ok().and_then(|idx| record.get(idx))
                    };

                    if let Some(v) = val {
                        row_data.insert(target_col.clone(), serde_json::Value::String(v.to_string()));
                    }
                }
                all_data.push(row_data);
            }
        } else if ext == "xlsx" || ext == "xls" || ext == "ods" {
            let mut workbook = open_workbook_auto(&config.file_path).map_err(|e| e.to_string())?;
            let sheet = if let Some(name) = &config.sheet_name {
                workbook.worksheet_range(name)
                    .map_err(|e| e.to_string())?
            } else {
                let names = workbook.sheet_names().to_vec();
                let first_sheet = names.get(0).ok_or_else(|| "Workbook is empty".to_string())?;
                workbook.worksheet_range(first_sheet)
                    .map_err(|e| e.to_string())?
            };

            let mut source_cols = Vec::new();
            for (i, row) in sheet.rows().enumerate() {
                if i == 0 && config.has_header {
                    source_cols = row.iter().map(|c| c.to_string()).collect();
                    continue;
                }
                
                let mut row_data = HashMap::new();
                for (source_col, target_col) in &config.column_mappings {
                    if target_col.is_empty() { continue; }
                    
                    let val = if config.has_header {
                        source_cols.iter().position(|h| h == source_col)
                            .and_then(|idx| row.get(idx))
                    } else {
                        source_col.parse::<usize>().ok().and_then(|idx| row.get(idx))
                    };

                    if let Some(v) = val {
                        let json_val = match v {
                            Data::String(s) => serde_json::Value::String(s.clone()),
                            Data::Float(f) => serde_json::Number::from_f64(f.to_owned()).map(serde_json::Value::Number).unwrap_or(serde_json::Value::Null),
                            Data::Int(i) => serde_json::Value::Number(i.clone().into()),
                            Data::Bool(b) => serde_json::Value::Bool(b.to_owned()),
                            _ => serde_json::Value::String(v.to_string()),
                        };
                        row_data.insert(target_col.clone(), json_val);
                    }
                }
                all_data.push(row_data);
            }
        }



        let total = all_data.len();
        let mut success = 0;
        let mut error = 0;

        log(&mut logs, format!("Importing {} rows...", total));
        progress(0, total, 0, 0, "Starting...", logs.clone());

        for (i, row) in all_data.into_iter().enumerate() {
            match db.insert_row(&config.target_table, row, config.catalog.clone(), config.schema.clone()).await {
                Ok(_) => success += 1,
                Err(e) => {
                    error += 1;
                    log(&mut logs, format!("Row {} Error: {}", i + 1, e));
                }
            }
            if i % 100 == 0 || i == total - 1 {
                progress(i + 1, total, success, error, "Importing...", logs.clone());
            }
        }

        log(&mut logs, format!("Import finished. Success: {}, Errors: {}", success, error));
        progress(total, total, success, error, "Finished", logs.clone());

        Ok(())
    }

    pub async fn perform_multi_import<F>(&self, config: &MultiSheetImportConfig, progress: F) -> Result<(), String>
    where F: Fn(usize, usize, usize, usize, &str, Vec<String>) + Send + 'static
    {
        use calamine::{Reader, open_workbook_auto, Data};
        use crate::models::{TableDefinition, TableColumn};

        let db = {
            let pools = self.pools.lock().await;
            pools.get(&config.connection_id).ok_or("Not connected")?.clone()
        };

        let mut logs: Vec<String> = Vec::new();

        let mut workbook = open_workbook_auto(&config.file_path).map_err(|e| e.to_string())?;

        // Read all sheet data upfront to know total row count
        let mut sheets_data: Vec<(String, Vec<HashMap<String, serde_json::Value>>)> = Vec::new();
        for mapping in &config.sheet_mappings {
            let sheet = match workbook.worksheet_range(&mapping.sheet_name) {
                Ok(s) => s,
                Err(e) => {
                    logs.push(format!("Cannot read sheet '{}': {}", mapping.sheet_name, e));
                    continue;
                }
            };

            let mut headers: Vec<String> = Vec::new();
            let mut rows: Vec<HashMap<String, serde_json::Value>> = Vec::new();

            for (i, row) in sheet.rows().enumerate() {
                if i == 0 && config.has_header {
                    headers = row.iter().enumerate().map(|(j, c)| {
                        let s = c.to_string();
                        if s.is_empty() { format!("col_{}", j) } else { s }
                    }).collect();
                    continue;
                }
                if headers.is_empty() {
                    headers = (0..row.len()).map(|j| format!("col_{}", j)).collect();
                }
                let mut row_map: HashMap<String, serde_json::Value> = HashMap::new();
                for (col_idx, val) in row.iter().enumerate() {
                    if col_idx >= headers.len() { break; }
                    let col_name = headers[col_idx].clone();
                    if col_name.is_empty() { continue; }
                    let json_val = match val {
                        Data::String(s) => serde_json::Value::String(s.clone()),
                        Data::Float(f) => serde_json::Number::from_f64(*f)
                            .map(serde_json::Value::Number)
                            .unwrap_or(serde_json::Value::Null),
                        Data::Int(n) => serde_json::Value::Number((*n).into()),
                        Data::Bool(b) => serde_json::Value::Bool(*b),
                        Data::Empty => serde_json::Value::Null,
                        _ => serde_json::Value::String(val.to_string()),
                    };
                    row_map.insert(col_name, json_val);
                }
                rows.push(row_map);
            }

            // Store headers alongside target table for table creation
            let target = mapping.target_table.clone();
            // If table doesn't exist we'll create it using headers; store header info separately
            // For now just store rows; we'll re-derive headers from first row keys later
            logs.push(format!("Read sheet '{}': {} rows", mapping.sheet_name, rows.len()));

            // Store (target_table, headers, rows) but we encode headers in a dummy entry
            // Actually let's store them properly via a tuple with headers
            let _ = target;
            sheets_data.push((mapping.target_table.clone(), rows));
            // Keep headers for table creation - re-read from mapping
        }

        // Re-read sheets to get headers for table creation (need them separately)
        let mut workbook2 = open_workbook_auto(&config.file_path).map_err(|e| e.to_string())?;
        let mut sheet_headers: HashMap<String, Vec<String>> = HashMap::new();
        for mapping in &config.sheet_mappings {
            if let Ok(sheet) = workbook2.worksheet_range(&mapping.sheet_name) {
                if let Some(first_row) = sheet.rows().next() {
                    if config.has_header {
                        let headers: Vec<String> = first_row.iter().enumerate().map(|(j, c)| {
                            let s = c.to_string();
                            if s.is_empty() { format!("col_{}", j) } else { s }
                        }).collect();
                        sheet_headers.insert(mapping.target_table.clone(), headers);
                    }
                }
            }
        }

        let total: usize = sheets_data.iter().map(|(_, rows)| rows.len()).sum();
        let mut processed = 0usize;
        let mut global_success = 0usize;
        let mut global_error = 0usize;

        logs.push(format!("Multi-sheet import: {} sheets, {} total rows", sheets_data.len(), total));
        progress(0, total, 0, 0, "Starting...", logs.clone());

        for (target_table, data_rows) in sheets_data {
            logs.push(format!("--- Importing into '{}'", target_table));
            progress(processed, total, global_success, global_error, &format!("Table: {}", target_table), logs.clone());

            // Check/create table
            let table_def = db.get_table_definition(&target_table, config.catalog.clone(), config.schema.clone()).await;
            let table_exists = match &table_def {
                Ok(def) => !def.columns.is_empty(),
                Err(_) => false,
            };

            if !table_exists {
                if let Some(headers) = sheet_headers.get(&target_table) {
                    logs.push(format!("Creating table '{}'...", target_table));
                    let columns: Vec<TableColumn> = headers.iter().map(|h| TableColumn {
                        name: h.clone(),
                        data_type: "TEXT".to_string(),
                        is_nullable: true,
                        is_primary_key: false,
                        is_auto_increment: false,
                        default_value: None,
                        comment: None,
                        length: None,
                        collation: None,
                    }).collect();

                    let def = TableDefinition {
                        name: target_table.clone(),
                        columns,
                        catalog: config.catalog.clone(),
                        schema: config.schema.clone(),
                        comment: None,
                        collation: None,
                    };

                    if let Err(e) = db.create_table(def).await {
                        logs.push(format!("Error creating '{}': {}", target_table, e));
                        processed += data_rows.len();
                        progress(processed, total, global_success, global_error, "Importing...", logs.clone());
                        continue;
                    }
                    logs.push(format!("Table '{}' created.", target_table));
                }
            }

            let sheet_total = data_rows.len();
            for (i, row) in data_rows.into_iter().enumerate() {
                match db.insert_row(&target_table, row, config.catalog.clone(), config.schema.clone()).await {
                    Ok(_) => global_success += 1,
                    Err(e) => {
                        global_error += 1;
                        if logs.len() < 1000 {
                            logs.push(format!("[{}] Row {} Error: {}", target_table, i + 1, e));
                        }
                    }
                }
                processed += 1;
                if i % 100 == 0 || i == sheet_total - 1 {
                    progress(processed, total, global_success, global_error, "Importing...", logs.clone());
                }
            }
            logs.push(format!("'{}' done.", target_table));
        }

        logs.push(format!("Import finished. Success: {}, Errors: {}", global_success, global_error));
        progress(total, total, global_success, global_error, "Finished", logs.clone());

        Ok(())
    }

    pub async fn perform_export(&self, config: ExportConfig) -> Result<(), String> {
        let db = {
            let pools = self.pools.lock().await;
            pools.get(&config.connection_id).ok_or("Not connected")?.clone()
        };

        // Multi-table Excel export: each table becomes a separate worksheet
        if config.source_type == "multi" {
            use rust_xlsxwriter::Workbook;
            let tables = config.source_tables.as_ref().ok_or("No tables specified for multi-table export")?;
            let mut workbook = Workbook::new();
            for table in tables {
                let result = db.execute_query(
                    &format!("SELECT * FROM {}", table),
                    Some(table.clone()),
                    config.catalog.clone(),
                    config.schema.clone()
                ).await?;
                let worksheet = workbook.add_worksheet();
                let sheet_name: String = table.chars()
                    .map(|c| match c { ':' | '\\' | '/' | '?' | '*' | '[' | ']' => '_', _ => c })
                    .take(31)
                    .collect();
                worksheet.set_name(&sheet_name).map_err(|e| e.to_string())?;
                for (col_idx, col_name) in result.columns.iter().enumerate() {
                    worksheet.write_string(0, col_idx as u16, col_name).map_err(|e| e.to_string())?;
                }
                for (row_idx, row) in result.rows.iter().enumerate() {
                    if let Some(obj) = row.as_object() {
                        for (col_idx, col_name) in result.columns.iter().enumerate() {
                            let val = obj.get(col_name).unwrap_or(&serde_json::Value::Null);
                            match val {
                                serde_json::Value::Number(n) => {
                                    if let Some(f) = n.as_f64() {
                                        worksheet.write_number((row_idx + 1) as u32, col_idx as u16, f).map_err(|e| e.to_string())?;
                                    }
                                },
                                serde_json::Value::String(s) => {
                                    worksheet.write_string((row_idx + 1) as u32, col_idx as u16, s).map_err(|e| e.to_string())?;
                                },
                                serde_json::Value::Bool(b) => {
                                    worksheet.write_boolean((row_idx + 1) as u32, col_idx as u16, *b).map_err(|e| e.to_string())?;
                                },
                                serde_json::Value::Null => {},
                                _ => {
                                    worksheet.write_string((row_idx + 1) as u32, col_idx as u16, &val.to_string()).map_err(|e| e.to_string())?;
                                }
                            }
                        }
                    }
                }
            }
            workbook.save(&config.output_path).map_err(|e| e.to_string())?;
            return Ok(());
        }

        let result = if config.source_type == "current" {
            let stash = self.export_data.lock().await;
            stash.clone().ok_or("No current result set found to export")?
        } else if config.source_type == "table" {
            let table = config.source_name.as_ref().ok_or("Table name missing")?;
            db.execute_query(&format!("SELECT * FROM {}", table), Some(table.clone()), config.catalog.clone(), config.schema.clone()).await?
        } else {
            let query = config.query.as_ref().ok_or("Query missing")?;
            db.execute_query(query, None, config.catalog.clone(), config.schema.clone()).await?
        };

        if config.output_format == "csv" {
            let mut wtr = csv::Writer::from_path(&config.output_path).map_err(|e| e.to_string())?;
            
            // Filter columns if specified
            let col_indices: Vec<usize> = if let Some(cols) = &config.columns {
                cols.iter().filter_map(|c| result.columns.iter().position(|rc| rc == c)).collect()
            } else {
                (0..result.columns.len()).collect()
            };

            // Write header
            let filtered_headers: Vec<String> = col_indices.iter().map(|&i| result.columns[i].clone()).collect();
            wtr.write_record(&filtered_headers).map_err(|e| e.to_string())?;

            for row in result.rows {
                if let Some(obj) = row.as_object() {
                    let mut record = Vec::new();
                    for &i in &col_indices {
                        let col_name = &result.columns[i];
                        let val = obj.get(col_name).unwrap_or(&serde_json::Value::Null);
                        record.push(match val {
                            serde_json::Value::Null => String::new(),
                            serde_json::Value::String(s) => s.clone(),
                            _ => val.to_string(),
                        });
                    }
                    wtr.write_record(&record).map_err(|e| e.to_string())?;
                }
            }
            wtr.flush().map_err(|e| e.to_string())?;
        } else if config.output_format == "excel" {
            use rust_xlsxwriter::{Workbook};
            let mut workbook = Workbook::new();
            let worksheet = workbook.add_worksheet();
            
            let col_indices: Vec<usize> = if let Some(cols) = &config.columns {
                cols.iter().filter_map(|c| result.columns.iter().position(|rc| rc == c)).collect()
            } else {
                (0..result.columns.len()).collect()
            };

            // Write header
            for (col_idx, &i) in col_indices.iter().enumerate() {
                worksheet.write_string(0, col_idx as u16, &result.columns[i]).map_err(|e| e.to_string())?;
            }

            for (row_idx, row) in result.rows.iter().enumerate() {
                if let Some(obj) = row.as_object() {
                    for (col_idx, &i) in col_indices.iter().enumerate() {
                        let col_name = &result.columns[i];
                        let val = obj.get(col_name).unwrap_or(&serde_json::Value::Null);
                        match val {
                            serde_json::Value::Number(n) => {
                                if let Some(f) = n.as_f64() {
                                    worksheet.write_number((row_idx + 1) as u32, col_idx as u16, f).map_err(|e| e.to_string())?;
                                }
                            },
                            serde_json::Value::String(s) => {
                                worksheet.write_string((row_idx + 1) as u32, col_idx as u16, s).map_err(|e| e.to_string())?;
                            },
                            serde_json::Value::Bool(b) => {
                                worksheet.write_boolean((row_idx + 1) as u32, col_idx as u16, *b).map_err(|e| e.to_string())?;
                            },
                            serde_json::Value::Null => {},
                            _ => {
                                worksheet.write_string((row_idx + 1) as u32, col_idx as u16, &val.to_string()).map_err(|e| e.to_string())?;
                            }
                        }
                    }
                }
            }
            workbook.save(&config.output_path).map_err(|e| e.to_string())?;
        } else if config.output_format == "sql" {
             let mut sql = String::new();
             let table_name = config.source_name.as_deref().unwrap_or("exported_table");
             
             for row in result.rows {
                 if let Some(obj) = row.as_object() {
                     let mut cols = Vec::new();
                     let mut vals = Vec::new();
                     
                     for key in obj.keys() {
                         cols.push(format!("`{}`", key));
                         let val = obj.get(key).unwrap();
                         vals.push(match val {
                             serde_json::Value::Null => "NULL".to_string(),
                             serde_json::Value::String(s) => format!("'{}'", s.replace("'", "''")),
                             serde_json::Value::Number(n) => n.to_string(),
                             serde_json::Value::Bool(b) => if *b { "1" } else { "0" }.to_string(),
                             _ => format!("'{}'", val.to_string().replace("'", "''")),
                         });
                     }
                     sql.push_str(&format!("INSERT INTO {} ({}) VALUES ({});\n", table_name, cols.join(", "), vals.join(", ")));
                 }
             }
             std::fs::write(&config.output_path, sql).map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    pub async fn generate_table_sql(&self, id: &str, old_definition: Option<TableDefinition>, new_definition: TableDefinition) -> Result<String, String> {
        let db = {
            let pools = self.pools.lock().await;
            pools.get(id).ok_or("Not connected")?.clone()
        };
        db.generate_table_sql(old_definition, new_definition).await
    }

    pub async fn is_connected(&self, id: &str) -> bool {
        let pools = self.pools.lock().await;
        pools.contains_key(id)
    }

    pub async fn connect(&self, config: ConnectionConfig) -> Result<String, String> {
        let id = config.id.clone();
        
        let (db, server_version): (Arc<dyn Database>, String) = match config.db_type {
            DbType::MySQL => {
                let url = self.build_url(&config);
                let pool = MySqlPool::connect(&url).await.map_err(|e| e.to_string())?;
                let db = MySqlDatabase::new(pool, config.database.clone()).await;
                let version = db.version.display.clone();
                (Arc::new(db), version)
            }
            DbType::PostgreSQL => {
                let url = self.build_url(&config);
                let pool = PgPool::connect(&url).await.map_err(|e| e.to_string())?;
                (Arc::new(PostgreSqlDatabase::new(pool, config.database.clone())), "Unknown".to_string())
            }
            DbType::SQLite => {
                let url = self.build_url(&config);
                let pool = SqlitePool::connect(&url).await.map_err(|e| e.to_string())?;
                (Arc::new(SqliteDatabase::new(pool)), "Local".to_string())
            }
            DbType::SQLServer => {
                let mut c = Config::new();
                c.host(&config.host);
                c.port(config.port);
                c.authentication(AuthMethod::sql_server(&config.user, config.password.as_deref().unwrap_or("")));
                c.trust_cert(); // Important for easier connections
                c.database(config.database.as_deref().unwrap_or(""));
                
                let db = SqlServerDatabase::new(c, config.database.clone()).await.map_err(|e| e.to_string())?;
                (Arc::new(db), "Unknown".to_string())
            }
            DbType::Oracle => {
                #[cfg(feature = "oracle")]
                {
                    let db_name = config.database.as_deref().unwrap_or("XE");
                    let db = OracleDatabase::new(&config.host, config.port, &config.user, config.password.as_deref().unwrap_or(""), db_name, config.database.clone()).await?;
                    (Arc::new(db), "Unknown".to_string())
                }
                #[cfg(not(feature = "oracle"))]
                {
                    return Err("Oracle support is not enabled in this build. Please install OCI libraries and compile with --features oracle".to_string());
                }
            }
            DbType::MongoDB => {
                // Determine if password has special chars, might need encoding if building URI manually
                // Or use simple URI if user/pass are simple. 
                // Better: construct URI properly? 
                // For now, simple format: mongodb://user:pass@host:port
                let uri = if !config.user.is_empty() {
                    format!(
                        "mongodb://{}:{}@{}:{}",
                        urlencoding::encode(&config.user),
                        urlencoding::encode(config.password.as_deref().unwrap_or("")),
                        config.host,
                        config.port
                    )
                } else {
                    format!("mongodb://{}:{}", config.host, config.port)
                };
                
                let db_name = config.database.as_deref().unwrap_or("test");
                let db = MongoDatabase::new(&uri, db_name).await?;
                (Arc::new(db), "Unknown".to_string())
            }
        };

        let mut pools = self.pools.lock().await;
        pools.insert(id, db);
        Ok(server_version)
    }

    pub async fn disconnect(&self, id: &str) {
        let mut pools = self.pools.lock().await;
        if let Some(db) = pools.remove(id) {
            db.close().await;
        }
    }

    pub async fn execute_query(&self, id: &str, query: &str, table_name: Option<String>, catalog: Option<String>, schema: Option<String>, exec_id: Option<String>) -> Result<QueryResult, String> {
        let db = {
            let pools = self.pools.lock().await;
            pools.get(id).ok_or("Not connected")?.clone()
        };

        let query_owned = query.to_string();
        let task = tokio::spawn(async move {
            db.execute_query(&query_owned, table_name, catalog, schema).await
        });

        if let Some(key) = exec_id.clone() {
            self.active_queries.lock().await.insert(key, task.abort_handle());
        }

        let result = match task.await {
            Ok(res) => res,
            Err(e) if e.is_cancelled() => Err("Query cancelled by user".to_string()),
            Err(e) => Err(format!("Task error: {}", e)),
        };

        if let Some(key) = exec_id {
            self.active_queries.lock().await.remove(&key);
        }

        result
    }

    pub async fn cancel_query(&self, exec_id: &str) -> bool {
        if let Some(handle) = self.active_queries.lock().await.remove(exec_id) {
            handle.abort();
            true
        } else {
            false
        }
    }

    pub async fn get_table_list(&self, id: &str, catalog: Option<String>, schema: Option<String>) -> Result<QueryResult, String> {
        let db = {
            let pools = self.pools.lock().await;
            pools.get(id).ok_or("Not connected")?.clone()
        };
        db.get_table_list(catalog, schema).await
    }

    pub async fn get_routine_list(&self, id: &str, catalog: Option<String>, schema: Option<String>) -> Result<QueryResult, String> {
        let db = {
            let pools = self.pools.lock().await;
            pools.get(id).ok_or("Not connected")?.clone()
        };
        db.get_routine_list(catalog, schema).await
    }

    pub async fn get_objects(&self, id: &str) -> Result<Vec<DbObject>, String> {
        let db = {
            let pools = self.pools.lock().await;
            pools.get(id).ok_or("Not connected")?.clone()
        };
        db.get_objects().await
    }

    pub async fn update_row(
        &self, 
        id: &str, 
        table_name: &str, 
        pks: HashMap<String, serde_json::Value>, 
        column: &str, 
        value: serde_json::Value,
        catalog: Option<String>,
        schema: Option<String>
    ) -> Result<RowActionResult, String> {
        let db = {
            let pools = self.pools.lock().await;
            pools.get(id).ok_or("Not connected")?.clone()
        };
        db.update_row(table_name, pks, column, value, catalog, schema).await
    }

    pub async fn insert_row(
        &self,
        id: &str,
        table_name: &str,
        data: HashMap<String, serde_json::Value>,
        catalog: Option<String>,
        schema: Option<String>
    ) -> Result<RowActionResult, String> {
        let db = {
            let pools = self.pools.lock().await;
            pools.get(id).ok_or("Not connected")?.clone()
        };
        db.insert_row(table_name, data, catalog, schema).await
    }

    pub async fn get_table_definition(&self, id: &str, table_name: &str, catalog: Option<String>, schema: Option<String>) -> Result<TableDefinition, String> {
        let db = {
            let pools = self.pools.lock().await;
            pools.get(id).ok_or("Not connected")?.clone()
        };
        db.get_table_definition(table_name, catalog, schema).await
    }

    pub async fn get_table_indexes(&self, id: &str, table_name: &str, catalog: Option<String>, schema: Option<String>) -> Result<crate::models::TableIndexInfo, String> {
        let db = {
            let pools = self.pools.lock().await;
            pools.get(id).ok_or("Not connected")?.clone()
        };
        db.get_table_indexes(table_name, catalog, schema).await
    }

    pub async fn create_table(&self, id: &str, definition: TableDefinition) -> Result<(), String> {
        let db = {
            let pools = self.pools.lock().await;
            pools.get(id).ok_or("Not connected")?.clone()
        };
        db.create_table(definition).await
    }

    pub async fn alter_table(&self, id: &str, old: TableDefinition, new: TableDefinition) -> Result<(), String> {
        let db = {
            let pools = self.pools.lock().await;
            pools.get(id).ok_or("Not connected")?.clone()
        };
        db.alter_table(old, new).await
    }

    pub async fn get_routine_definition(&self, id: &str, name: &str, routine_type: &str, catalog: Option<String>, schema: Option<String>) -> Result<RoutineDefinition, String> {
        let db = {
            let pools = self.pools.lock().await;
            pools.get(id).ok_or("Not connected")?.clone()
        };
        db.get_routine_definition(name, routine_type, catalog, schema).await
    }

    pub async fn save_routine(&self, id: &str, definition: RoutineDefinition) -> Result<(), String> {
        let db = {
            let pools = self.pools.lock().await;
            pools.get(id).ok_or("Not connected")?.clone()
        };
        db.save_routine(definition).await
    }

    pub async fn get_view_definition(&self, id: &str, name: &str, catalog: Option<String>, schema: Option<String>) -> Result<ViewDefinition, String> {
        let db = {
            let pools = self.pools.lock().await;
            pools.get(id).ok_or("Not connected")?.clone()
        };
        db.get_view_definition(name, catalog, schema).await
    }

    pub async fn save_view(&self, id: &str, definition: ViewDefinition) -> Result<(), String> {
        let db = {
            let pools = self.pools.lock().await;
            pools.get(id).ok_or("Not connected")?.clone()
        };
        db.save_view(definition).await
    }

    fn build_url(&self, config: &ConnectionConfig) -> String {
        match config.db_type {
            DbType::MySQL => {
                format!(
                    "mysql://{}:{}@{}:{}/{}",
                    urlencoding::encode(&config.user),
                    urlencoding::encode(config.password.as_deref().unwrap_or("")),
                    config.host,
                    config.port,
                    config.database.as_deref().unwrap_or("")
                )
            }
            DbType::PostgreSQL => {
                format!(
                    "postgres://{}:{}@{}:{}/{}",
                    urlencoding::encode(&config.user),
                    urlencoding::encode(config.password.as_deref().unwrap_or("")),
                    config.host,
                    config.port,
                    config.database.as_deref().unwrap_or("")
                )
            }
            DbType::SQLite => {
                format!(
                    "sqlite:{}",
                    config.host // For SQLite, host is used as path
                )
            }
            _ => String::new() // Should not handle others via build_url
        }
    }
}
