use async_trait::async_trait;
use sibyl::{Connection, Environment};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::models::{
    ColumnInfo, DbObject, QueryResult, RoutineDefinition, RowActionResult, TableColumn,
    TableDefinition, ViewDefinition,
};
use super::Database;

pub struct OracleDatabase {
    connection: Arc<Mutex<Connection<'static>>>,
    env: Arc<Environment>,
    pub default_schema: Option<String>,
}

// Sibyl Connection<'static> is not Send/Sync by default due to the OCI handle.
unsafe impl Send for OracleDatabase {}
unsafe impl Sync for OracleDatabase {}

impl OracleDatabase {
    pub async fn new(
        host: &str,
        port: u16,
        user: &str,
        pass: &str,
        db_name: &str,
        default_schema: Option<String>,
    ) -> Result<Self, String> {
        let env = Environment::new().map_err(|e| e.to_string())?;
        let env = Arc::new(env);
        let connect_string = format!("//{}:{}/{}", host, port, db_name);
        // SAFETY: env is Arc-managed and outlives the connection stored in the same struct.
        let env_ref: &'static Environment = unsafe { std::mem::transmute(&*env) };
        let conn = env_ref
            .connect(user, pass, &connect_string)
            .await
            .map_err(|e| e.to_string())?;
        Ok(Self {
            env,
            connection: Arc::new(Mutex::new(conn)),
            default_schema,
        })
    }

    /// Converts a JSON value to an Oracle SQL literal.
    fn value_to_sql_literal(val: &serde_json::Value) -> String {
        match val {
            serde_json::Value::Null => "NULL".to_string(),
            serde_json::Value::Bool(b) => if *b { "1".to_string() } else { "0".to_string() },
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::String(s) => format!("'{}'", s.replace('\'', "''")),
            _ => format!("'{}'", val.to_string().replace('\'', "''")),
        }
    }

    /// Maps a generic data type to the closest Oracle equivalent.
    fn map_type_to_oracle(data_type: &str, length: Option<&str>) -> String {
        match data_type.to_uppercase().as_str() {
            "VARCHAR" | "TEXT" | "STRING" | "NVARCHAR" | "VARCHAR2" => {
                format!("VARCHAR2({})", length.unwrap_or("255"))
            }
            "CHAR" | "NCHAR" => format!("CHAR({})", length.unwrap_or("1")),
            "INT" | "INTEGER" => "NUMBER(10)".to_string(),
            "BIGINT" => "NUMBER(19)".to_string(),
            "SMALLINT" => "NUMBER(5)".to_string(),
            "DECIMAL" | "NUMERIC" | "NUMBER" => {
                format!("NUMBER({})", length.unwrap_or("18,2"))
            }
            "FLOAT" | "DOUBLE" | "REAL" => "FLOAT".to_string(),
            "BOOLEAN" | "BOOL" => "NUMBER(1)".to_string(),
            "DATE" => "DATE".to_string(),
            "DATETIME" | "TIMESTAMP" => "TIMESTAMP".to_string(),
            "CLOB" | "TEXT" => "CLOB".to_string(),
            "BLOB" | "BINARY" => "BLOB".to_string(),
            _ => data_type.to_string(),
        }
    }

    fn get_create_table_sql(def: &TableDefinition) -> String {
        let schema_name = def.schema.as_deref().unwrap_or_default();
        let table = if schema_name.is_empty() {
            format!("\"{}\"", def.name)
        } else {
            format!("\"{}\".\"{}\"", schema_name, def.name)
        };

        let mut col_defs = Vec::new();
        let mut pk_cols = Vec::new();

        for col in &def.columns {
            let type_str = Self::map_type_to_oracle(&col.data_type, col.length.as_deref());
            let mut col_def = format!("\"{}\" {}", col.name, type_str);
            if !col.is_nullable {
                col_def.push_str(" NOT NULL");
            }
            if let Some(default) = &col.default_value {
                col_def = format!("\"{}\" {} DEFAULT {}", col.name, type_str, default);
                if !col.is_nullable {
                    col_def.push_str(" NOT NULL");
                }
            }
            col_defs.push(col_def);
            if col.is_primary_key {
                pk_cols.push(format!("\"{}\"", col.name));
            }
        }

        if !pk_cols.is_empty() {
            col_defs.push(format!("PRIMARY KEY ({})", pk_cols.join(", ")));
        }

        format!("CREATE TABLE {} (\n    {}\n)", table, col_defs.join(",\n    "))
    }

