use super::{utils, Database};
use crate::models::ColumnInfo;
use crate::models::{
    DbObject, QueryResult, RoutineDefinition, RowActionResult, TableColumn, TableDefinition,
    ViewDefinition,
};
use async_trait::async_trait;
use chrono::{DateTime, NaiveDateTime, Utc};
use sqlx::{Column, PgPool, Row, TypeInfo};
use std::collections::HashMap;
use std::time::Instant;

/// PostgreSQL implementation of the Database trait.
pub struct PostgreSqlDatabase {
    pub pool: PgPool,
    pub default_schema: Option<String>,
}

impl PostgreSqlDatabase {
    /// Creates a new PostgreSQL database instance.
    pub fn new(pool: PgPool, default_schema: Option<String>) -> Self {
        Self {
            pool,
            default_schema,
        }
    }

    /// Generates the SQL string needed to create a table from a definition,
    /// including SERIAL types for auto-increments and table/column comments.
    fn get_create_table_sql(&self, definition: &TableDefinition) -> String {
        let mut sql = format!("CREATE TABLE \"{}\" (", definition.name);
        let mut col_defs = Vec::new();
        let mut pks = Vec::new();

        for col in &definition.columns {
            let mut data_type = col.data_type.clone();

            if col.is_auto_increment {
                if data_type.to_uppercase() == "INT" || data_type.to_uppercase() == "INTEGER" {
                    data_type = "SERIAL".to_string();
                } else if data_type.to_uppercase() == "BIGINT" {
                    data_type = "BIGSERIAL".to_string();
                } else if data_type.to_uppercase() == "SMALLINT" {
                    data_type = "SMALLSERIAL".to_string();
                }
            }

            let mut def = format!("\"{}\" {}", col.name, data_type);

            if let Some(len) = &col.length {
                if !len.is_empty() && len != "0" && !col.is_auto_increment {
                    def.push_str(&format!("({})", len));
                }
            }

            if !col.is_nullable {
                def.push_str(" NOT NULL");
            }

            if let Some(default) = &col.default_value {
                if !default.is_empty() && !col.is_auto_increment {
                    def.push_str(&format!(" DEFAULT {}", default));
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
                    .map(|k| format!("\"{}\"", k))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            col_defs.push(pk_sql);
        }

        sql.push_str(&col_defs.join(", "));
        sql.push(')');

        let mut final_statements = vec![sql];

        if let Some(comment) = &definition.comment {
            if !comment.is_empty() {
                final_statements.push(format!(
                    "COMMENT ON TABLE \"{}\" IS '{}'",
                    definition.name,
                    comment.replace('\'', "''")
                ));
            }
        }

        for col in &definition.columns {
            if let Some(comment) = &col.comment {
                if !comment.is_empty() {
                    final_statements.push(format!(
                        "COMMENT ON COLUMN \"{}\".\"{}\" IS '{}'",
                        definition.name,
                        col.name,
                        comment.replace('\'', "''")
                    ));
                }
            }
        }

        final_statements.join(";\n")
    }

    /// Compares two table definitions and generates a list of ALTER statements
    /// (Rename, type changes, nullability, defaults, sequences, and comments).
    fn get_alter_table_sql(&self, old: &TableDefinition, new: &TableDefinition) -> Vec<String> {
        let mut statements = Vec::new();

        // 1. Check for table rename
        if old.name != new.name {
            statements.push(format!(
                "ALTER TABLE \"{}\" RENAME TO \"{}\"",
                old.name, new.name
            ));
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
                    "ALTER TABLE \"{}\" DROP COLUMN \"{}\"",
                    table_name, name
                ));
            }
        }

