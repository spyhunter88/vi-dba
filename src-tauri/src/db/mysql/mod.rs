mod legacy;
mod v8;
mod v84;
mod version;

use super::{utils, Database};
use crate::models::{
    ColumnInfo, DbObject, QueryResult, RoutineDefinition, RowActionResult, TableColumn,
    TableDefinition, ViewDefinition,
};
pub use version::{MySqlVersion, MySqlVersionGroup};

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use sqlx::{Column, Executor, MySqlPool, Row, TypeInfo};
use std::collections::HashMap;
use std::time::Instant;

/// MySQL implementation of the Database trait.
pub struct MySqlDatabase {
    pub pool: MySqlPool,
    pub default_database: Option<String>,
    pub version: MySqlVersion,
}

impl MySqlDatabase {
    /// Creates a new MySQL database instance with version detection.
    pub async fn new(pool: MySqlPool, default_database: Option<String>) -> Self {
        let version_str = match sqlx::query("SELECT VERSION() as v").fetch_one(&pool).await {
            Ok(row) => row.try_get("v").unwrap_or_else(|_| "0.0.0".to_string()),
            Err(_) => "0.0.0".to_string(),
        };

        // Fallback: detect current database if not provided
        let mut actual_default = default_database;
        if actual_default.is_none() || actual_default.as_deref() == Some("") {
            if let Ok(row) = sqlx::query("SELECT DATABASE() as d").fetch_one(&pool).await {
                actual_default = row.try_get::<Option<String>, _>("d").ok().flatten();
            }
        }

        Self {
            pool,
            default_database: actual_default,
            version: MySqlVersion::parse(&version_str),
        }
    }