    fn build_alter_statements(old: &TableDefinition, new: &TableDefinition) -> Vec<String> {
        let schema_name = new.schema.as_deref().unwrap_or_default();
        let table = if schema_name.is_empty() {
            format!("\"{}\"", new.name)
        } else {
            format!("\"{}\".\"{}\"", schema_name, new.name)
        };
        let mut statements = Vec::new();

        let old_cols: HashMap<&str, &TableColumn> =
            old.columns.iter().map(|c| (c.name.as_str(), c)).collect();
        let new_cols: HashMap<&str, &TableColumn> =
            new.columns.iter().map(|c| (c.name.as_str(), c)).collect();

        for old_col in &old.columns {
            if !new_cols.contains_key(old_col.name.as_str()) {
                statements.push(format!("ALTER TABLE {} DROP COLUMN \"{}\"", table, old_col.name));
            }
        }

        for new_col in &new.columns {
            if let Some(old_col) = old_cols.get(new_col.name.as_str()) {
                let new_type = Self::map_type_to_oracle(&new_col.data_type, new_col.length.as_deref());
                let old_type = Self::map_type_to_oracle(&old_col.data_type, old_col.length.as_deref());
                if new_type != old_type || new_col.is_nullable != old_col.is_nullable {
                    let nullable = if new_col.is_nullable { "" } else { " NOT NULL" };
                    statements.push(format!(
                        "ALTER TABLE {} MODIFY \"{}\" {}{}",
                        table, new_col.name, new_type, nullable
                    ));
                }
            } else {
                let type_str = Self::map_type_to_oracle(&new_col.data_type, new_col.length.as_deref());
                let nullable = if new_col.is_nullable { "" } else { " NOT NULL" };
                let default_clause = new_col.default_value.as_ref()
                    .map(|d| format!(" DEFAULT {}", d))
                    .unwrap_or_default();
                statements.push(format!(
                    "ALTER TABLE {} ADD \"{}\" {}{}{}",
                    table, new_col.name, type_str, default_clause, nullable
                ));
            }
        }

        let old_pks: Vec<&str> = old.columns.iter().filter(|c| c.is_primary_key).map(|c| c.name.as_str()).collect();
        let new_pks: Vec<&str> = new.columns.iter().filter(|c| c.is_primary_key).map(|c| c.name.as_str()).collect();
        if old_pks != new_pks {
            if !old_pks.is_empty() {
                statements.push(format!(
                    "ALTER TABLE {} DROP PRIMARY KEY",
                    table
                ));
            }
            if !new_pks.is_empty() {
                let pk_str = new_pks.iter().map(|c| format!("\"{}\"", c)).collect::<Vec<_>>().join(", ");
                statements.push(format!("ALTER TABLE {} ADD PRIMARY KEY ({})", table, pk_str));
            }
        }

        statements
    }
}

