use super::{utils, Database};
use crate::models::ColumnInfo;
use crate::models::{
    DbObject, QueryResult, RoutineDefinition, RowActionResult, TableColumn, TableDefinition,
    ViewDefinition,
};
use async_trait::async_trait;
use sqlx::{Column, Row, SqlitePool, TypeInfo};
use std::collections::HashMap;
use std::time::Instant;

/// SQLite implementation of the Database trait.
pub struct SqliteDatabase {
    pub pool: SqlitePool,
}

impl SqliteDatabase {
    /// Creates a new SQLite database instance.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
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

            if let Some(default) = &col.default_value {
                if !default.is_empty() {
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

    /// Generates the SQL statements needed to alter a table.
    /// SQLite doesn't support most ALTER TABLE operations, so this implements
    /// a temporary table pattern (Create New -> Move Data -> Drop old -> Rename).
    fn get_alter_table_sql(&self, old: &TableDefinition, new: &TableDefinition) -> Vec<String> {
        let mut statements = Vec::new();

        // 1. Check if it's a simple rename
        let old_cols_basic: Vec<(String, String)> = old
            .columns
            .iter()
            .map(|c| (c.name.clone(), c.data_type.clone()))
            .collect();
        let new_cols_basic: Vec<(String, String)> = new
            .columns
            .iter()
            .map(|c| (c.name.clone(), c.data_type.clone()))
            .collect();

        if old.name != new.name && old_cols_basic == new_cols_basic {
            statements.push(format!(
                "ALTER TABLE `{}` RENAME TO `{}`",
                old.name, new.name
            ));
            return statements;
        }

        // 2. Complex modification using temp table pattern
        let temp_table_name = format!("_{}_new", new.name);

        // a. Create new table with temporary name
        let mut temp_def = new.clone();
        temp_def.name = temp_table_name.clone();
        statements.push(self.get_create_table_sql(&temp_def));

        // b. Identified common columns
        let old_col_names: std::collections::HashSet<String> =
            old.columns.iter().map(|c| c.name.clone()).collect();
        let common_cols: Vec<String> = new
            .columns
            .iter()
            .filter(|c| old_col_names.contains(&c.name))
            .map(|c| c.name.clone())
            .collect();

        if !common_cols.is_empty() {
            let cols_str = common_cols
                .iter()
                .map(|c| format!("`{}`", c))
                .collect::<Vec<_>>()
                .join(", ");
            statements.push(format!(
                "INSERT INTO `{}` ({}) SELECT {} FROM `{}`",
                temp_table_name, cols_str, cols_str, old.name
            ));
        }

        // c. Drop old table
        statements.push(format!("DROP TABLE `{}`", old.name));

        // d. Rename temp table to final name
        statements.push(format!(
            "ALTER TABLE `{}` RENAME TO `{}`",
            temp_table_name, new.name
        ));

        statements
    }

    /// Converts a SQLite row value into a JSON value based on simple type mapping.
    fn row_to_json(&self, row: &sqlx::sqlite::SqliteRow, col: &str) -> serde_json::Value {
        let col_info = row.column(col);
        let type_info = col_info.type_info();
        let type_name = type_info.name();

        let result = match type_name {
            "BOOLEAN" | "BOOL" => row.try_get::<bool, _>(col).map(|v| v.into()),
            "INTEGER" | "INT" | "BIGINT" | "TINYINT" | "SMALLINT" => row
                .try_get::<i64, _>(col)
                .map(|v| v.into())
                .or_else(|_| row.try_get::<u64, _>(col).map(|v| v.into())),
            "REAL" | "FLOAT" | "DOUBLE" => row.try_get::<f64, _>(col).map(|v| {
                serde_json::Number::from_f64(v)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null)
            }),
            "TEXT" | "VARCHAR" | "CHAR" => row.try_get::<String, _>(col).map(|v| v.into()),
            "BLOB" => row
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
                if err_str.contains("UnexpectedNullError") || err_str.to_lowercase().contains("is null") {
                    return serde_json::Value::Null;
                }

                if let Ok(v) = row.try_get::<String, _>(col) {
                    return v.into();
                }

                log::error!(
                    "Failed to decode SQLite column '{}' (type: {}): {}",
                    col,
                    type_name,
                    e
                );
                serde_json::Value::String(format!("[Decode Error: {}]", e))
            }
        }
    }