        // Additions and Modifications
        for col in &new.columns {
            if let Some(old_col) = old_cols.get(&col.name) {
                // Check for type change
                if old_col.data_type != col.data_type || old_col.length != col.length {
                    let mut type_def = col.data_type.clone();
                    if let Some(len) = &col.length {
                        if !len.is_empty() && len != "0" {
                            type_def.push_str(&format!("({})", len));
                        }
                    }
                    statements.push(format!(
                        "ALTER TABLE \"{}\" ALTER COLUMN \"{}\" TYPE {}",
                        table_name, col.name, type_def
                    ));
                }

                // Check for nullability change
                if old_col.is_nullable != col.is_nullable {
                    if col.is_nullable {
                        statements.push(format!(
                            "ALTER TABLE \"{}\" ALTER COLUMN \"{}\" DROP NOT NULL",
                            table_name, col.name
                        ));
                    } else {
                        statements.push(format!(
                            "ALTER TABLE \"{}\" ALTER COLUMN \"{}\" SET NOT NULL",
                            table_name, col.name
                        ));
                    }
                }

                // Check for default value change
                if old_col.default_value != col.default_value {
                    if let Some(default) = &col.default_value {
                        if !default.is_empty() {
                            statements.push(format!(
                                "ALTER TABLE \"{}\" ALTER COLUMN \"{}\" SET DEFAULT {}",
                                table_name, col.name, default
                            ));
                        } else {
                            statements.push(format!(
                                "ALTER TABLE \"{}\" ALTER COLUMN \"{}\" DROP DEFAULT",
                                table_name, col.name
                            ));
                        }
                    } else {
                        statements.push(format!(
                            "ALTER TABLE \"{}\" ALTER COLUMN \"{}\" DROP DEFAULT",
                            table_name, col.name
                        ));
                    }
                }

                // AUTO_INCREMENT (Sequence) toggling
                if old_col.is_auto_increment != col.is_auto_increment {
                    let seq_name = format!("{}_{}_seq", table_name, col.name);
                    if col.is_auto_increment {
                        statements.push(format!("CREATE SEQUENCE IF NOT EXISTS \"{}\"", seq_name));
                        statements.push(format!("ALTER TABLE \"{}\" ALTER COLUMN \"{}\" SET DEFAULT nextval('\"{}\"'::regclass)", table_name, col.name, seq_name));
                        statements.push(format!(
                            "ALTER SEQUENCE \"{}\" OWNED BY \"{}\".\"{}\"",
                            seq_name, table_name, col.name
                        ));
                    } else {
                        statements.push(format!(
                            "ALTER TABLE \"{}\" ALTER COLUMN \"{}\" DROP DEFAULT",
                            table_name, col.name
                        ));
                        statements.push(format!("DROP SEQUENCE IF EXISTS \"{}\"", seq_name));
                    }
                }

                // Comment change
                if old_col.comment != col.comment {
                    let comment_val = col.comment.as_deref().unwrap_or("");
                    statements.push(format!(
                        "COMMENT ON COLUMN \"{}\".\"{}\" IS '{}'",
                        table_name,
                        col.name,
                        comment_val.replace('\'', "''")
                    ));
                }
            } else {
                // New column
                let mut data_type = col.data_type.clone();
                if col.is_auto_increment {
                    if data_type.to_uppercase() == "INT" || data_type.to_uppercase() == "INTEGER" {
                        data_type = "SERIAL".to_string();
                    } else if data_type.to_uppercase() == "BIGINT" {
                        data_type = "BIGSERIAL".to_string();
                    } else if data_type.to_uppercase() == "SMALLINT" {
                        data_type = "SMALLSERIAL".to_string();
                    }
                }

                let mut def = format!("\"{}\" {}", col.name, data_type);
                if let Some(len) = &col.length {
                    if !len.is_empty() && len != "0" && !col.is_auto_increment {
                        def.push_str(&format!("({})", len));
                    }
                }
                if !col.is_nullable {
                    def.push_str(" NOT NULL");
                }
                if let Some(default) = &col.default_value {
                    if !default.is_empty() && !col.is_auto_increment {
                        def.push_str(&format!(" DEFAULT {}", default));
                    }
                }
                statements.push(format!("ALTER TABLE \"{}\" ADD COLUMN {}", table_name, def));

                if let Some(comment) = &col.comment {
                    if !comment.is_empty() {
                        statements.push(format!(
                            "COMMENT ON COLUMN \"{}\".\"{}\" IS '{}'",
                            table_name,
                            col.name,
                            comment.replace('\'', "''")
                        ));
                    }
                }
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
            // Find old constraint name (default is <table>_pkey)
            // For simplicity we assume the default name.
            // A more robust way would be to query pg_constraint.
            statements.push(format!(
                "ALTER TABLE \"{}\" DROP CONSTRAINT IF EXISTS \"{}_pkey\"",
                table_name, old.name
            ));
            if !new_pks.is_empty() {
                let pks_str = new_pks
                    .iter()
                    .map(|k| format!("\"{}\"", k))
                    .collect::<Vec<_>>()
                    .join(", ");
                statements.push(format!(
                    "ALTER TABLE \"{}\" ADD PRIMARY KEY ({})",
                    table_name, pks_str
                ));
            }
        }

        // 4. Table Comment
        if old.comment != new.comment {
            let comment_val = new.comment.as_deref().unwrap_or("");
            statements.push(format!(
                "COMMENT ON TABLE \"{}\" IS '{}'",
                table_name,
                comment_val.replace('\'', "''")
            ));
        }

        statements
    }