#[async_trait]
impl Database for OracleDatabase {
    async fn execute_query(
        &self,
        query: &str,
        _table_name: Option<String>,
        catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<QueryResult, String> {
        let start = std::time::Instant::now();
        let conn = self.connection.lock().await;

        let q_trimmed = query.trim().to_uppercase();
        let effective_schema = schema.or(catalog).or_else(|| self.default_schema.clone());
        if let Some(sch) = effective_schema {
            if !q_trimmed.starts_with("ALTER SESSION") {
                let alter_session = format!("ALTER SESSION SET CURRENT_SCHEMA = {}", sch);
                let stmt = conn.prepare(&alter_session).await.map_err(|e| e.to_string())?;
                stmt.execute(&[]).await.map_err(|e| {
                    format!("Failed to switch schema to `{}`: {}", sch, e)
                })?;
            }
        }

        let stmt = conn.prepare(query).await.map_err(|e| e.to_string())?;
        match stmt.query(&[]).await {
            Ok(mut rows_iter) => {
                let mut columns = Vec::new();
                let mut rows = Vec::new();

                if let Ok(cols) = rows_iter.columns() {
                    columns = cols.iter().map(|c| c.name().to_string()).collect();
                }

                while let Some(row) = rows_iter.next().await.map_err(|e| e.to_string())? {
                    let mut map = serde_json::Map::new();
                    for (i, col_name) in columns.iter().enumerate() {
                        let val = row.get::<Option<String>>(i)
                            .map(|v| v.map(serde_json::Value::String).unwrap_or(serde_json::Value::Null))
                            .or_else(|_| row.get::<Option<f64>>(i).map(|v| {
                                v.map(|n| serde_json::Number::from_f64(n)
                                    .map(serde_json::Value::Number)
                                    .unwrap_or(serde_json::Value::Null))
                                    .unwrap_or(serde_json::Value::Null)
                            }))
                            .or_else(|_| row.get::<Option<i64>>(i).map(|v| {
                                v.map(|n| serde_json::Value::Number(n.into()))
                                    .unwrap_or(serde_json::Value::Null)
                            }))
                            .unwrap_or(serde_json::Value::Null);
                        map.insert(col_name.clone(), val);
                    }
                    rows.push(serde_json::Value::Object(map));
                }

                let col_count = columns.len();
                Ok(QueryResult {
                    columns,
                    column_types: vec!["TEXT".to_string(); col_count],
                    rows,
                    affected_rows: 0,
                    execution_time_ms: start.elapsed().as_millis(),
                    primary_keys: vec![],
                    table_name: None,
                })
            }
            Err(_) => {
                // Attempt as DML
                let affected = stmt.execute(&[]).await.map_err(|e| e.to_string())?;
                Ok(QueryResult {
                    columns: vec![],
                    column_types: vec![],
                    rows: vec![],
                    affected_rows: affected as u64,
                    execution_time_ms: start.elapsed().as_millis(),
                    primary_keys: vec![],
                    table_name: None,
                })
            }
        }
    }

    async fn get_table_list(
        &self,
        _catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<QueryResult, String> {
        let owner = schema.unwrap_or_else(|| "USER".to_string()).to_uppercase();
        let query = format!(
            "SELECT TABLE_NAME AS \"Name\" FROM ALL_TABLES WHERE OWNER = '{}' ORDER BY TABLE_NAME",
            owner
        );
        self.execute_query(&query, None, None, None).await
    }

    async fn get_routine_list(
        &self,
        _catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<QueryResult, String> {
        let owner = schema.unwrap_or_else(|| "USER".to_string()).to_uppercase();
        let query = format!(
            "SELECT OBJECT_NAME AS \"Name\", OBJECT_TYPE AS \"Type\" FROM ALL_OBJECTS WHERE OWNER = '{}' AND OBJECT_TYPE IN ('PROCEDURE', 'FUNCTION') ORDER BY OBJECT_NAME",
            owner
        );
        self.execute_query(&query, None, None, None).await
    }

    async fn get_objects(&self) -> Result<Vec<DbObject>, String> {
        let schema = self.default_schema.as_deref().unwrap_or("").to_uppercase();
        let owner_filter = if schema.is_empty() {
            "OWNER = SYS_CONTEXT('USERENV', 'CURRENT_SCHEMA')".to_string()
        } else {
            format!("OWNER = '{}'", schema)
        };

        let table_query = format!(
            "SELECT OWNER, TABLE_NAME AS name, 'table' AS object_type
             FROM ALL_TABLES WHERE {} ORDER BY TABLE_NAME",
            owner_filter
        );
        let view_query = format!(
            "SELECT OWNER, VIEW_NAME AS name, 'view' AS object_type
             FROM ALL_VIEWS WHERE {} ORDER BY VIEW_NAME",
            owner_filter
        );
        let routine_query = format!(
            "SELECT OWNER, OBJECT_NAME AS name, LOWER(OBJECT_TYPE) AS object_type
             FROM ALL_OBJECTS WHERE {} AND OBJECT_TYPE IN ('PROCEDURE', 'FUNCTION', 'PACKAGE') ORDER BY OBJECT_NAME",
            owner_filter
        );
        let col_query = format!(
            "SELECT OWNER, TABLE_NAME, COLUMN_NAME, DATA_TYPE FROM ALL_TAB_COLUMNS WHERE {} ORDER BY TABLE_NAME, COLUMN_ID",
            owner_filter
        );

        let table_res = self.execute_query(&table_query, None, None, None).await.unwrap_or_default();
        let view_res = self.execute_query(&view_query, None, None, None).await.unwrap_or_default();
        let routine_res = self.execute_query(&routine_query, None, None, None).await.unwrap_or_default();
        let col_res = self.execute_query(&col_query, None, None, None).await.unwrap_or_default();

        let mut column_map: HashMap<String, Vec<ColumnInfo>> = HashMap::new();
        for row in col_res.rows {
            if let Some(obj) = row.as_object() {
                let table = obj.get("TABLE_NAME").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let name = obj.get("COLUMN_NAME").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let data_type = obj.get("DATA_TYPE").and_then(|v| v.as_str()).unwrap_or("").to_string();
                column_map.entry(table).or_default().push(ColumnInfo { name, data_type });
            }
        }

        let mut objects = Vec::new();

        for row in table_res.rows.iter().chain(view_res.rows.iter()) {
            if let Some(map) = row.as_object() {
                let name = map.get("NAME").or_else(|| map.get("name")).and_then(|v| v.as_str()).unwrap_or("").to_string();
                let object_type = map.get("OBJECT_TYPE").or_else(|| map.get("object_type")).and_then(|v| v.as_str()).unwrap_or("table").to_string();
                let schema_val = map.get("OWNER").and_then(|v| v.as_str()).map(|s| s.to_string());
                let cols = column_map.get(&name).cloned();
                objects.push(DbObject {
                    name,
                    object_type,
                    schema: schema_val,
                    catalog: None,
                    columns: cols,
                    parent: None,
                    description: None,
                });
            }
        }

        for row in routine_res.rows {
            if let Some(map) = row.as_object() {
                let name = map.get("NAME").or_else(|| map.get("name")).and_then(|v| v.as_str()).unwrap_or("").to_string();
                let object_type = map.get("OBJECT_TYPE").or_else(|| map.get("object_type")).and_then(|v| v.as_str()).unwrap_or("procedure").to_string();
                let schema_val = map.get("OWNER").and_then(|v| v.as_str()).map(|s| s.to_string());
                objects.push(DbObject {
                    name,
                    object_type,
                    schema: schema_val,
                    catalog: None,
                    columns: None,
                    parent: None,
                    description: None,
                });
            }
        }

        Ok(objects)
    }

    async fn update_row(
        &self,
        table_name: &str,
        pks: HashMap<String, serde_json::Value>,
        column: &str,
        value: serde_json::Value,
        _catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<RowActionResult, String> {
        let schema_name = schema.or_else(|| self.default_schema.clone()).unwrap_or_default();
        let full_table = if schema_name.is_empty() {
            format!("\"{}\"", table_name)
        } else {
            format!("\"{}\".\"{}\"", schema_name, table_name)
        };

        let val_literal = Self::value_to_sql_literal(&value);
        let mut pk_list: Vec<(String, serde_json::Value)> = pks.into_iter().collect();
        pk_list.sort_by(|a, b| a.0.cmp(&b.0));
        let where_clause = pk_list
            .iter()
            .map(|(k, v)| format!("\"{}\" = {}", k, Self::value_to_sql_literal(v)))
            .collect::<Vec<_>>()
            .join(" AND ");

        let query = format!(
            "UPDATE {} SET \"{}\" = {} WHERE {}",
            full_table, column, val_literal, where_clause
        );

        self.execute_query(&query, None, None, None).await?;

        Ok(RowActionResult { affected_rows: 1, query })
    }

    async fn insert_row(
        &self,
        table_name: &str,
        data: HashMap<String, serde_json::Value>,
        _catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<RowActionResult, String> {
        let schema_name = schema.or_else(|| self.default_schema.clone()).unwrap_or_default();
        let full_table = if schema_name.is_empty() {
            format!("\"{}\"", table_name)
        } else {
            format!("\"{}\".\"{}\"", schema_name, table_name)
        };

        let mut cols: Vec<String> = data.keys().cloned().collect();
        cols.sort();
        let col_list = cols.iter().map(|c| format!("\"{}\"", c)).collect::<Vec<_>>().join(", ");
        let val_list = cols.iter().map(|c| Self::value_to_sql_literal(&data[c])).collect::<Vec<_>>().join(", ");

        let query = format!("INSERT INTO {} ({}) VALUES ({})", full_table, col_list, val_list);

        self.execute_query(&query, None, None, None).await?;

        Ok(RowActionResult { affected_rows: 1, query })
    }

    async fn get_table_definition(
        &self,
        table_name: &str,
        _catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<TableDefinition, String> {
        let owner = schema.clone().or_else(|| self.default_schema.clone()).unwrap_or_default().to_uppercase();
        let owner_filter = if owner.is_empty() {
            format!("TABLE_NAME = '{}'", table_name.to_uppercase())
        } else {
            format!("OWNER = '{}' AND TABLE_NAME = '{}'", owner, table_name.to_uppercase())
        };

        let col_query = format!(
            "SELECT COLUMN_NAME, DATA_TYPE, NULLABLE, DATA_LENGTH, DATA_PRECISION, DATA_SCALE, DATA_DEFAULT
             FROM ALL_TAB_COLUMNS WHERE {} ORDER BY COLUMN_ID",
            owner_filter
        );
        let pk_query = format!(
            "SELECT c.COLUMN_NAME FROM ALL_CONS_COLUMNS c JOIN ALL_CONSTRAINTS k ON c.CONSTRAINT_NAME = k.CONSTRAINT_NAME AND c.OWNER = k.OWNER
             WHERE k.CONSTRAINT_TYPE = 'P' AND k.TABLE_NAME = '{}' {}",
            table_name.to_uppercase(),
            if owner.is_empty() { String::new() } else { format!("AND k.OWNER = '{}'", owner) }
        );

        let col_res = self.execute_query(&col_query, None, None, None).await?;
        let pk_res = self.execute_query(&pk_query, None, None, None).await.unwrap_or_default();

        let pk_set: HashSet<String> = pk_res.rows.iter().filter_map(|r| {
            r.get("COLUMN_NAME").and_then(|v| v.as_str()).map(|s| s.to_string())
        }).collect();

        let mut columns = Vec::new();
        for row in col_res.rows {
            if let serde_json::Value::Object(map) = row {
                let col_name = map.get("COLUMN_NAME").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let data_type = map.get("DATA_TYPE").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let is_nullable = map.get("NULLABLE").and_then(|v| v.as_str()).unwrap_or("Y") == "Y";
                let data_length = map.get("DATA_LENGTH").and_then(|v| v.as_i64());
                let precision = map.get("DATA_PRECISION").and_then(|v| v.as_i64());
                let scale = map.get("DATA_SCALE").and_then(|v| v.as_i64());
                let default_value = map.get("DATA_DEFAULT").and_then(|v| v.as_str())
                    .filter(|s| !s.trim().is_empty())
                    .map(|s| s.trim().to_string());

                let length = if let Some(p) = precision {
                    if let Some(s) = scale {
                        if s > 0 { Some(format!("{},{}", p, s)) } else { Some(p.to_string()) }
                    } else { Some(p.to_string()) }
                } else {
                    data_length.map(|l| l.to_string())
                };

                columns.push(TableColumn {
                    is_primary_key: pk_set.contains(&col_name),
                    name: col_name,
                    data_type,
                    is_nullable,
                    is_auto_increment: false,
                    default_value,
                    comment: None,
                    length,
                    collation: None,
                });
            }
        }

        Ok(TableDefinition {
            name: table_name.to_string(),
            columns,
            catalog: None,
            schema,
            comment: None,
            collation: None,
        })
    }

    async fn create_table(&self, definition: TableDefinition) -> Result<(), String> {
        let sql = Self::get_create_table_sql(&definition);
        self.execute_query(&sql, None, None, None).await?;
        Ok(())
    }

    async fn alter_table(&self, old: TableDefinition, new: TableDefinition) -> Result<(), String> {
        for stmt in Self::build_alter_statements(&old, &new) {
            self.execute_query(&stmt, None, None, None).await.map_err(|e| {
                format!("Failed: {}\nError: {}", stmt, e)
            })?;
        }
        Ok(())
    }

    async fn generate_table_sql(
        &self,
        old: Option<TableDefinition>,
        new: TableDefinition,
    ) -> Result<String, String> {
        if let Some(old_def) = old {
            Ok(Self::build_alter_statements(&old_def, &new).join(";\n"))
        } else {
            Ok(Self::get_create_table_sql(&new))
        }
    }

    async fn get_routine_definition(
        &self,
        name: &str,
        routine_type: &str,
        _catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<RoutineDefinition, String> {
        let owner = schema.clone().or_else(|| self.default_schema.clone()).unwrap_or_default().to_uppercase();
        let owner_filter = if owner.is_empty() {
            format!("NAME = '{}'", name.to_uppercase())
        } else {
            format!("OWNER = '{}' AND NAME = '{}'", owner, name.to_uppercase())
        };

        let query = format!(
            "SELECT TEXT FROM ALL_SOURCE WHERE {} ORDER BY LINE",
            owner_filter
        );

        let res = self.execute_query(&query, None, None, None).await?;
        let definition = res.rows.iter()
            .filter_map(|r| r.get("TEXT").and_then(|v| v.as_str()))
            .collect::<Vec<_>>()
            .join("");

        Ok(RoutineDefinition {
            name: name.to_string(),
            routine_type: routine_type.to_string(),
            definition,
            catalog: None,
            schema,
        })
    }

    async fn save_routine(&self, definition: RoutineDefinition) -> Result<(), String> {
        let def_sql = definition.definition.trim();
        let upper = def_sql.to_uppercase();
        let normalized = if upper.starts_with("CREATE OR REPLACE") {
            def_sql.to_string()
        } else if upper.starts_with("CREATE") {
            format!("CREATE OR REPLACE{}", &def_sql["CREATE".len()..])
        } else {
            let schema_name = definition.schema.as_deref()
                .or(self.default_schema.as_deref())
                .unwrap_or_default();
            let rt = definition.routine_type.to_uppercase();
            let prefix = if schema_name.is_empty() {
                format!("CREATE OR REPLACE {} \"{}\" AS\n", rt, definition.name)
            } else {
                format!("CREATE OR REPLACE {} \"{}\".\"{}\" AS\n", rt, schema_name, definition.name)
            };
            format!("{}{}", prefix, def_sql)
        };
        self.execute_query(&normalized, None, None, None).await?;
        Ok(())
    }

    async fn get_view_definition(
        &self,
        name: &str,
        _catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<ViewDefinition, String> {
        let owner = schema.clone().or_else(|| self.default_schema.clone()).unwrap_or_default().to_uppercase();
        let owner_filter = if owner.is_empty() {
            format!("VIEW_NAME = '{}'", name.to_uppercase())
        } else {
            format!("OWNER = '{}' AND VIEW_NAME = '{}'", owner, name.to_uppercase())
        };

        let query = format!("SELECT TEXT FROM ALL_VIEWS WHERE {}", owner_filter);
        let res = self.execute_query(&query, None, None, None).await?;

        let definition = res.rows.first()
            .and_then(|r| r.get("TEXT").and_then(|v| v.as_str()))
            .unwrap_or("")
            .to_string();

        Ok(ViewDefinition {
            name: name.to_string(),
            definition: format!("CREATE OR REPLACE VIEW \"{}\" AS\n{}", name, definition),
            catalog: None,
            schema,
        })
    }

    async fn save_view(&self, definition: ViewDefinition) -> Result<(), String> {
        let def_sql = definition.definition.trim();
        let upper = def_sql.to_uppercase();
        let normalized = if upper.starts_with("CREATE OR REPLACE") {
            def_sql.to_string()
        } else if upper.starts_with("CREATE") {
            format!("CREATE OR REPLACE{}", &def_sql["CREATE".len()..])
        } else {
            let schema_name = definition.schema.as_deref()
                .or(self.default_schema.as_deref())
                .unwrap_or_default();
            let prefix = if schema_name.is_empty() {
                format!("CREATE OR REPLACE VIEW \"{}\" AS\n", definition.name)
            } else {
                format!("CREATE OR REPLACE VIEW \"{}\".\"{}\" AS\n", schema_name, definition.name)
            };
            format!("{}{}", prefix, def_sql)
        };
        self.execute_query(&normalized, None, None, None).await?;
        Ok(())
    }

    async fn close(&self) {
        // Connection cleanup handled by drop; Sibyl doesn't have an async close in Arc context
    }
}