    /// Generates the SQL string needed to create a table from a definition.
    fn get_create_table_sql(&self, definition: &TableDefinition) -> String {
        let mut sql = format!("CREATE TABLE `{}` (", definition.name);
        let mut col_defs = Vec::new();
        let mut pks = Vec::new();

        for col in &definition.columns {
            let mut def = format!("`{}` {}", col.name, col.data_type);

            if let Some(len) = &col.length {
                if !len.is_empty() && len != "0" {
                    def.push_str(&format!("({})", len));
                }
            }

            if !col.is_nullable {
                def.push_str(" NOT NULL");
            }

            if col.is_auto_increment {
                def.push_str(" AUTO_INCREMENT");
            }

            if let Some(default) = &col.default_value {
                if !default.is_empty() {
                    def.push_str(&format!(" DEFAULT {}", default));
                }
            }

            if let Some(comment) = &col.comment {
                if !comment.is_empty() {
                    def.push_str(&format!(" COMMENT '{}'", comment.replace('\'', "''")));
                }
            }

            col_defs.push(def);

            if col.is_primary_key {
                pks.push(col.name.clone());
            }
        }

        if !pks.is_empty() {
            let pk_sql = format!(
                "PRIMARY KEY ({})",
                pks.iter()
                    .map(|k| format!("`{}`", k))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            col_defs.push(pk_sql);
        }

        sql.push_str(&col_defs.join(", "));
        sql.push(')');
        sql
    }

    /// Compares two table definitions and generates a list of ALTER statements.
    fn get_alter_table_sql(&self, old: &TableDefinition, new: &TableDefinition) -> Vec<String> {
        let mut statements = Vec::new();

        // 1. Check for table rename
        if old.name != new.name {
            statements.push(format!("RENAME TABLE `{}` TO `{}`", old.name, new.name));
        }

        let table_name = &new.name;

        // 2. Identify column changes
        let old_cols: HashMap<String, &TableColumn> =
            old.columns.iter().map(|c| (c.name.clone(), c)).collect();
        let new_cols: HashMap<String, &TableColumn> =
            new.columns.iter().map(|c| (c.name.clone(), c)).collect();

        // Drops
        for (name, _) in &old_cols {
            if !new_cols.contains_key(name) {
                statements.push(format!(
                    "ALTER TABLE `{}` DROP COLUMN `{}`",
                    table_name, name
                ));
            }
        }

        // Additions and Modifications
        for col in &new.columns {
            if let Some(old_col) = old_cols.get(&col.name) {
                // Check if anything changed
                if old_col.data_type != col.data_type
                    || old_col.length != col.length
                    || old_col.is_nullable != col.is_nullable
                    || old_col.is_auto_increment != col.is_auto_increment
                    || old_col.default_value != col.default_value
                    || old_col.comment != col.comment
                {
                    let mut def = format!("`{}` {}", col.name, col.data_type);
                    if let Some(len) = &col.length {
                        if !len.is_empty() && len != "0" {
                            def.push_str(&format!("({})", len));
                        }
                    }
                    if !col.is_nullable {
                        def.push_str(" NOT NULL");
                    }
                    if col.is_auto_increment {
                        def.push_str(" AUTO_INCREMENT");
                    }
                    if let Some(default) = &col.default_value {
                        if !default.is_empty() {
                            def.push_str(&format!(" DEFAULT {}", default));
                        }
                    }
                    if let Some(comment) = &col.comment {
                        if !comment.is_empty() {
                            def.push_str(&format!(" COMMENT '{}'", comment.replace('\'', "''")));
                        }
                    }

                    statements.push(format!(
                        "ALTER TABLE `{}` MODIFY COLUMN {}",
                        table_name, def
                    ));
                }
            } else {
                // New column
                let mut def = format!("`{}` {}", col.name, col.data_type);
                if let Some(len) = &col.length {
                    if !len.is_empty() && len != "0" {
                        def.push_str(&format!("({})", len));
                    }
                }
                if !col.is_nullable {
                    def.push_str(" NOT NULL");
                }
                if col.is_auto_increment {
                    def.push_str(" AUTO_INCREMENT");
                }
                if let Some(default) = &col.default_value {
                    if !default.is_empty() {
                        def.push_str(&format!(" DEFAULT {}", default));
                    }
                }
                if let Some(comment) = &col.comment {
                    if !comment.is_empty() {
                        def.push_str(&format!(" COMMENT '{}'", comment.replace('\'', "''")));
                    }
                }
                statements.push(format!("ALTER TABLE `{}` ADD COLUMN {}", table_name, def));
            }
        }

        // 3. Primary Key changes
        let old_pks: Vec<String> = old
            .columns
            .iter()
            .filter(|c| c.is_primary_key)
            .map(|c| c.name.clone())
            .collect();
        let new_pks: Vec<String> = new
            .columns
            .iter()
            .filter(|c| c.is_primary_key)
            .map(|c| c.name.clone())
            .collect();

        if old_pks != new_pks {
            if !old_pks.is_empty() {
                statements.push(format!("ALTER TABLE `{}` DROP PRIMARY KEY", table_name));
            }
            if !new_pks.is_empty() {
                let pks_str = new_pks
                    .iter()
                    .map(|k| format!("`{}`", k))
                    .collect::<Vec<_>>()
                    .join(", ");
                statements.push(format!(
                    "ALTER TABLE `{}` ADD PRIMARY KEY ({})",
                    table_name, pks_str
                ));
            }
        }

        statements
    }

    /// Converts a MySQL row value into a JSON value based on simple type mapping.
    ///
    /// Uses the column's ordinal index (not name) for all lookups. sqlx-mysql's prepared
    /// statement path (sqlx::query → binary protocol) caches a `column_names` HashMap from
    /// PREPARE-time metadata; for `EXPLAIN` in MySQL 8.4+ the server returns no/wrong column
    /// metadata at prepare time, so name lookups would panic with ColumnNotFound. The
    /// per-row `columns` vec is refreshed at execute time, so index-based access is correct.
    fn row_to_json(&self, row: &sqlx::mysql::MySqlRow, col: usize) -> serde_json::Value {
        let col_info = row.column(col);
        let type_info = col_info.type_info();
        let type_name = type_info.name();
        let normalized = type_name.to_uppercase();

        if normalized == "BOOLEAN" {
            if let Ok(v) = row.try_get::<Option<bool>, _>(col) {
                return v.map(|n| n.into()).unwrap_or(serde_json::Value::Null);
            }
            if let Ok(v) = row.try_get::<Option<i8>, _>(col) {
                return v.map(|n| n.into()).unwrap_or(serde_json::Value::Null);
            }
        } else if normalized.starts_with("TINYINT") {
            if let Ok(v) = row.try_get::<Option<i8>, _>(col) {
                return v.map(|n| n.into()).unwrap_or(serde_json::Value::Null);
            }
            if let Ok(v) = row.try_get::<Option<u8>, _>(col) {
                return v.map(|n| n.into()).unwrap_or(serde_json::Value::Null);
            }
        } else if normalized.starts_with("SMALLINT") {
            if let Ok(v) = row.try_get::<Option<i16>, _>(col) {
                return v.map(|n| n.into()).unwrap_or(serde_json::Value::Null);
            }
            if let Ok(v) = row.try_get::<Option<u16>, _>(col) {
                return v.map(|n| n.into()).unwrap_or(serde_json::Value::Null);
            }
        } else if normalized.starts_with("BIGINT") {
            if let Ok(v) = row.try_get::<Option<i64>, _>(col) {
                return v.map(|n| n.into()).unwrap_or(serde_json::Value::Null);
            }
            if let Ok(v) = row.try_get::<Option<u64>, _>(col) {
                return v.map(|n| n.into()).unwrap_or(serde_json::Value::Null);
            }
        } else if normalized.starts_with("INT") || normalized.starts_with("MEDIUMINT") {
            if let Ok(v) = row.try_get::<Option<i32>, _>(col) {
                return v.map(|n| n.into()).unwrap_or(serde_json::Value::Null);
            }
            if let Ok(v) = row.try_get::<Option<u32>, _>(col) {
                return v.map(|n| n.into()).unwrap_or(serde_json::Value::Null);
            }
            if let Ok(v) = row.try_get::<Option<i64>, _>(col) {
                return v.map(|n| n.into()).unwrap_or(serde_json::Value::Null);
            }
        } else if normalized == "FLOAT" {
            if let Ok(v) = row.try_get::<Option<f32>, _>(col) {
                return v
                    .and_then(|n| {
                        // Round-trip through f32's Display so the f64 we hand to
                        // serde_json reflects f32's ~7-digit precision. A direct
                        // `n as f64` exposes the binary approximation, e.g.
                        // 0.0082_f32 → 0.008200000040233135_f64.
                        format!("{}", n)
                            .parse::<f64>()
                            .ok()
                            .and_then(|f| serde_json::Number::from_f64(f).map(serde_json::Value::Number))
                    })
                    .unwrap_or(serde_json::Value::Null);
            }
        } else if normalized == "DOUBLE" {
            if let Ok(v) = row.try_get::<Option<f64>, _>(col) {
                return v
                    .and_then(|n| serde_json::Number::from_f64(n).map(serde_json::Value::Number))
                    .unwrap_or(serde_json::Value::Null);
            }
        } else if normalized == "DECIMAL" {
            if let Ok(v) = row.try_get::<Option<rust_decimal::Decimal>, _>(col) {
                return v
                    .map(|n| serde_json::Value::String(n.to_string()))
                    .unwrap_or(serde_json::Value::Null);
            }
        } else if normalized == "TIMESTAMP" || normalized == "DATETIME" {
            if let Ok(v) = row.try_get::<Option<DateTime<Utc>>, _>(col) {
                return v
                    .map(|n| n.to_rfc3339().into())
                    .unwrap_or(serde_json::Value::Null);
            }
            if let Ok(v) = row.try_get::<Option<NaiveDateTime>, _>(col) {
                return v
                    .map(|n| n.format("%Y-%m-%d %H:%M:%S").to_string().into())
                    .unwrap_or(serde_json::Value::Null);
            }
        } else if normalized == "DATE" {
            if let Ok(v) = row.try_get::<Option<NaiveDate>, _>(col) {
                return v
                    .map(|n| n.to_string().into())
                    .unwrap_or(serde_json::Value::Null);
            }
        } else if normalized == "TIME" {
            if let Ok(v) = row.try_get::<Option<NaiveTime>, _>(col) {
                return v
                    .map(|n| n.to_string().into())
                    .unwrap_or(serde_json::Value::Null);
            }
        } else if normalized == "YEAR" {
            if let Ok(v) = row.try_get::<Option<i32>, _>(col) {
                return v.map(|n| n.into()).unwrap_or(serde_json::Value::Null);
            }
        } else if normalized == "BIT" {
            if let Ok(v) = row.try_get::<Option<u64>, _>(col) {
                return v.map(|n| n.into()).unwrap_or(serde_json::Value::Null);
            }
            if let Ok(v) = row.try_get::<Option<bool>, _>(col) {
                return v.map(|n| n.into()).unwrap_or(serde_json::Value::Null);
            }
        } else if normalized == "JSON" {
            if let Ok(v) = row.try_get::<Option<serde_json::Value>, _>(col) {
                return v.unwrap_or(serde_json::Value::Null);
            }
        } else if normalized == "ENUM" || normalized == "SET" {
            if let Ok(v) = row.try_get::<Option<String>, _>(col) {
                return v.map(|s| s.into()).unwrap_or(serde_json::Value::Null);
            }
        } else if normalized.contains("BLOB") || normalized.contains("BINARY") {
            if let Ok(v) = row.try_get::<Option<Vec<u8>>, _>(col) {
                return v
                    .map(|bytes| {
                        // Try to decode as UTF-8 first (useful for MySQL 8.4 metadata)
                        if let Ok(s) = String::from_utf8(bytes.clone()) {
                            s.into()
                        } else {
                            // Fallback to hex for actual binary data
                            let hex_val: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
                            format!("0x{}", hex_val).into()
                        }
                    })
                    .unwrap_or(serde_json::Value::Null);
            }
        }

        if let Ok(v) = row.try_get::<Option<String>, _>(col) {
            return v.map(|s| s.into()).unwrap_or(serde_json::Value::Null);
        } else if let Ok(v) = row.try_get::<Option<Vec<u8>>, _>(col) {
            // Fallback for binary-flagged strings (common in MySQL 8.4 metadata)
            return v
                .map(|b| String::from_utf8_lossy(&b).to_string().into())
                .unwrap_or(serde_json::Value::Null);
        }

        serde_json::Value::Null
    }

    /// Processes a list of MySqlRows into a QueryResult.
    fn process_rows(
        &self,
        rows: Vec<sqlx::mysql::MySqlRow>,
        time_ms: u128,
        pks: Vec<String>,
        table_name: Option<String>,
        fallback_cols: Vec<String>,
    ) -> Result<QueryResult, String> {
        if rows.is_empty() {
            return Ok(QueryResult {
                columns: fallback_cols.clone(),
                column_types: vec!["TEXT".to_string(); fallback_cols.len()],
                rows: vec![],
                affected_rows: 0,
                execution_time_ms: time_ms,
                primary_keys: pks,
                table_name,
            });
        }
        let columns: Vec<String> = rows[0]
            .columns()
            .iter()
            .map(|c| c.name().to_string())
            .collect();
        let column_types: Vec<String> = rows[0]
            .columns()
            .iter()
            .map(|c| c.type_info().to_string())
            .collect();
        let mut result_rows = Vec::new();
        for row in rows {
            let mut map = serde_json::Map::new();
            for col in row.columns() {
                let name = col.name();
                let val = self.row_to_json(&row, col.ordinal());
                map.insert(name.to_string(), val);
            }
            result_rows.push(serde_json::Value::Object(map));
        }
        Ok(QueryResult {
            columns,
            column_types,
            rows: result_rows,
            affected_rows: 0,
            execution_time_ms: time_ms,
            primary_keys: pks,
            table_name,
        })
    }

    /// Fetches the primary key column names for a given table.
    async fn get_primary_keys(
        &self,
        table: &str,
        catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<Vec<String>, String> {
        let db_name = catalog.or(schema);
        let query = if let Some(db) = db_name {
            format!(
                "SHOW KEYS FROM `{}`.`{}` WHERE Key_name = 'PRIMARY'",
                db, table
            )
        } else {
            format!("SHOW KEYS FROM `{}` WHERE Key_name = 'PRIMARY'", table)
        };
        let rows = sqlx::query(&query)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(rows
            .into_iter()
            .map(|r| r.get::<String, _>("Column_name"))
            .collect())
    }

    /// Fetches all column names for a given table using DESCRIBE.
    async fn get_columns_internal(
        &self,
        table: &str,
        catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<Vec<String>, String> {
        let db_name = catalog.or(schema);
        let query = if let Some(db) = db_name {
            format!("DESCRIBE `{}`.`{}`", db, table)
        } else {
            format!("DESCRIBE `{}`", table)
        };
        let rows = sqlx::query(&query)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(rows
            .into_iter()
            .map(|r| r.get::<String, _>("Field"))
            .collect())
    }

    fn val_to_opt_string(&self, val: &serde_json::Value) -> Option<String> {
        match val {
            serde_json::Value::Null => None,
            serde_json::Value::String(s) => Some(s.clone()),
            _ => Some(val.to_string().trim_matches('"').to_string()),
        }
    }

    /// Switches the active database on the given connection to the effective catalog/schema.
    /// No-op when no context can be resolved.
    async fn switch_context(
        &self,
        conn: &mut sqlx::MySqlConnection,
        catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<(), String> {
        let effective_catalog =
            utils::get_effective_context(catalog, schema, self.default_database.clone());
        if let Some(cat) = effective_catalog {
            let use_query = format!("USE `{}`;", cat);
            conn.execute(sqlx::raw_sql(&use_query))
                .await
                .map_err(|e| format!("Failed to switch database context to `{}`: {}", cat, e))?;
        }
        Ok(())
    }

    /// Runs a single statement on an already-acquired connection. Shared by `execute_query`
    /// (one statement per pooled connection) and `execute_script` (many statements on one
    /// connection). Context switching is handled by the caller.
    async fn exec_stmt(
        &self,
        conn: &mut sqlx::MySqlConnection,
        query: &str,
        table_name: Option<String>,
        catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<QueryResult, String> {
        let start = Instant::now();
        let info = utils::parse_query(query, table_name);

        let pks = if let Some(table) = &info.detected_table_name {
            self.get_primary_keys(table, catalog.clone(), schema.clone())
                .await
                .unwrap_or_default()
        } else {
            vec![]
        };

        if info.is_select {
            let res = sqlx::query(query).fetch_all(&mut *conn).await;
            if let Err(e) = res {
                let err_msg = e.to_string();
                if err_msg.contains("1046")
                    || err_msg.to_lowercase().contains("no database selected")
                {
                    return Err("No database selected. Please specify a database in your connection settings or run a 'USE database_name;' statement first.".to_string());
                }
                return Err(err_msg);
            }
            let rows = res.unwrap();
            let fallback = if rows.is_empty() {
                if let Some(table) = &info.detected_table_name {
                    self.get_columns_internal(table, catalog.clone(), schema.clone())
                        .await
                        .unwrap_or_default()
                } else {
                    vec![]
                }
            } else {
                vec![]
            };

            self.process_rows(
                rows,
                start.elapsed().as_millis(),
                pks,
                info.detected_table_name,
                fallback,
            )
        } else {
            let res = sqlx::query(query)
                .execute(&mut *conn)
                .await
                .map_err(|e| e.to_string())?;
            Ok(QueryResult {
                columns: vec![],
                column_types: vec![],
                rows: vec![],
                affected_rows: res.rows_affected(),
                execution_time_ms: start.elapsed().as_millis(),
                primary_keys: vec![],
                table_name: None,
            })
        }
    }
}

#[async_trait]
impl Database for MySqlDatabase {
    async fn execute_query(
        &self,
        query: &str,
        table_name: Option<String>,
        catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<QueryResult, String> {
        let mut conn = self.pool.acquire().await.map_err(|e| e.to_string())?;

        // Run the context switch on the SAME acquired connection that will execute the
        // query. Executing it against `&self.pool` would grab a different pooled
        // connection (this one is still borrowed), leaving the query connection without
        // a database context. Skip it when the query itself is a `USE`.
        if !query.trim_start().to_uppercase().starts_with("USE ") {
            self.switch_context(&mut conn, catalog.clone(), schema.clone())
                .await?;
        }

        self.exec_stmt(&mut conn, query, table_name, catalog, schema)
            .await
    }

    async fn execute_script(
        &self,
        statements: &[String],
        table_name: Option<String>,
        catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<Vec<QueryResult>, String> {
        if statements.is_empty() {
            return Ok(vec![]);
        }

        // A single connection for the whole script keeps session state alive across
        // statements: transactions (BEGIN/COMMIT), temporary tables, session variables.
        let mut conn = self.pool.acquire().await.map_err(|e| e.to_string())?;
        self.switch_context(&mut conn, catalog.clone(), schema.clone())
            .await?;

        let mut results = Vec::with_capacity(statements.len());
        for stmt in statements {
            let r = self
                .exec_stmt(
                    &mut conn,
                    stmt,
                    table_name.clone(),
                    catalog.clone(),
                    schema.clone(),
                )
                .await?;
            results.push(r);
        }
        Ok(results)
    }

    async fn get_table_list(
        &self,
        catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<QueryResult, String> {
        let start = Instant::now();
        let target = catalog.or(schema).unwrap_or_default();
        let query = "
            SELECT
                TABLE_NAME as `Name`,
                ENGINE as `Engine`,
                TABLE_ROWS as `Rows`,
                DATA_LENGTH as `Data Length`,
                AUTO_INCREMENT as `Auto Increment`,
                UPDATE_TIME as `Modified At`,
                TABLE_COLLATION as `Collation`,
                TABLE_COMMENT as `Comment`
            FROM information_schema.TABLES
            WHERE TABLE_SCHEMA = ?
            ORDER BY TABLE_NAME
        ";

        let rows = sqlx::query(query)
            .bind(target)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        self.process_rows(rows, start.elapsed().as_millis(), vec![], None, vec![])
    }

    async fn get_routine_list(
        &self,
        catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<QueryResult, String> {
        let start = Instant::now();
        let target = catalog.or(schema).unwrap_or_default();
        let query = "
            SELECT 
                ROUTINE_NAME as `Name`,
                ROUTINE_TYPE as `Type`,
                DATA_TYPE as `Return Type`,
                CREATED as `Created At`,
                LAST_ALTERED as `Modified At`,
                ROUTINE_COMMENT as `Comment`
            FROM information_schema.ROUTINES
            WHERE ROUTINE_SCHEMA = ?
            ORDER BY ROUTINE_NAME
        ";
        let rows = sqlx::query(query)
            .bind(target)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        self.process_rows(rows, start.elapsed().as_millis(), vec![], None, vec![])
    }

    async fn get_table_indexes(
        &self,
        table_name: &str,
        catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<crate::models::TableIndexInfo, String> {
        use crate::models::{TableForeignKey, TableIndex, TableIndexInfo};
        use std::collections::BTreeMap;

        let effective_db = catalog.or(schema).unwrap_or_else(|| {
            self.default_database.clone().unwrap_or_default()
        });

        // Fetch all index rows grouped by index name
        let idx_rows = sqlx::query(
            "SELECT INDEX_NAME, NON_UNIQUE, SEQ_IN_INDEX, COLUMN_NAME, INDEX_TYPE
             FROM information_schema.STATISTICS
             WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ?
             ORDER BY INDEX_NAME, SEQ_IN_INDEX",
        )
        .bind(&effective_db)
        .bind(table_name)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| e.to_string())?;

        let mut index_map: BTreeMap<String, TableIndex> = BTreeMap::new();
        for row in &idx_rows {
            let name: String = row.try_get("INDEX_NAME").unwrap_or_default();
            let non_unique: i8 = row.try_get("NON_UNIQUE").unwrap_or(1);
            let col: String = row.try_get("COLUMN_NAME").unwrap_or_default();
            let method: String = row.try_get("INDEX_TYPE").unwrap_or_default();

            let index_type = if name == "PRIMARY" {
                "PRIMARY"
            } else if non_unique == 0 {
                "UNIQUE"
            } else {
                "INDEX"
            };

            let entry = index_map.entry(name.clone()).or_insert_with(|| TableIndex {
                name: name.clone(),
                index_type: index_type.to_string(),
                method: method.clone(),
                columns: vec![],
            });
            entry.columns.push(col);
        }

        // Fetch foreign key rows
        let fk_rows = sqlx::query(
            "SELECT kcu.CONSTRAINT_NAME, kcu.COLUMN_NAME, kcu.ORDINAL_POSITION,
                    kcu.REFERENCED_TABLE_NAME, kcu.REFERENCED_COLUMN_NAME,
                    rc.UPDATE_RULE, rc.DELETE_RULE
             FROM information_schema.KEY_COLUMN_USAGE kcu
             JOIN information_schema.REFERENTIAL_CONSTRAINTS rc
               ON kcu.CONSTRAINT_NAME = rc.CONSTRAINT_NAME
              AND kcu.TABLE_SCHEMA = rc.CONSTRAINT_SCHEMA
              AND kcu.TABLE_NAME = rc.TABLE_NAME
             WHERE kcu.TABLE_SCHEMA = ? AND kcu.TABLE_NAME = ?
             ORDER BY kcu.CONSTRAINT_NAME, kcu.ORDINAL_POSITION",
        )
        .bind(&effective_db)
        .bind(table_name)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| e.to_string())?;

        let mut fk_map: BTreeMap<String, TableForeignKey> = BTreeMap::new();
        for row in &fk_rows {
            let name: String = row.try_get("CONSTRAINT_NAME").unwrap_or_default();
            let col: String = row.try_get("COLUMN_NAME").unwrap_or_default();
            let ref_table: String = row.try_get("REFERENCED_TABLE_NAME").unwrap_or_default();
            let ref_col: String = row.try_get("REFERENCED_COLUMN_NAME").unwrap_or_default();
            let on_update: String = row.try_get("UPDATE_RULE").unwrap_or_else(|_| "RESTRICT".to_string());
            let on_delete: String = row.try_get("DELETE_RULE").unwrap_or_else(|_| "RESTRICT".to_string());

            let entry = fk_map.entry(name.clone()).or_insert_with(|| TableForeignKey {
                name: name.clone(),
                columns: vec![],
                referenced_table: ref_table,
                referenced_columns: vec![],
                on_update,
                on_delete,
            });
            entry.columns.push(col);
            entry.referenced_columns.push(ref_col);
        }

        Ok(TableIndexInfo {
            indexes: index_map.into_values().collect(),
            foreign_keys: fk_map.into_values().collect(),
        })
    }

    async fn get_objects(&self) -> Result<Vec<DbObject>, String> {
        match self.version.group() {
            MySqlVersionGroup::Mysql84Plus => {
                v84::get_objects_v84(&self.pool, self.default_database.clone()).await
            }
            MySqlVersionGroup::Mysql80 | MySqlVersionGroup::Mysql57 => {
                v8::get_objects_v8(&self.pool, self.default_database.clone()).await
            }
            _ => legacy::get_objects(&self.pool, self.default_database.clone()).await,
        }
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
        let db_name = catalog.or(schema);
        let full_table = match db_name {
            Some(db) => format!("`{}`.`{}`", db, table_name),
            None => format!("`{}`", table_name),
        };
        let mut query = format!("UPDATE {} SET `{}` = ? WHERE ", full_table, column);
        let mut pk_list: Vec<(String, serde_json::Value)> = pks.into_iter().collect();
        pk_list.sort_by(|a, b| a.0.cmp(&b.0));

        let pk_terms: Vec<String> = pk_list
            .iter()
            .map(|(k, _)| format!("`{}` = ?", k))
            .collect();
        query.push_str(&pk_terms.join(" AND "));

        let val_str = self.val_to_opt_string(&value);
        let mut q = sqlx::query(&query).bind(val_str.clone());
        for (_, v) in &pk_list {
            q = q.bind(self.val_to_opt_string(v));
        }
        let res = q.execute(&self.pool).await.map_err(|e| e.to_string())?;

        let mut logged_query = format!(
            "UPDATE {} SET `{}` = '{}' WHERE ",
            full_table,
            column,
            val_str
                .unwrap_or_else(|| "NULL".to_string())
                .replace('\'', "''")
        );
        let pk_logged: Vec<String> = pk_list
            .iter()
            .map(|(k, v)| {
                format!(
                    "`{}` = '{}'",
                    k,
                    self.val_to_opt_string(v)
                        .unwrap_or_else(|| "NULL".to_string())
                        .replace('\'', "''")
                )
            })
            .collect();
        logged_query.push_str(&pk_logged.join(" AND "));

        Ok(RowActionResult {
            affected_rows: res.rows_affected(),
            query: logged_query,
        })
    }

    async fn insert_row(
        &self,
        table_name: &str,
        data: HashMap<String, serde_json::Value>,
        catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<RowActionResult, String> {
        let db_name = catalog.or(schema);
        let full_table = match db_name {
            Some(db) => format!("`{}`.`{}`", db, table_name),
            None => format!("`{}`", table_name),
        };
        let columns: Vec<String> = data.keys().cloned().collect();
        let placeholders: Vec<String> = vec!["?".to_string(); columns.len()];

        let query = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            full_table,
            columns
                .iter()
                .map(|c| format!("`{}`", c))
                .collect::<Vec<_>>()
                .join(", "),
            placeholders.join(", ")
        );

        let mut q = sqlx::query(&query);
        let mut vals_logged = Vec::new();
        for col in &columns {
            let val = self.val_to_opt_string(&data[col]);
            q = q.bind(val.clone());
            vals_logged.push(match val {
                Some(v) => format!("'{}'", v.replace('\'', "''")),
                None => "NULL".to_string(),
            });
        }
        let res = q.execute(&self.pool).await.map_err(|e| e.to_string())?;

        let logged_query = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            full_table,
            columns
                .iter()
                .map(|c| format!("`{}`", c))
                .collect::<Vec<_>>()
                .join(", "),
            vals_logged.join(", ")
        );

        Ok(RowActionResult {
            affected_rows: res.rows_affected(),
            query: logged_query,
        })
    }

    async fn get_table_definition(
        &self,
        table_name: &str,
        catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<TableDefinition, String> {
        let target_schema = catalog.clone().or(schema.clone());
        let effective_db = target_schema.clone().unwrap_or_else(|| {
            self.default_database
                .clone()
                .unwrap_or_else(|| "mysql".to_string())
        });
        let mut columns = Vec::new();

        let table_collation: Option<String> = sqlx::query(
            "SELECT TABLE_COLLATION FROM information_schema.TABLES WHERE TABLE_NAME = ? AND TABLE_SCHEMA = ?"
        )
        .bind(table_name)
        .bind(&effective_db)
        .fetch_optional(&self.pool)
        .await
        .ok()
        .flatten()
        .and_then(|row| row.try_get::<Option<String>, _>("TABLE_COLLATION").ok().flatten());

        let query = "
            SELECT
                COLUMN_NAME as column_name,
                DATA_TYPE as data_type,
                COLUMN_TYPE as column_type,
                IS_NULLABLE as is_nullable,
                COLUMN_DEFAULT as column_default,
                CHARACTER_MAXIMUM_LENGTH as character_maximum_length,
                NUMERIC_PRECISION as numeric_precision,
                NUMERIC_SCALE as numeric_scale,
                COLUMN_COMMENT as column_comment,
                COLUMN_KEY as column_key,
                EXTRA as extra,
                COLLATION_NAME as collation_name
            FROM information_schema.COLUMNS
            WHERE TABLE_NAME = ? AND TABLE_SCHEMA = ?
            ORDER BY ORDINAL_POSITION
        ";
        let rows = sqlx::query(query)
            .bind(table_name)
            .bind(effective_db)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        for row in rows {
            let get_num = |row: &sqlx::mysql::MySqlRow, col: &str| -> Option<i64> {
                row.try_get::<Option<i64>, _>(col).unwrap_or_else(|_| {
                    row.try_get::<Option<i32>, _>(col)
                        .map(|v| v.map(|i| i as i64))
                        .unwrap_or_else(|_| {
                            row.try_get::<Option<i16>, _>(col)
                                .map(|v| v.map(|i| i as i64))
                                .unwrap_or(None)
                        })
                })
            };

            let char_len = get_num(&row, "character_maximum_length");
            let num_prec = get_num(&row, "numeric_precision");
            let num_scale = get_num(&row, "numeric_scale");
            let col_type: String = row.try_get::<String, _>("column_type")
                .or_else(|_| row.try_get::<Vec<u8>, _>("column_type").map(|b| String::from_utf8_lossy(&b).to_string()))
                .unwrap_or_default();

            let length = if let Some(p) = num_prec {
                if let Some(s) = num_scale {
                    if s > 0 {
                        Some(format!("{},{}", p, s))
                    } else {
                        Some(p.to_string())
                    }
                } else {
                    Some(p.to_string())
                }
            } else if let Some(l) = char_len {
                Some(l.to_string())
            } else if let Some(start) = col_type.find('(') {
                if let Some(end) = col_type.find(')') {
                    Some(col_type[start + 1..end].to_string())
                } else {
                    None
                }
            } else {
                None
            };

            columns.push(TableColumn {
                name: row.try_get::<String, _>("column_name")
                    .or_else(|_| row.try_get::<Vec<u8>, _>("column_name").map(|b| String::from_utf8_lossy(&b).to_string()))
                    .unwrap_or_default(),
                data_type: row.try_get::<String, _>("data_type")
                    .or_else(|_| row.try_get::<Vec<u8>, _>("data_type").map(|b| String::from_utf8_lossy(&b).to_string()))
                    .unwrap_or_default(),
                is_nullable: row.try_get::<String, _>("is_nullable")
                    .or_else(|_| row.try_get::<Vec<u8>, _>("is_nullable").map(|b| String::from_utf8_lossy(&b).to_string()))
                    .unwrap_or_default() == "YES",
                is_primary_key: row.try_get::<String, _>("column_key")
                    .or_else(|_| row.try_get::<Vec<u8>, _>("column_key").map(|b| String::from_utf8_lossy(&b).to_string()))
                    .unwrap_or_default() == "PRI",
                is_auto_increment: row
                    .try_get::<String, _>("extra")
                    .or_else(|_| row.try_get::<Vec<u8>, _>("extra").map(|b| String::from_utf8_lossy(&b).to_string()))
                    .unwrap_or_default()
                    .contains("auto_increment"),
                default_value: row.try_get::<String, _>("column_default")
                    .or_else(|_| row.try_get::<Vec<u8>, _>("column_default").map(|b| String::from_utf8_lossy(&b).to_string()))
                    .ok(),
                comment: row.try_get::<String, _>("column_comment")
                    .or_else(|_| row.try_get::<Vec<u8>, _>("column_comment").map(|b| String::from_utf8_lossy(&b).to_string()))
                    .ok(),
                length,
                collation: row.try_get::<Option<String>, _>("collation_name")
                    .or_else(|_| row.try_get::<Option<Vec<u8>>, _>("collation_name").map(|b| b.map(|v| String::from_utf8_lossy(&v).to_string())))
                    .ok()
                    .flatten(),
            });
        }

        Ok(TableDefinition {
            name: table_name.to_string(),
            columns,
            catalog,
            schema,
            comment: None,
            collation: table_collation,
        })
    }

    async fn create_table(&self, definition: TableDefinition) -> Result<(), String> {
        let sql = self.get_create_table_sql(&definition);
        sqlx::query(&sql)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn alter_table(&self, old: TableDefinition, new: TableDefinition) -> Result<(), String> {
        let statements = self.get_alter_table_sql(&old, &new);
        for sql in statements {
            sqlx::query(&sql)
                .execute(&self.pool)
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    async fn generate_table_sql(
        &self,
        old: Option<TableDefinition>,
        new: TableDefinition,
    ) -> Result<String, String> {
        if let Some(old_def) = old {
            Ok(self.get_alter_table_sql(&old_def, &new).join(";\n"))
        } else {
            Ok(self.get_create_table_sql(&new))
        }
    }

    async fn get_routine_definition(
        &self,
        name: &str,
        routine_type: &str,
        catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<RoutineDefinition, String> {
        let db_name = catalog.clone().or(schema.clone());
        let full_name = if let Some(db) = &db_name {
            format!("`{}`.`{}`", db, name)
        } else {
            format!("`{}`", name)
        };

        let query = if routine_type.to_lowercase() == "procedure" {
            format!("SHOW CREATE PROCEDURE {}", full_name)
        } else {
            format!("SHOW CREATE FUNCTION {}", full_name)
        };

        let row = sqlx::query(&query)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        let definition: String = row.try_get::<String, _>(2)
            .or_else(|_| row.try_get::<Vec<u8>, _>(2).map(|b| String::from_utf8_lossy(&b).to_string()))
            .unwrap_or_else(|_| {
                if routine_type.to_lowercase() == "procedure" {
                    row.try_get::<String, _>("Create Procedure")
                        .or_else(|_| row.try_get::<Vec<u8>, _>("Create Procedure").map(|b| String::from_utf8_lossy(&b).to_string()))
                        .unwrap_or_default()
                } else {
                    row.try_get::<String, _>("Create Function")
                        .or_else(|_| row.try_get::<Vec<u8>, _>("Create Function").map(|b| String::from_utf8_lossy(&b).to_string()))
                        .unwrap_or_default()
                }
            });

        Ok(RoutineDefinition {
            name: name.to_string(),
            routine_type: routine_type.to_string(),
            definition,
            catalog,
            schema,
        })
    }

    async fn save_routine(&self, definition: RoutineDefinition) -> Result<(), String> {
        let old_def = self
            .get_routine_definition(
                &definition.name,
                &definition.routine_type,
                definition.catalog.clone(),
                definition.schema.clone(),
            )
            .await
            .ok();

        let db_name = definition.catalog.clone().or(definition.schema.clone());
        let full_name = if let Some(db) = &db_name {
            format!("`{}`.`{}`", db, definition.name)
        } else {
            format!("`{}`", definition.name)
        };

        let drop_query = if definition.routine_type.to_lowercase() == "procedure" {
            format!("DROP PROCEDURE IF EXISTS {}", full_name)
        } else {
            format!("DROP FUNCTION IF EXISTS {}", full_name)
        };

        sqlx::raw_sql(&drop_query)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Failed to drop existing routine: {}", e))?;

        if let Err(e) = sqlx::raw_sql(&definition.definition)
            .execute(&self.pool)
            .await
        {
            if let Some(old) = old_def {
                let _ = sqlx::raw_sql(&old.definition).execute(&self.pool).await;
                return Err(format!(
                    "Failed to create routine: {}. The original routine was restored.",
                    e
                ));
            }
            return Err(format!(
                "Failed to create routine and no backup was available: {}",
                e
            ));
        }

        Ok(())
    }

    async fn get_view_definition(
        &self,
        name: &str,
        catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<ViewDefinition, String> {
        let db_name = catalog.clone().or(schema.clone());
        let full_name = if let Some(db) = &db_name {
            format!("`{}`.`{}`", db, name)
        } else {
            format!("`{}`", name)
        };

        let query = format!("SHOW CREATE VIEW {}", full_name);
        let row = sqlx::query(&query)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        let definition: String = row
            .try_get::<String, _>(1)
            .or_else(|_| row.try_get::<Vec<u8>, _>(1).map(|b| String::from_utf8_lossy(&b).to_string()))
            .unwrap_or_else(|_| {
                row.try_get::<String, _>("Create View")
                    .or_else(|_| row.try_get::<Vec<u8>, _>("Create View").map(|b| String::from_utf8_lossy(&b).to_string()))
                    .unwrap_or_default()
            });

        Ok(ViewDefinition {
            name: name.to_string(),
            definition,
            catalog,
            schema,
        })
    }

    async fn save_view(&self, definition: ViewDefinition) -> Result<(), String> {
        let old_def = self
            .get_view_definition(
                &definition.name,
                definition.catalog.clone(),
                definition.schema.clone(),
            )
            .await
            .ok();

        let mut sql = definition.definition.clone();
        if !sql.to_uppercase().contains("CREATE OR REPLACE") {
            if let Some(pos) = sql.to_uppercase().find("CREATE") {
                sql.replace_range(pos..pos + 6, "CREATE OR REPLACE");
            }
        }

        if let Err(e) = sqlx::raw_sql(&sql).execute(&self.pool).await {
            if let Some(old) = old_def {
                let _ = sqlx::raw_sql(&old.definition).execute(&self.pool).await;
            }
            return Err(format!("Failed to save view: {}", e));
        }
        Ok(())
    }

    async fn close(&self) {
        self.pool.close().await;
    }
}