    /// Processes a list of SqliteRows into a QueryResult, including column types.
    fn process_rows(
        &self,
        rows: Vec<sqlx::sqlite::SqliteRow>,
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

    /// Fetches the primary key column names for a given table using PRAGMA table_info.
    async fn get_primary_keys(&self, table: &str) -> Result<Vec<String>, String> {
        let q = format!("PRAGMA table_info(`{}`)", table);
        let rows = sqlx::query(&q)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(rows
            .into_iter()
            .filter(|r| r.get::<i32, _>("pk") > 0)
            .map(|r| r.get::<String, _>("name"))
            .collect())
    }

    /// Fetches all column names for a given table using PRAGMA table_info.
    async fn get_columns(&self, table: &str) -> Result<Vec<String>, String> {
        let q = format!("PRAGMA table_info(`{}`)", table);
        let rows = sqlx::query(&q)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(rows
            .into_iter()
            .map(|r| r.get::<String, _>("name"))
            .collect())
    }

    /// Converts a serde_json::Value to a String option, stripping quotes for consistency.
    fn val_to_opt_string(&self, val: &serde_json::Value) -> Option<String> {
        match val {
            serde_json::Value::Null => None,
            serde_json::Value::String(s) => Some(s.clone()),
            _ => Some(val.to_string().trim_matches('"').to_string()),
        }
    }

    /// Runs a single statement on an already-acquired connection. Shared by `execute_query`
    /// and `execute_script` so that a whole script runs on one connection (temporary tables
    /// and transactions in SQLite are connection-scoped).
    async fn exec_stmt(
        &self,
        conn: &mut sqlx::SqliteConnection,
        query: &str,
        table_name: Option<String>,
    ) -> Result<QueryResult, String> {
        let start = Instant::now();
        let info = utils::parse_query(query, table_name);

        if info.is_select {
            let pks = if let Some(table) = &info.detected_table_name {
                self.get_primary_keys(table).await.unwrap_or_default()
            } else {
                vec![]
            };

            let rows = sqlx::query(query)
                .fetch_all(&mut *conn)
                .await
                .map_err(|e| e.to_string())?;
            let fallback = if rows.is_empty() {
                if let Some(table) = &info.detected_table_name {
                    self.get_columns(table).await.unwrap_or_default()
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
impl Database for SqliteDatabase {
    /// Executes a SQL query. For SELECTs, it returns rows and metadata.
    /// For other statements, it returns affected rows.
    async fn execute_query(
        &self,
        query: &str,
        table_name: Option<String>,
        _catalog: Option<String>,
        _schema: Option<String>,
    ) -> Result<QueryResult, String> {
        let mut conn = self.pool.acquire().await.map_err(|e| e.to_string())?;
        self.exec_stmt(&mut conn, query, table_name).await
    }

    async fn execute_script(
        &self,
        statements: &[String],
        table_name: Option<String>,
        _catalog: Option<String>,
        _schema: Option<String>,
    ) -> Result<Vec<QueryResult>, String> {
        if statements.is_empty() {
            return Ok(vec![]);
        }

        // Temporary tables and transactions in SQLite live on a single connection, so run
        // the entire script on one acquired connection rather than one-per-statement.
        let mut conn = self.pool.acquire().await.map_err(|e| e.to_string())?;
        let mut results = Vec::with_capacity(statements.len());
        for stmt in statements {
            let r = self
                .exec_stmt(&mut conn, stmt, table_name.clone())
                .await?;
            results.push(r);
        }
        Ok(results)
    }

    /// Fetches a list of all user-defined tables from sqlite_master.
    async fn get_table_list(
        &self,
        _catalog: Option<String>,
        _schema: Option<String>,
    ) -> Result<QueryResult, String> {
        let start = Instant::now();
        let query = "SELECT name as \"Name\", 'table' as \"Type\" FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'";
        let rows = sqlx::query(query)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        self.process_rows(rows, start.elapsed().as_millis(), vec![], None, vec![])
    }

    /// Returns an empty list as SQLite doesn't support stored procedures.
    async fn get_routine_list(
        &self,
        _catalog: Option<String>,
        _schema: Option<String>,
    ) -> Result<QueryResult, String> {
        Ok(QueryResult {
            columns: vec!["Name".to_string(), "Type".to_string()],
            column_types: vec!["TEXT".to_string(), "TEXT".to_string()],
            rows: vec![],
            affected_rows: 0,
            execution_time_ms: 0,
            primary_keys: vec![],
            table_name: None,
        })
    }

    /// Returns a comprehensive list of all schema objects for autocomplete/sidebar.
    async fn get_objects(&self) -> Result<Vec<DbObject>, String> {
        let query = "
            SELECT 
                NULL as catalog, 
                NULL as schema, 
                name, 
                type as object_type
            FROM sqlite_master 
            WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%'
        ";
        let rows = sqlx::query(query)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        let mut objects = Vec::new();
        for row in rows {
            let name: String = row.try_get("name").unwrap_or_default();
            let object_type: String = row.try_get("object_type").unwrap_or_default();

            // Fetch columns for each table/view
            let mut columns = None;
            if object_type == "table" || object_type == "view" {
                let col_query = format!("PRAGMA table_info(`{}`)", name);
                if let Ok(col_rows) = sqlx::query(&col_query).fetch_all(&self.pool).await {
                    let infos: Vec<ColumnInfo> = col_rows
                        .into_iter()
                        .map(|r| ColumnInfo {
                            name: r.try_get::<String, _>("name").unwrap_or_default(),
                            data_type: r.try_get::<String, _>("type").unwrap_or_default(),
                        })
                        .collect();
                    columns = Some(infos);
                }
            }

            objects.push(DbObject {
                name,
                object_type,
                schema: None,
                catalog: None,
                parent: None,
                columns,
                description: None,
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
        _schema: Option<String>,
    ) -> Result<RowActionResult, String> {
        let mut query = format!("UPDATE `{}` SET `{}` = ? WHERE ", table_name, column);
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

        // Reconstruct query for logging
        let mut logged_query = format!(
            "UPDATE `{}` SET `{}` = '{}' WHERE ",
            table_name,
            column,
            val_str
                .unwrap_or_else(|| "NULL".to_string())
                .replace("'", "''")
        );
        let pk_logged: Vec<String> = pk_list
            .iter()
            .map(|(k, v)| {
                format!(
                    "`{}` = '{}'",
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
        _schema: Option<String>,
    ) -> Result<RowActionResult, String> {
        let columns: Vec<String> = data.keys().cloned().collect();
        let placeholders: Vec<String> = vec!["?".to_string(); columns.len()];

        let query = format!(
            "INSERT INTO `{}` ({}) VALUES ({})",
            table_name,
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
                Some(v) => format!("'{}'", v.replace("'", "''")),
                None => "NULL".to_string(),
            });
        }
        let res = q.execute(&self.pool).await.map_err(|e| e.to_string())?;

        let logged_query = format!(
            "INSERT INTO `{}` ({}) VALUES ({})",
            table_name,
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

    /// Fetches the structure of a table, identifying primary keys via PRAGMA table_info.
    async fn get_table_definition(
        &self,
        table_name: &str,
        catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<TableDefinition, String> {
        let q = format!("PRAGMA table_info(`{}`)", table_name);
        let rows = sqlx::query(&q)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        let mut columns = Vec::new();

        for row in rows {
            let raw_type: String = row.try_get("type").unwrap_or_default();
            let mut data_type = raw_type.clone();
            let mut length = None;

            if let Some(start) = raw_type.find('(') {
                if let Some(end) = raw_type.find(')') {
                    data_type = raw_type[..start].trim().to_string();
                    length = Some(raw_type[start + 1..end].to_string());
                }
            }

            columns.push(TableColumn {
                name: row.try_get("name").unwrap_or_default(),
                data_type,
                is_nullable: row.try_get::<i32, _>("notnull").unwrap_or_default() == 0,
                is_primary_key: row.try_get::<i32, _>("pk").unwrap_or_default() > 0,
                is_auto_increment: false, // SQLite auto-increment is complex to detect via PRAGMA
                default_value: row.try_get("dflt_value").ok(),
                comment: None,
                length,
                collation: None,
            });
        }

        Ok(TableDefinition {
            name: table_name.to_string(),
            columns,
            catalog,
            schema,
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

    async fn get_routine_definition(
        &self,
        _name: &str,
        _routine_type: &str,
        _catalog: Option<String>,
        _schema: Option<String>,
    ) -> Result<RoutineDefinition, String> {
        Err("Stored procedures and functions are not supported in SQLite via SQL.".to_string())
    }

    async fn save_routine(&self, _definition: RoutineDefinition) -> Result<(), String> {
        Err("Stored procedures and functions are not supported in SQLite via SQL.".to_string())
    }

    /// Fetches the source definition for a view from sqlite_master.
    async fn get_view_definition(
        &self,
        name: &str,
        catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<ViewDefinition, String> {
        let query = "SELECT sql FROM sqlite_master WHERE type='view' AND name=?";
        let row = sqlx::query(query)
            .bind(name)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        let definition: String = row.try_get(0).unwrap_or_default();

        Ok(ViewDefinition {
            name: name.to_string(),
            definition,
            catalog,
            schema,
        })
    }

    async fn save_view(&self, definition: ViewDefinition) -> Result<(), String> {
        let drop_query = format!("DROP VIEW IF EXISTS \"{}\"", definition.name);
        let mut tx = self.pool.begin().await.map_err(|e| e.to_string())?;
        sqlx::query(&drop_query)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        sqlx::query(&definition.definition)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        tx.commit().await.map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn close(&self) {
        self.pool.close().await;
    }
}