    /// Converts a PostgreSQL row value into a JSON value based on simple type mapping.
    fn row_to_json(&self, row: &sqlx::postgres::PgRow, col: &str) -> serde_json::Value {
        let col_info = row.column(col);
        let type_info = col_info.type_info();
        let type_name = type_info.name();

        let result = match type_name {
            "BOOL" => row.try_get::<bool, _>(col).map(|v| v.into()),
            "INT2" => row.try_get::<i16, _>(col).map(|v| v.into()),
            "INT4" => row.try_get::<i32, _>(col).map(|v| v.into()),
            "INT8" => row.try_get::<i64, _>(col).map(|v| v.into()),
            "FLOAT4" => row.try_get::<f32, _>(col).map(|v| {
                // Round-trip through f32's Display so the f64 reflects f32's
                // ~7-digit precision; `v as f64` would expose the binary
                // approximation (e.g. 0.0082_f32 → 0.008200000040233135_f64).
                format!("{}", v)
                    .parse::<f64>()
                    .ok()
                    .and_then(|f| serde_json::Number::from_f64(f).map(serde_json::Value::Number))
                    .unwrap_or(serde_json::Value::Null)
            }),
            "FLOAT8" => row.try_get::<f64, _>(col).map(|v| {
                serde_json::Number::from_f64(v)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null)
            }),
            "NUMERIC" => row
                .try_get::<rust_decimal::Decimal, _>(col)
                .map(|v| serde_json::Value::String(v.to_string())),
            "TIMESTAMP" | "TIMESTAMPTZ" => {
                row.try_get::<DateTime<Utc>, _>(col)
                    .map(|v| v.to_rfc3339().into())
                    .or_else(|_| row.try_get::<NaiveDateTime, _>(col).map(|v| v.to_string().into()))
            }
            "DATE" => row
                .try_get::<chrono::NaiveDate, _>(col)
                .map(|v| v.to_string().into()),
            "TIME" | "TIMETZ" => row
                .try_get::<chrono::NaiveTime, _>(col)
                .map(|v| v.to_string().into()),
            "JSON" | "JSONB" => row.try_get::<serde_json::Value, _>(col),
            "UUID" => row.try_get::<String, _>(col).map(|v| v.into()),
            "BYTEA" => row
                .try_get::<String, _>(col)
                .map(|v| v.into())
                .or_else(|_| {
                    row.try_get::<Vec<u8>, _>(col).map(|v| {
                        let hex_val: String = v.iter().map(|b| format!("{:02x}", b)).collect();
                        format!("0x{}", hex_val).into()
                    })
                }),
            _ => row.try_get::<String, _>(col).map(|v| v.into()),
        };

