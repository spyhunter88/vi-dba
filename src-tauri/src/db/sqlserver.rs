use super::{utils, Database};
use crate::models::ColumnInfo;
use crate::models::{
    DbObject, QueryResult, RoutineDefinition, RowActionResult, TableColumn, TableDefinition,
    ViewDefinition,
};
use async_trait::async_trait;
use futures::StreamExt;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tiberius::{Client, Config, QueryItem};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

/// SQL Server implementation of the Database trait using Tiberius.
pub struct SqlServerDatabase {
    client: Arc<Mutex<Option<Client<Compat<TcpStream>>>>>,
    pub default_database: Option<String>,
}

impl SqlServerDatabase {
    /// Creates a new SQL Server database instance and establishes a TCP connection.
    pub async fn new(config: Config, default_database: Option<String>) -> Result<Self, String> {
        let tcp = TcpStream::connect(config.get_addr())
            .await
            .map_err(|e| e.to_string())?;
        tcp.set_nodelay(true).map_err(|e| e.to_string())?;

        let client = Client::connect(config, tcp.compat_write())
            .await
            .map_err(|e| e.to_string())?;

        Ok(Self {
            client: Arc::new(Mutex::new(Some(client))),
            default_database,
        })
    }

    /// Converts a Tiberius row cell into a JSON value based on simple type mapping.
    fn row_to_json(&self, row: &tiberius::Row, col_name: &str) -> serde_json::Value {
        // Try common types in order
        if let Ok(Some(s)) = row.try_get::<&str, _>(col_name) {
            return s.into();
        }
        if let Ok(Some(i)) = row.try_get::<i32, _>(col_name) {
            return i.into();
        }
        if let Ok(Some(i)) = row.try_get::<i64, _>(col_name) {
            return i.into();
        }
        if let Ok(Some(f)) = row.try_get::<f64, _>(col_name) {
            return serde_json::Number::from_f64(f)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null);
        }
        if let Ok(Some(b)) = row.try_get::<bool, _>(col_name) {
            return b.into();
        }
        if let Ok(Some(dt)) = row.try_get::<chrono::NaiveDateTime, _>(col_name) {
            return dt.to_string().into();
        }