        match result {
            Ok(val) => val,
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("UnexpectedNullError")
                    || err_str.to_lowercase().contains("is null")
                    || err_str.to_lowercase().contains("unexpected null")
                {
                    return serde_json::Value::Null;
                }

                if let Ok(v) = row.try_get::<String, _>(col) {
                    return v.into();
                }

                log::error!(
                    "Failed to decode Postgres column '{}' (type: {}): {}",
                    col,
                    type_name,
                    e
                );
                serde_json::Value::String(format!("[Decode Error: {}]", e))
            }
        }
    }

    /// Processes a list of PgRows into a QueryResult, including column types.
    fn process_rows(
        &self,
        rows: Vec<sqlx::postgres::PgRow>,
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
                let val = self.row_to_json(&row, name);
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

    async fn get_primary_keys(
        &self,
        table: &str,
        catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<Vec<String>, String> {
        let target_schema = schema.or(catalog).unwrap_or_else(|| "public".to_string());
        let full_table_name = format!("\"{}\".\"{}\"", target_schema, table);
        let query = "
            SELECT a.attname
            FROM pg_index i
            JOIN pg_attribute a ON a.attrelid = i.indrelid AND a.attnum = ANY(i.indkey)
            WHERE i.indrelid = $1::regclass AND i.indisprimary
        ";
        let rows = sqlx::query(query)
            .bind(&full_table_name)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(rows.into_iter().map(|r| r.get::<String, _>(0)).collect())
    }

    async fn get_columns_internal(
        &self,
        table: &str,
        catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<Vec<String>, String> {
        let target_schema = schema.or(catalog).unwrap_or_else(|| "public".to_string());
        let query = "
            SELECT column_name
            FROM information_schema.columns
            WHERE table_name = $1 AND table_schema = $2
            ORDER BY ordinal_position
        ";
        let rows = sqlx::query(query)
            .bind(table)
            .bind(target_schema)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(rows.into_iter().map(|r| r.get::<String, _>(0)).collect())
    }

    /// Converts a serde_json::Value to a String option, stripping quotes for consistency.
    fn val_to_opt_string(&self, val: &serde_json::Value) -> Option<String> {
        match val {
            serde_json::Value::Null => None,
            serde_json::Value::String(s) => Some(s.clone()),
            _ => Some(val.to_string().trim_matches('"').to_string()),
        }
    }
}

#[async_trait]
impl Database for PostgreSqlDatabase {
    /// Executes a SQL query. For SELECTs, it returns rows and metadata.
    /// For other statements, it returns affected rows.
    /// Handles schema context switching using SET search_path.
    async fn execute_query(
        &self,
        query: &str,
        table_name: Option<String>,
        catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<QueryResult, String> {
        let start = Instant::now();
        let info = utils::parse_query(query, table_name);

        let mut conn = self.pool.acquire().await.map_err(|e| e.to_string())?;

        // Handle schema context switching for PostgreSQL
        let effective_schema = utils::get_effective_context(
            schema.clone(),
            catalog.clone(),
            self.default_schema.clone(),
        );

        if let Some(sch) = effective_schema {
            if !info.q_trimmed.starts_with("SET SEARCH_PATH") {
                let set_path = format!("SET search_path TO \"{}\"", sch);
                sqlx::raw_sql(&set_path)
                    .execute(&self.pool)
                    .await
                    .map_err(|e| format!("Failed to switch schema context to `{}`: {}", sch, e))?;
            }
        }

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
                if err_msg.to_lowercase().contains("no database selected") {
                    return Err("No database selected. Please specify a database in your connection settings.".to_string());
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

    /// Fetches a list of tables and their metadata (row count estimate, comments) in the schema.
    async fn get_table_list(
        &self,
        _catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<QueryResult, String> {
        let start = Instant::now();
        let target = schema.unwrap_or_else(|| "public".to_string());
        let query = "
            SELECT 
                t.table_name as \"Name\",
                pg_stat_get_live_tuples(c.oid) as \"Rows\",
                obj_description(c.oid, 'pg_class') as \"Comment\"
            FROM information_schema.tables t
            JOIN pg_class c ON c.relname = t.table_name
            JOIN pg_namespace n ON n.oid = c.relnamespace AND n.nspname = t.table_schema
            WHERE t.table_schema = $1 AND t.table_type = 'BASE TABLE'
            ORDER BY t.table_name
        ";
        let rows = sqlx::query(query)
            .bind(target)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        self.process_rows(rows, start.elapsed().as_millis(), vec![], None, vec![])
    }

    /// Fetches a list of routines (functions/procedures) and their metadata in the schema.
    async fn get_routine_list(
        &self,
        _catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<QueryResult, String> {
        let start = Instant::now();
        let target = schema.unwrap_or_else(|| "public".to_string());
        let query = "
            SELECT 
                p.proname as \"Name\",
                CASE 
                    WHEN p.prokind = 'f' THEN 'FUNCTION'
                    WHEN p.prokind = 'p' THEN 'PROCEDURE'
                    WHEN p.prokind = 'a' THEN 'AGGREGATE'
                    WHEN p.prokind = 'w' THEN 'WINDOW'
                    ELSE 'OTHER'
                END as \"Type\",
                pg_get_function_result(p.oid) as \"Result Type\",
                p.proretset as \"Returns Set\",
                l.lanname as \"Language\",
                CASE 
                    WHEN p.provolatile = 'i' THEN 'IMMUTABLE'
                    WHEN p.provolatile = 's' THEN 'STABLE'
                    WHEN p.provolatile = 'v' THEN 'VOLATILE'
                END as \"Volatility\",
                obj_description(p.oid, 'pg_proc') as \"Comment\"
            FROM pg_proc p
            JOIN pg_namespace n ON n.oid = p.pronamespace
            JOIN pg_language l ON l.oid = p.prolang
            WHERE n.nspname = $1
            ORDER BY p.proname
        ";
        let rows = sqlx::query(query)
            .bind(target)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        self.process_rows(rows, start.elapsed().as_millis(), vec![], None, vec![])
    }

    /// Returns a comprehensive list of all schema objects for autocomplete/sidebar.
    async fn get_objects(&self) -> Result<Vec<DbObject>, String> {
        let mut objects = Vec::new();

        // 1. Fetch tables and views
        let table_query = "
            SELECT 
                t.table_catalog as catalog, 
                t.table_schema as schema, 
                t.table_name as name, 
                CASE WHEN t.table_type = 'BASE TABLE' THEN 'table' ELSE 'view' END as object_type,
                obj_description(c.oid, 'pg_class') as description
            FROM information_schema.tables t
            JOIN pg_class c ON c.relname = t.table_name
            JOIN pg_namespace n ON n.oid = c.relnamespace AND n.nspname = t.table_schema
            WHERE t.table_schema NOT IN ('information_schema', 'pg_catalog')
        ";
        let table_rows = sqlx::query(table_query)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        // 2. Fetch routines
        let routine_query = "
            SELECT 
                routine_catalog as catalog, 
                routine_schema as schema, 
                routine_name as name, 
                LOWER(routine_type) as object_type
            FROM information_schema.routines
            WHERE routine_schema NOT IN ('information_schema', 'pg_catalog')
        ";
        let routine_rows = sqlx::query(routine_query)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        // 3. Fetch columns
        let column_query = "
            SELECT table_schema, table_name, column_name, data_type
            FROM information_schema.columns
            WHERE table_schema NOT IN ('information_schema', 'pg_catalog')
            ORDER BY table_schema, table_name, ordinal_position
        ";
        let column_rows = sqlx::query(column_query)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        let mut column_map: HashMap<(String, String), Vec<ColumnInfo>> = HashMap::new();
        for row in column_rows {
            let schema: String = row.try_get("table_schema").unwrap_or_default();
            let table: String = row.try_get("table_name").unwrap_or_default();
            let name: String = row.try_get("column_name").unwrap_or_default();
            let data_type: String = row.try_get("data_type").unwrap_or_default();
            column_map
                .entry((schema, table))
                .or_default()
                .push(ColumnInfo { name, data_type });
        }

        // Combine findings
        for row in table_rows {
            let schema: Option<String> = row.try_get("schema").ok().flatten();
            let name: String = row.try_get("name").unwrap_or_default();
            objects.push(DbObject {
                name: name.clone(),
                object_type: row.try_get("object_type").unwrap_or_default(),
                schema: schema.clone().filter(|s| !s.is_empty()),
                catalog: row
                    .try_get::<Option<String>, _>("catalog")
                    .ok()
                    .flatten()
                    .filter(|s| !s.is_empty()),
                description: row.try_get("description").ok(),
                parent: None,
                columns: column_map.remove(&(schema.unwrap_or_default(), name)),
            });
        }

        for row in routine_rows {
            objects.push(DbObject {
                name: row.try_get("name").unwrap_or_default(),
                object_type: row.try_get("object_type").unwrap_or_default(),
                schema: row.try_get("schema").ok(),
                catalog: row.try_get("catalog").ok(),
                parent: None,
                columns: None,
                description: row.try_get("description").ok(),
            });
        }

        Ok(objects)
    }

    /// Updates a specific row's cell using primary keys for identification.
    async fn update_row(
        &self,
        table_name: &str,
        pks: HashMap<String, serde_json::Value>,
        column: &str,
        value: serde_json::Value,
        _catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<RowActionResult, String> {
        let full_table = match schema {
            Some(sch) => format!("\"{}\".\"{}\"", sch, table_name),
            None => format!("\"{}\"", table_name),
        };
        let mut query = format!("UPDATE {} SET \"{}\" = $1 WHERE ", full_table, column);
        let mut pk_list: Vec<(String, serde_json::Value)> = pks.into_iter().collect();
        pk_list.sort_by(|a, b| a.0.cmp(&b.0));

        let pk_terms: Vec<String> = pk_list
            .iter()
            .enumerate()
            .map(|(i, (k, _))| format!("\"{}\" = ${}", k, i + 2))
            .collect();
        query.push_str(&pk_terms.join(" AND "));

        let val_str = self.val_to_opt_string(&value);
        let mut q = sqlx::query(&query).bind(val_str.clone());
        for (_, v) in &pk_list {
            q = q.bind(self.val_to_opt_string(v));
        }
        let res = q.execute(&self.pool).await.map_err(|e| e.to_string())?;

        // Reconstruct query for logging
        let mut logged_query = format!(
            "UPDATE {} SET \"{}\" = '{}' WHERE ",
            full_table,
            column,
            val_str
                .unwrap_or_else(|| "NULL".to_string())
                .replace("'", "''")
        );
        let pk_logged: Vec<String> = pk_list
            .iter()
            .map(|(k, v)| {
                format!(
                    "\"{}\" = '{}'",
                    k,
                    self.val_to_opt_string(v)
                        .unwrap_or_else(|| "NULL".to_string())
                        .replace("'", "''")
                )
            })
            .collect();
        logged_query.push_str(&pk_logged.join(" AND "));

        Ok(RowActionResult {
            affected_rows: res.rows_affected(),
            query: logged_query,
        })
    }

    /// Inserts a new row with the provided column-value map.
    async fn insert_row(
        &self,
        table_name: &str,
        data: HashMap<String, serde_json::Value>,
        _catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<RowActionResult, String> {
        let full_table = match schema {
            Some(sch) => format!("\"{}\".\"{}\"", sch, table_name),
            None => format!("\"{}\"", table_name),
        };
        let columns: Vec<String> = data.keys().cloned().collect();
        let placeholders: Vec<String> = (0..columns.len()).map(|i| format!("${}", i + 1)).collect();

        let query = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            full_table,
            columns
                .iter()
                .map(|c| format!("\"{}\"", c))
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
                Some(v) => format!("'{}'", v.replace("'", "''")),
                None => "NULL".to_string(),
            });
        }
        let res = q.execute(&self.pool).await.map_err(|e| e.to_string())?;

        let logged_query = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            full_table,
            columns
                .iter()
                .map(|c| format!("\"{}\"", c))
                .collect::<Vec<_>>()
                .join(", "),
            vals_logged.join(", ")
        );

        Ok(RowActionResult {
            affected_rows: res.rows_affected(),
            query: logged_query,
        })
    }

    /// Fetches the structure of a table, identifying primary keys and auto-increments.
    async fn get_table_definition(
        &self,
        table_name: &str,
        catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<TableDefinition, String> {
        let target_schema = schema.clone().unwrap_or_else(|| "public".to_string());
        let mut columns = Vec::new();

        let query = "
            SELECT 
                cols.column_name, 
                cols.data_type, 
                cols.is_nullable, 
                cols.column_default, 
                cols.character_maximum_length,
                cols.numeric_precision,
                cols.numeric_scale,
                (SELECT pg_catalog.col_description(c.oid, cols.ordinal_position::int)
                 FROM pg_catalog.pg_class c
                 WHERE c.relname = cols.table_name AND c.relnamespace = (SELECT oid FROM pg_catalog.pg_namespace WHERE nspname = cols.table_schema)) as column_comment,
                EXISTS (
                    SELECT 1 FROM information_schema.key_column_usage kcu
                    JOIN information_schema.table_constraints tc ON kcu.constraint_name = tc.constraint_name
                    WHERE kcu.table_name = cols.table_name AND kcu.column_name = cols.column_name AND tc.constraint_type = 'PRIMARY KEY' AND kcu.table_schema = cols.table_schema
                ) as is_primary
            FROM information_schema.columns cols
            WHERE table_name = $1 AND table_schema = $2
            ORDER BY ordinal_position
        ";
        let rows = sqlx::query(query)
            .bind(table_name)
            .bind(&target_schema)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        for row in rows {
            let get_num = |row: &sqlx::postgres::PgRow, col: &str| -> Option<i64> {
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
            } else {
                char_len.map(|l| l.to_string())
            };

            let is_auto_increment = row
                .try_get::<Option<String>, _>("column_default")
                .map(|v| v.map(|s| s.contains("nextval(")).unwrap_or(false))
                .unwrap_or(false);

            columns.push(TableColumn {
                name: row.try_get("column_name").unwrap_or_default(),
                data_type: row.try_get("data_type").unwrap_or_default(),
                is_nullable: row.try_get::<String, _>("is_nullable").unwrap_or_default() == "YES",
                is_primary_key: row.try_get("is_primary").unwrap_or_default(),
                is_auto_increment,
                default_value: row.try_get("column_default").ok(),
                comment: row.try_get("column_comment").ok(),
                length,
                collation: None,
            });
        }

        Ok(TableDefinition {
            name: table_name.to_string(),
            columns,
            catalog,
            schema: Some(target_schema),
            comment: None,
            collation: None,
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

    /// Fetches the DDL (CREATE OR REPLACE) for a routine using pg_get_functiondef.
    async fn get_routine_definition(
        &self,
        name: &str,
        routine_type: &str,
        catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<RoutineDefinition, String> {
        let target_schema = schema.clone().unwrap_or_else(|| "public".to_string());

        let query = "
            SELECT pg_get_functiondef(p.oid)
            FROM pg_proc p
            JOIN pg_namespace n ON n.oid = p.pronamespace
            WHERE p.proname = $1 AND n.nspname = $2
        ";

        // Postgres can have multiple functions with the same name (overloading).
        // For now, we take the first one. A better way would be to include arguments in the name.
        let row = sqlx::query(query)
            .bind(name)
            .bind(&target_schema)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        let definition: String = row.try_get(0).unwrap_or_default();

        Ok(RoutineDefinition {
            name: name.to_string(),
            routine_type: routine_type.to_string(),
            definition,
            catalog,
            schema: Some(target_schema),
        })
    }

    async fn save_routine(&self, definition: RoutineDefinition) -> Result<(), String> {
        if let Some(schema) = &definition.schema {
            sqlx::query(&format!("SET search_path TO \"{}\"", schema))
                .execute(&self.pool)
                .await
                .map_err(|e| format!("Failed to set schema context: {}", e))?;
        }
        sqlx::query(&definition.definition)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Failed to save routine: {}", e))?;
        Ok(())
    }

    /// Fetches the DDL (CREATE OR REPLACE VIEW) for a view using pg_get_viewdef.
    async fn get_view_definition(
        &self,
        name: &str,
        catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<ViewDefinition, String> {
        let target_schema = schema.clone().unwrap_or_else(|| "public".to_string());
        let full_name = format!("\"{}\".\"{}\"", target_schema, name);

        let query = "SELECT pg_get_viewdef($1::regclass, true)";
        let row = sqlx::query(query)
            .bind(&full_name)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        let body: String = row.try_get(0).unwrap_or_default();

        let definition = format!("CREATE OR REPLACE VIEW {} AS\n{}", full_name, body);

        Ok(ViewDefinition {
            name: name.to_string(),
            definition,
            catalog,
            schema: Some(target_schema),
        })
    }

    async fn save_view(&self, definition: ViewDefinition) -> Result<(), String> {
        if let Some(schema) = &definition.schema {
            sqlx::query(&format!("SET search_path TO \"{}\"", schema))
                .execute(&self.pool)
                .await
                .map_err(|e| format!("Failed to set schema context: {}", e))?;
        }
        sqlx::query(&definition.definition)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Failed to save view: {}", e))?;
        Ok(())
    }

    async fn close(&self) {
        self.pool.close().await;
    }
}