        // Final check: is it null or is it a genuine decode error?
        match row.try_get::<&str, _>(col_name) {
            Ok(None) => serde_json::Value::Null,
            Err(e) => {
                log::error!("Failed to decode SQL Server column '{}': {}", col_name, e);
                serde_json::Value::String(format!("[Decode Error: {}]", e))
            }
            _ => serde_json::Value::Null,
        }
    }
    /// Fetches primary key column names for a given table using INFORMATION_SCHEMA.
    async fn get_primary_keys_internal(
        &self,
        table: &str,
        schema: Option<String>,
    ) -> Result<Vec<String>, String> {
        let mut client_guard = self.client.lock().await;
        let client = client_guard.as_mut().ok_or("Connection closed")?;

        let schema_name = schema.unwrap_or_else(|| "dbo".to_string());
        let query = "
            SELECT COLUMN_NAME
            FROM INFORMATION_SCHEMA.KEY_COLUMN_USAGE
            WHERE OBJECTPROPERTY(OBJECT_ID(CONSTRAINT_SCHEMA + '.' + CONSTRAINT_NAME), 'IsPrimaryKey') = 1
            AND TABLE_NAME = @p1 AND TABLE_SCHEMA = @p2
        ";

        let mut stream = client
            .query(query, &[&table, &schema_name])
            .await
            .map_err(|e| e.to_string())?;
        let mut pks = Vec::new();
        while let Some(item) = stream.next().await {
            if let QueryItem::Row(row) = item.map_err(|e| e.to_string())? {
                if let Ok(Some(name)) = row.try_get::<&str, _>(0) {
                    pks.push(name.to_string());
                }
            }
        }
        Ok(pks)
    }

    /// Fetches all column names for a given table using INFORMATION_SCHEMA.
    async fn get_columns_internal(
        &self,
        table: &str,
        schema: Option<String>,
    ) -> Result<Vec<String>, String> {
        let mut client_guard = self.client.lock().await;
        let client = client_guard.as_mut().ok_or("Connection closed")?;

        let schema_name = schema.unwrap_or_else(|| "dbo".to_string());
        let query = "
            SELECT COLUMN_NAME
            FROM INFORMATION_SCHEMA.COLUMNS
            WHERE TABLE_NAME = @p1 AND TABLE_SCHEMA = @p2
            ORDER BY ORDINAL_POSITION
        ";

        let mut stream = client
            .query(query, &[&table, &schema_name])
            .await
            .map_err(|e| e.to_string())?;
        let mut cols = Vec::new();
        while let Some(item) = stream.next().await {
            if let QueryItem::Row(row) = item.map_err(|e| e.to_string())? {
                if let Ok(Some(name)) = row.try_get::<&str, _>(0) {
                    cols.push(name.to_string());
                }
            }
        }
        Ok(cols)
    }

    /// Converts a JSON value to a T-SQL literal string for safe embedding in queries.
    fn value_to_sql_literal(val: &serde_json::Value) -> String {
        match val {
            serde_json::Value::Null => "NULL".to_string(),
            serde_json::Value::Bool(b) => if *b { "1".to_string() } else { "0".to_string() },
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::String(s) => format!("N'{}'", s.replace('\'', "''")),
            _ => format!("N'{}'", val.to_string().replace('\'', "''")),
        }
    }

    /// Maps a generic data type name to the closest SQL Server equivalent.
    fn map_type_to_sqlserver(data_type: &str, length: Option<&str>) -> String {
        match data_type.to_uppercase().as_str() {
            "VARCHAR" | "TEXT" | "STRING" | "NVARCHAR" => {
                let len = length.unwrap_or("255");
                if len.eq_ignore_ascii_case("MAX") || len == "-1" {
                    "NVARCHAR(MAX)".to_string()
                } else {
                    format!("NVARCHAR({})", len)
                }
            }
            "CHAR" | "NCHAR" => format!("NCHAR({})", length.unwrap_or("1")),
            "INT" | "INTEGER" => "INT".to_string(),
            "BIGINT" => "BIGINT".to_string(),
            "SMALLINT" => "SMALLINT".to_string(),
            "TINYINT" => "TINYINT".to_string(),
            "DECIMAL" | "NUMERIC" => format!("DECIMAL({})", length.unwrap_or("18,2")),
            "FLOAT" | "DOUBLE" | "REAL" => "FLOAT".to_string(),
            "BOOLEAN" | "BOOL" | "BIT" => "BIT".to_string(),
            "DATE" => "DATE".to_string(),
            "DATETIME" | "TIMESTAMP" | "DATETIME2" => "DATETIME2".to_string(),
            "SMALLDATETIME" => "SMALLDATETIME".to_string(),
            "TIME" => "TIME".to_string(),
            "BINARY" | "BLOB" | "VARBINARY" => "VARBINARY(MAX)".to_string(),
            "UNIQUEIDENTIFIER" => "UNIQUEIDENTIFIER".to_string(),
            "XML" => "XML".to_string(),
            "MONEY" => "MONEY".to_string(),
            _ => data_type.to_string(),
        }
    }

    /// Generates a CREATE TABLE SQL string for SQL Server.
    fn get_create_table_sql(def: &TableDefinition) -> String {
        let schema_name = def.schema.as_deref().unwrap_or("dbo");
        let table = format!("[{}].[{}]", schema_name, def.name);
        let mut col_defs = Vec::new();
        let mut pk_cols = Vec::new();

        for col in &def.columns {
            let type_str = Self::map_type_to_sqlserver(&col.data_type, col.length.as_deref());
            let mut col_def = format!("[{}] {}", col.name, type_str);
            if col.is_auto_increment {
                col_def.push_str(" IDENTITY(1,1)");
            }
            col_def.push_str(if col.is_nullable { " NULL" } else { " NOT NULL" });
            if let Some(default) = &col.default_value {
                col_def.push_str(&format!(" DEFAULT {}", default));
            }
            col_defs.push(col_def);
            if col.is_primary_key {
                pk_cols.push(format!("[{}]", col.name));
            }
        }

        if !pk_cols.is_empty() {
            col_defs.push(format!("PRIMARY KEY ({})", pk_cols.join(", ")));
        }

        format!("CREATE TABLE {} (\n    {}\n)", table, col_defs.join(",\n    "))
    }

    /// Generates a list of ALTER TABLE SQL statements to transform old into new.
    fn build_alter_statements(old: &TableDefinition, new: &TableDefinition) -> Vec<String> {
        let schema_name = new.schema.as_deref().unwrap_or("dbo");
        let table = format!("[{}].[{}]", schema_name, new.name);
        let mut statements = Vec::new();

        let old_cols: HashMap<&str, &TableColumn> =
            old.columns.iter().map(|c| (c.name.as_str(), c)).collect();
        let new_cols: HashMap<&str, &TableColumn> =
            new.columns.iter().map(|c| (c.name.as_str(), c)).collect();

        for old_col in &old.columns {
            if !new_cols.contains_key(old_col.name.as_str()) {
                statements.push(format!(
                    "ALTER TABLE {} DROP COLUMN [{}]",
                    table, old_col.name
                ));
            }
        }

        for new_col in &new.columns {
            if let Some(old_col) = old_cols.get(new_col.name.as_str()) {
                let new_type = Self::map_type_to_sqlserver(&new_col.data_type, new_col.length.as_deref());
                let old_type = Self::map_type_to_sqlserver(&old_col.data_type, old_col.length.as_deref());
                if new_type != old_type || new_col.is_nullable != old_col.is_nullable {
                    let nullable = if new_col.is_nullable { "NULL" } else { "NOT NULL" };
                    statements.push(format!(
                        "ALTER TABLE {} ALTER COLUMN [{}] {} {}",
                        table, new_col.name, new_type, nullable
                    ));
                }
            } else {
                let type_str = Self::map_type_to_sqlserver(&new_col.data_type, new_col.length.as_deref());
                let nullable = if new_col.is_nullable { "NULL" } else { "NOT NULL" };
                let mut add_stmt = format!(
                    "ALTER TABLE {} ADD [{}] {} {}",
                    table, new_col.name, type_str, nullable
                );
                if let Some(default) = &new_col.default_value {
                    add_stmt.push_str(&format!(" DEFAULT {}", default));
                }
                statements.push(add_stmt);
            }
        }

        // Handle PK changes
        let old_pks: Vec<&str> = old.columns.iter().filter(|c| c.is_primary_key).map(|c| c.name.as_str()).collect();
        let new_pks: Vec<&str> = new.columns.iter().filter(|c| c.is_primary_key).map(|c| c.name.as_str()).collect();
        if old_pks != new_pks {
            if !old_pks.is_empty() {
                // Drop existing PK constraint by looking up its name dynamically
                statements.push(format!(
                    "DECLARE @pk_{0} NVARCHAR(256) = (SELECT TOP 1 name FROM sys.key_constraints WHERE type = 'PK' AND parent_object_id = OBJECT_ID('[{1}].[{0}]')); IF @pk_{0} IS NOT NULL EXEC(N'ALTER TABLE {2} DROP CONSTRAINT [' + @pk_{0} + N']')",
                    new.name, schema_name, table
                ));
            }
            if !new_pks.is_empty() {
                let pk_str = new_pks.iter().map(|c| format!("[{}]", c)).collect::<Vec<_>>().join(", ");
                statements.push(format!("ALTER TABLE {} ADD PRIMARY KEY ({})", table, pk_str));
            }
        }

        statements
    }
}

#[async_trait]
impl Database for SqlServerDatabase {
    /// Executes a SQL query. For SELECTs, it returns rows and metadata.
    /// For other statements, it returns affected rows.
    /// Handles database context switching using USE [db].
    async fn execute_query(
        &self,
        query: &str,
        table_name: Option<String>,
        catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<QueryResult, String> {
        let info = utils::parse_query(query, table_name);
        let start = std::time::Instant::now();

        let rs_final = {
            let (columns, col_types, rs) = {
                let mut client_guard = self.client.lock().await;
                let client = client_guard.as_mut().ok_or("Connection closed")?;

                // Handle database context switching for SQL Server
                let effective_db = utils::get_effective_context(
                    catalog.clone(),
                    schema.clone(),
                    self.default_database.clone(),
                );

                if let Some(db) = effective_db {
                    if !info.q_trimmed.starts_with("USE ") {
                        let use_query = format!("USE [{}];", db);
                        client.execute(&use_query, &[]).await.map_err(|e| {
                            format!("Failed to switch database context to `{}`: {}", db, e)
                        })?;
                    }
                }

                let mut stream = client.query(query, &[]).await.map_err(|e| e.to_string())?;

                let mut cols: Vec<String> = Vec::new();
                let mut types: Vec<String> = Vec::new();
                let mut rs = Vec::new();

                while let Some(item) = stream.next().await {
                    match item.map_err(|e| e.to_string())? {
                        QueryItem::Metadata(meta) => {
                            if cols.is_empty() {
                                cols = meta
                                    .columns()
                                    .iter()
                                    .map(|c| c.name().to_string())
                                    .collect();
                                types = meta
                                    .columns()
                                    .iter()
                                    .map(|c| format!("{:?}", c.column_type()))
                                    .collect();
                            }
                        }
                        QueryItem::Row(row) => {
                            let mut map = serde_json::Map::new();
                            for col_name in &cols {
                                let val = self.row_to_json(&row, col_name);
                                map.insert(col_name.to_string(), val);
                            }
                            rs.push(serde_json::Value::Object(map));
                        }
                    }
                }
                (cols, types, rs)
            };

            let pks = if let Some(table) = &info.detected_table_name {
                self.get_primary_keys_internal(table, schema.clone())
                    .await
                    .unwrap_or_default()
            } else {
                vec![]
            };

            let fallback = if rs.is_empty() {
                if let Some(table) = &info.detected_table_name {
                    self.get_columns_internal(table, schema.clone())
                        .await
                        .unwrap_or_default()
                } else {
                    vec![]
                }
            } else {
                vec![]
            };

            let final_cols = if columns.is_empty() {
                fallback
            } else {
                columns
            };
            let final_types = if col_types.is_empty() {
                vec!["TEXT".to_string(); final_cols.len()]
            } else {
                col_types
            };

            QueryResult {
                columns: final_cols,
                column_types: final_types,
                rows: rs,
                affected_rows: 0,
                execution_time_ms: start.elapsed().as_millis(),
                primary_keys: pks,
                table_name: info.detected_table_name,
            }
        };

        Ok(rs_final)
    }

    /// Returns a list of all tables in the current database.
    async fn get_table_list(
        &self,
        _catalog: Option<String>,
        _schema: Option<String>,
    ) -> Result<QueryResult, String> {
        let query = "
            SELECT 
                TABLE_NAME as Name, 
                TABLE_SCHEMA as [Schema],
                TABLE_TYPE as Type
            FROM INFORMATION_SCHEMA.TABLES 
            ORDER BY TABLE_NAME
        ";
        self.execute_query(query, None, None, None).await
    }

    /// Returns a list of all routines (stored procedures/functions) in the database.
    async fn get_routine_list(
        &self,
        _catalog: Option<String>,
        _schema: Option<String>,
    ) -> Result<QueryResult, String> {
        let query = "
            SELECT 
                ROUTINE_NAME as Name,
                ROUTINE_TYPE as Type,
                DATA_TYPE as [Return Type],
                ROUTINE_SCHEMA as [Schema],
                CREATED as Created,
                LAST_ALTERED as Modified
            FROM INFORMATION_SCHEMA.ROUTINES
            ORDER BY ROUTINE_NAME
        ";
        self.execute_query(query, None, None, None).await
    }

    /// Returns a comprehensive list of all schema objects for autocomplete/sidebar.
    async fn get_objects(&self) -> Result<Vec<DbObject>, String> {
        let table_query = "
            SELECT 
                TABLE_CATALOG as catalog,
                TABLE_SCHEMA as [schema],
                TABLE_NAME as name,
                CASE WHEN TABLE_TYPE = 'BASE TABLE' THEN 'table' ELSE 'view' END as object_type,
                (SELECT TOP 1 CAST(value AS NVARCHAR(MAX)) FROM sys.extended_properties WHERE major_id = OBJECT_ID(QUOTENAME(TABLE_SCHEMA) + '.' + QUOTENAME(TABLE_NAME)) AND minor_id = 0 AND name = 'MS_Description') as description
            FROM INFORMATION_SCHEMA.TABLES
        ";
        let routine_query = "
            SELECT 
                ROUTINE_CATALOG as catalog,
                ROUTINE_SCHEMA as [schema],
                ROUTINE_NAME as name,
                LOWER(ROUTINE_TYPE) as object_type,
                (SELECT TOP 1 CAST(value AS NVARCHAR(MAX)) FROM sys.extended_properties WHERE major_id = OBJECT_ID(QUOTENAME(ROUTINE_SCHEMA) + '.' + QUOTENAME(ROUTINE_NAME)) AND minor_id = 0 AND name = 'MS_Description') as description
            FROM INFORMATION_SCHEMA.ROUTINES
        ";
        let column_query = "
            SELECT TABLE_SCHEMA as table_schema, TABLE_NAME as table_name, COLUMN_NAME as column_name, DATA_TYPE as data_type
            FROM INFORMATION_SCHEMA.COLUMNS
            ORDER BY TABLE_SCHEMA, TABLE_NAME, ORDINAL_POSITION
        ";

        let table_res = self.execute_query(table_query, None, None, None).await?;
        let routine_res = self.execute_query(routine_query, None, None, None).await?;
        let column_res = self.execute_query(column_query, None, None, None).await?;

        let mut column_map: HashMap<(String, String), Vec<ColumnInfo>> = HashMap::new();
        for row in column_res.rows {
            if let Some(obj) = row.as_object() {
                let schema = obj
                    .get("table_schema")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let table = obj
                    .get("table_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let name = obj
                    .get("column_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let data_type = obj
                    .get("data_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                column_map
                    .entry((schema, table))
                    .or_default()
                    .push(ColumnInfo { name, data_type });
            }
        }

        let mut objects = Vec::new();
        for row in table_res.rows {
            if let Some(map) = row.as_object() {
                let name = map
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let catalog = map
                    .get("catalog")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string());
                let schema = map
                    .get("schema")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string());
                objects.push(DbObject {
                    name: name.clone(),
                    object_type: map
                        .get("object_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    schema: schema.clone(),
                    catalog: catalog,
                    description: map
                        .get("description")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    parent: None,
                    columns: column_map.remove(&(schema.unwrap_or_default(), name)),
                });
            }
        }

        for row in routine_res.rows {
            if let Some(map) = row.as_object() {
                objects.push(DbObject {
                    name: map
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    object_type: map
                        .get("object_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    schema: map
                        .get("schema")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    catalog: map
                        .get("catalog")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    description: map
                        .get("description")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    parent: None,
                    columns: None,
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
        catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<RowActionResult, String> {
        let schema_name = schema.as_deref().unwrap_or("dbo");
        let full_table = if let Some(cat) = &catalog {
            format!("[{}].[{}].[{}]", cat, schema_name, table_name)
        } else {
            format!("[{}].[{}]", schema_name, table_name)
        };

        let val_literal = Self::value_to_sql_literal(&value);
        let mut pk_list: Vec<(String, serde_json::Value)> = pks.into_iter().collect();
        pk_list.sort_by(|a, b| a.0.cmp(&b.0));
        let where_clause = pk_list
            .iter()
            .map(|(k, v)| format!("[{}] = {}", k, Self::value_to_sql_literal(v)))
            .collect::<Vec<_>>()
            .join(" AND ");

        let query = format!(
            "UPDATE {} SET [{}] = {} WHERE {}",
            full_table, column, val_literal, where_clause
        );

        let mut client_guard = self.client.lock().await;
        let client = client_guard.as_mut().ok_or("Connection closed")?;
        let res = client.execute(&query, &[]).await.map_err(|e| e.to_string())?;

        Ok(RowActionResult {
            affected_rows: res.total(),
            query,
        })
    }

    async fn insert_row(
        &self,
        table_name: &str,
        data: HashMap<String, serde_json::Value>,
        catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<RowActionResult, String> {
        let schema_name = schema.as_deref().unwrap_or("dbo");
        let full_table = if let Some(cat) = &catalog {
            format!("[{}].[{}].[{}]", cat, schema_name, table_name)
        } else {
            format!("[{}].[{}]", schema_name, table_name)
        };

        let mut cols: Vec<String> = data.keys().cloned().collect();
        cols.sort();
        let col_list = cols.iter().map(|c| format!("[{}]", c)).collect::<Vec<_>>().join(", ");
        let val_list = cols
            .iter()
            .map(|c| Self::value_to_sql_literal(&data[c]))
            .collect::<Vec<_>>()
            .join(", ");

        let query = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            full_table, col_list, val_list
        );

        let mut client_guard = self.client.lock().await;
        let client = client_guard.as_mut().ok_or("Connection closed")?;
        let res = client.execute(&query, &[]).await.map_err(|e| e.to_string())?;

        Ok(RowActionResult {
            affected_rows: res.total(),
            query,
        })
    }

    /// Fetches the full structure of a table using sys.columns for accurate metadata.
    async fn get_table_definition(
        &self,
        table_name: &str,
        catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<TableDefinition, String> {
        let schema_name = schema.clone().unwrap_or_else(|| "dbo".to_string());
        let full_name = format!("[{}].[{}]", schema_name, table_name);

        let col_query = format!(
            "SELECT
                c.name AS column_name,
                t.name AS data_type,
                c.is_nullable,
                c.is_identity,
                CASE WHEN t.name IN ('nvarchar','nchar') THEN c.max_length / 2
                     WHEN c.max_length = -1 THEN -1
                     ELSE c.max_length END AS char_length,
                c.precision,
                c.scale,
                dc.definition AS default_value
            FROM sys.columns c
            JOIN sys.types t ON c.user_type_id = t.user_type_id
            LEFT JOIN sys.default_constraints dc ON c.default_object_id = dc.object_id
            WHERE c.object_id = OBJECT_ID('{}')
            ORDER BY c.column_id",
            full_name
        );

        let pk_query = format!(
            "SELECT c.name
            FROM sys.key_constraints k
            JOIN sys.index_columns ic ON k.parent_object_id = ic.object_id AND k.unique_index_id = ic.index_id
            JOIN sys.columns c ON ic.object_id = c.object_id AND ic.column_id = c.column_id
            WHERE k.type = 'PK' AND k.parent_object_id = OBJECT_ID('{}')",
            full_name
        );

        let col_res = self.execute_query(&col_query, None, None, None).await?;
        let pk_res = self.execute_query(&pk_query, None, None, None).await?;

        let pk_set: HashSet<String> = pk_res
            .rows
            .iter()
            .filter_map(|r| r.get("name").and_then(|v| v.as_str()).map(|s| s.to_string()))
            .collect();

        let mut columns = Vec::new();
        for row in col_res.rows {
            if let serde_json::Value::Object(map) = row {
                let col_name = map.get("column_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let data_type = map.get("data_type").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let is_nullable = map.get("is_nullable").and_then(|v| v.as_bool())
                    .or_else(|| map.get("is_nullable").and_then(|v| v.as_i64()).map(|i| i != 0))
                    .unwrap_or(true);
                let is_identity = map.get("is_identity").and_then(|v| v.as_bool())
                    .or_else(|| map.get("is_identity").and_then(|v| v.as_i64()).map(|i| i != 0))
                    .unwrap_or(false);
                let char_length = map.get("char_length").and_then(|v| v.as_i64());
                let precision = map.get("precision").and_then(|v| v.as_i64());
                let scale = map.get("scale").and_then(|v| v.as_i64());
                let default_value = map.get("default_value").and_then(|v| v.as_str()).map(|s| s.to_string());

                let length = match data_type.to_lowercase().as_str() {
                    "nvarchar" | "varchar" | "nchar" | "char" | "varbinary" | "binary" => {
                        char_length.map(|l| if l == -1 { "MAX".to_string() } else { l.to_string() })
                    }
                    "decimal" | "numeric" => {
                        precision.map(|p| {
                            if let Some(s) = scale { if s > 0 { format!("{},{}", p, s) } else { p.to_string() } }
                            else { p.to_string() }
                        })
                    }
                    _ => None,
                };

                columns.push(TableColumn {
                    is_primary_key: pk_set.contains(&col_name),
                    is_auto_increment: is_identity,
                    name: col_name,
                    data_type,
                    is_nullable,
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
            catalog,
            schema: Some(schema_name),
            comment: None,
            collation: None,
        })
    }

    async fn create_table(&self, definition: TableDefinition) -> Result<(), String> {
        let sql = Self::get_create_table_sql(&definition);
        let mut client_guard = self.client.lock().await;
        let client = client_guard.as_mut().ok_or("Connection closed")?;
        client.execute(&sql, &[]).await.map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn alter_table(&self, old: TableDefinition, new: TableDefinition) -> Result<(), String> {
        let statements = Self::build_alter_statements(&old, &new);
        if statements.is_empty() {
            return Ok(());
        }
        let mut client_guard = self.client.lock().await;
        let client = client_guard.as_mut().ok_or("Connection closed")?;
        for stmt in statements {
            client.execute(&stmt, &[]).await.map_err(|e| format!("Failed: {}\nError: {}", stmt, e))?;
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

    /// Fetches the source definition for a routine using OBJECT_DEFINITION.
    async fn get_routine_definition(
        &self,
        name: &str,
        _routine_type: &str,
        _catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<RoutineDefinition, String> {
        let s = schema.unwrap_or("dbo".to_string());
        let query = format!(
            "SELECT OBJECT_DEFINITION(OBJECT_ID('{}.{}')) as Definition",
            s.clone(),
            name
        );
        let res = self
            .execute_query(&query, None, None, Some(s.clone()))
            .await?;
        let def = if let Some(row) = res.rows.first() {
            row.get("Definition")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        } else {
            "".to_string()
        };

        Ok(RoutineDefinition {
            name: name.to_string(),
            routine_type: "PROCEDURE".to_string(), // Simplified
            definition: def,
            catalog: None,
            schema: Some(s),
        })
    }

    async fn save_routine(&self, definition: RoutineDefinition) -> Result<(), String> {
        // Normalize to CREATE OR ALTER (SQL Server 2016+)
        let def_sql = definition.definition.trim();
        let upper = def_sql.to_uppercase();
        let normalized = if upper.starts_with("CREATE OR ALTER") {
            def_sql.to_string()
        } else if upper.starts_with("CREATE") {
            format!("CREATE OR ALTER{}", &def_sql["CREATE".len()..])
        } else {
            let schema_name = definition.schema.as_deref().unwrap_or("dbo");
            let rt = definition.routine_type.to_uppercase();
            format!(
                "CREATE OR ALTER {} [{}].[{}] AS\n{}",
                rt, schema_name, definition.name, def_sql
            )
        };
        let mut client_guard = self.client.lock().await;
        let client = client_guard.as_mut().ok_or("Connection closed")?;
        client.execute(&normalized, &[]).await.map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Fetches the source definition for a view using OBJECT_DEFINITION.
    async fn get_view_definition(
        &self,
        name: &str,
        _catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<ViewDefinition, String> {
        let s = schema.unwrap_or("dbo".to_string());
        let query = format!(
            "SELECT OBJECT_DEFINITION(OBJECT_ID('{}.{}')) as Definition",
            s.clone(),
            name
        );
        let res = self
            .execute_query(&query, None, None, Some(s.clone()))
            .await?;
        let def = if let Some(row) = res.rows.first() {
            row.get("Definition")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        } else {
            "".to_string()
        };

        Ok(ViewDefinition {
            name: name.to_string(),
            definition: def,
            catalog: None,
            schema: Some(s),
        })
    }

    async fn save_view(&self, definition: ViewDefinition) -> Result<(), String> {
        let def_sql = definition.definition.trim();
        let upper = def_sql.to_uppercase();
        let normalized = if upper.starts_with("CREATE OR ALTER") {
            def_sql.to_string()
        } else if upper.starts_with("CREATE") {
            format!("CREATE OR ALTER{}", &def_sql["CREATE".len()..])
        } else {
            let schema_name = definition.schema.as_deref().unwrap_or("dbo");
            format!(
                "CREATE OR ALTER VIEW [{}].[{}] AS\n{}",
                schema_name, definition.name, def_sql
            )
        };
        let mut client_guard = self.client.lock().await;
        let client = client_guard.as_mut().ok_or("Connection closed")?;
        client.execute(&normalized, &[]).await.map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn close(&self) {
        let mut client_guard = self.client.lock().await;
        if let Some(client) = client_guard.take() {
            let _ = client.close().await;
        }
    }
}
