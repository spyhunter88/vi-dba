use crate::models::{ColumnInfo, DbObject};
use sqlx::{MySqlPool, Row};
use std::collections::HashMap;

/// Entry point for MySQL 8.4+ object discovery.
/// MySQL 8.4 uses the Transactional Data Dictionary, which backs information_schema
/// with real InnoDB tables. Queries use literal schema names in WHERE clauses
/// (not parameterized) to let the optimizer push the predicate into the dictionary lookup.
pub async fn get_objects_v84(
    pool: &MySqlPool,
    default_db: Option<String>,
) -> Result<Vec<DbObject>, String> {
    let mut effective_db = default_db
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    if effective_db.is_none() {
        // Fallback: Check if there's a current database selected in the session
        // This is crucial for MySQL 8.4 where the connection state might have a DB
        // not explicitly tracked in the initial config.
        if let Ok(row) = sqlx::query("SELECT DATABASE() as db").fetch_one(pool).await {
            effective_db = row
                .try_get::<Option<String>, _>("db")
                .or_else(|_| {
                    row.try_get::<Option<Vec<u8>>, _>("db")
                        .map(|opt| opt.map(|b| String::from_utf8_lossy(&b).to_string()))
                })
                .ok()
                .flatten();
        }
    }

    if let Some(db) = effective_db {
        get_objects_for_db(pool, &db).await
    } else {
        get_objects_all_dbs(pool).await
    }
}

async fn get_objects_for_db(pool: &MySqlPool, db: &str) -> Result<Vec<DbObject>, String> {
    let db_str = db.to_string();
    let db_lit = db.replace('\'', "''"); // escape single quotes for inline literal
    let mut objects: Vec<DbObject> = Vec::new();

    // Tables
    let table_rows = sqlx::query(&format!(
        "SELECT TABLE_NAME as n, TABLE_COMMENT as c
         FROM information_schema.TABLES
         WHERE TABLE_SCHEMA = '{db_lit}' AND TABLE_TYPE = 'BASE TABLE'
         ORDER BY TABLE_NAME"
    ))
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    // Views
    let view_rows = sqlx::query(&format!(
        "SELECT TABLE_NAME as n
         FROM information_schema.TABLES
         WHERE TABLE_SCHEMA = '{db_lit}' AND TABLE_TYPE = 'VIEW'
         ORDER BY TABLE_NAME"
    ))
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    // Columns
    let column_rows = sqlx::query(&format!(
        "SELECT TABLE_NAME as t, COLUMN_NAME as c, DATA_TYPE as d
         FROM information_schema.COLUMNS
         WHERE TABLE_SCHEMA = '{db_lit}'
         ORDER BY TABLE_NAME, ORDINAL_POSITION"
    ))
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    // Routines
    let routine_rows = sqlx::query(&format!(
        "SELECT ROUTINE_NAME as n, ROUTINE_TYPE as t
         FROM information_schema.ROUTINES
         WHERE ROUTINE_SCHEMA = '{db_lit}'
         ORDER BY ROUTINE_NAME"
    ))
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let mut column_map: HashMap<String, Vec<ColumnInfo>> = HashMap::new();
    for row in column_rows {
        let table: String = row
            .try_get::<String, _>("t")
            .or_else(|_| {
                row.try_get::<Vec<u8>, _>("t")
                    .map(|b| String::from_utf8_lossy(&b).to_string())
            })
            .unwrap_or_default();
        let name: String = row
            .try_get::<String, _>("c")
            .or_else(|_| {
                row.try_get::<Vec<u8>, _>("c")
                    .map(|b| String::from_utf8_lossy(&b).to_string())
            })
            .unwrap_or_default();
        let data_type: String = row
            .try_get::<String, _>("d")
            .or_else(|_| {
                row.try_get::<Vec<u8>, _>("d")
                    .map(|b| String::from_utf8_lossy(&b).to_string())
            })
            .unwrap_or_default();

        if table.is_empty() {
            continue;
        }
        column_map
            .entry(table)
            .or_default()
            .push(ColumnInfo { name, data_type });
    }

    for row in &table_rows {
        let name: String = row
            .try_get::<String, _>("n")
            .or_else(|_| {
                row.try_get::<Vec<u8>, _>("n")
                    .map(|b| String::from_utf8_lossy(&b).to_string())
            })
            .unwrap_or_default();

        if name.is_empty() {
            continue;
        }

        let description: Option<String> =
            row.try_get::<Option<String>, _>("c").unwrap_or_else(|_| {
                row.try_get::<Option<Vec<u8>>, _>("c")
                    .ok()
                    .flatten()
                    .map(|b| String::from_utf8_lossy(&b).to_string())
            });
        let columns = column_map.remove(&name);

        objects.push(DbObject {
            name,
            object_type: "table".to_string(),
            schema: None,
            catalog: Some(db_str.clone()),
            description,
            parent: None,
            columns,
        });
    }

    for row in &view_rows {
        let name: String = row
            .try_get::<String, _>("n")
            .or_else(|_| {
                row.try_get::<Vec<u8>, _>("n")
                    .map(|b| String::from_utf8_lossy(&b).to_string())
            })
            .unwrap_or_default();

        if name.is_empty() {
            continue;
        }
        let columns = column_map.remove(&name);
        objects.push(DbObject {
            name,
            object_type: "view".to_string(),
            schema: None,
            catalog: Some(db_str.clone()),
            description: None,
            parent: None,
            columns,
        });
    }

    for row in routine_rows {
        let name: String = row
            .try_get::<String, _>("n")
            .or_else(|_| {
                row.try_get::<Vec<u8>, _>("n")
                    .map(|b| String::from_utf8_lossy(&b).to_string())
            })
            .unwrap_or_default();
        let routine_type: String = row
            .try_get::<String, _>("t")
            .or_else(|_| {
                row.try_get::<Vec<u8>, _>("t")
                    .map(|b| String::from_utf8_lossy(&b).to_string())
            })
            .unwrap_or_default();

        if name.is_empty() {
            continue;
        }
        let object_type = match routine_type.to_uppercase().as_str() {
            "PROCEDURE" => "procedure",
            "FUNCTION" => "function",
            _ => continue,
        };
        objects.push(DbObject {
            name,
            object_type: object_type.to_string(),
            schema: None,
            catalog: Some(db_str.clone()),
            description: None,
            parent: None,
            columns: None,
        });
    }

    Ok(objects)
}

async fn get_objects_all_dbs(pool: &MySqlPool) -> Result<Vec<DbObject>, String> {
    let mut objects: Vec<DbObject> = Vec::new();

    let table_rows = sqlx::query(
        "SELECT TABLE_SCHEMA as s, TABLE_NAME as n, TABLE_TYPE as tp, TABLE_COMMENT as c
         FROM information_schema.TABLES
         WHERE TABLE_SCHEMA NOT IN ('information_schema','performance_schema','mysql','sys')
           AND TABLE_TYPE IN ('BASE TABLE','VIEW')
         ORDER BY TABLE_SCHEMA, TABLE_NAME",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let column_rows = sqlx::query(
        "SELECT TABLE_SCHEMA as s, TABLE_NAME as t, COLUMN_NAME as c, DATA_TYPE as d
         FROM information_schema.COLUMNS
         WHERE TABLE_SCHEMA NOT IN ('information_schema','performance_schema','mysql','sys')
         ORDER BY TABLE_SCHEMA, TABLE_NAME, ORDINAL_POSITION",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let routine_rows = sqlx::query(
        "SELECT ROUTINE_SCHEMA as s, ROUTINE_NAME as n, ROUTINE_TYPE as t
         FROM information_schema.ROUTINES
         WHERE ROUTINE_SCHEMA NOT IN ('information_schema','performance_schema','mysql','sys')
         ORDER BY ROUTINE_SCHEMA, ROUTINE_NAME",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let mut column_map: HashMap<(String, String), Vec<ColumnInfo>> = HashMap::new();
    for row in column_rows {
        let db: String = row
            .try_get::<String, _>("s")
            .or_else(|_| {
                row.try_get::<Vec<u8>, _>("s")
                    .map(|b| String::from_utf8_lossy(&b).to_string())
            })
            .unwrap_or_default();
        let table: String = row
            .try_get::<String, _>("t")
            .or_else(|_| {
                row.try_get::<Vec<u8>, _>("t")
                    .map(|b| String::from_utf8_lossy(&b).to_string())
            })
            .unwrap_or_default();
        let name: String = row
            .try_get::<String, _>("c")
            .or_else(|_| {
                row.try_get::<Vec<u8>, _>("c")
                    .map(|b| String::from_utf8_lossy(&b).to_string())
            })
            .unwrap_or_default();
        let data_type: String = row
            .try_get::<String, _>("d")
            .or_else(|_| {
                row.try_get::<Vec<u8>, _>("d")
                    .map(|b| String::from_utf8_lossy(&b).to_string())
            })
            .unwrap_or_default();

        if db.is_empty() || table.is_empty() {
            continue;
        }
        column_map
            .entry((db, table))
            .or_default()
            .push(ColumnInfo { name, data_type });
    }

    for row in &table_rows {
        let db: String = row
            .try_get::<String, _>("s")
            .or_else(|_| {
                row.try_get::<Vec<u8>, _>("s")
                    .map(|b| String::from_utf8_lossy(&b).to_string())
            })
            .unwrap_or_default();
        let name: String = row
            .try_get::<String, _>("n")
            .or_else(|_| {
                row.try_get::<Vec<u8>, _>("n")
                    .map(|b| String::from_utf8_lossy(&b).to_string())
            })
            .unwrap_or_default();
        let tp: String = row
            .try_get::<String, _>("tp")
            .or_else(|_| {
                row.try_get::<Vec<u8>, _>("tp")
                    .map(|b| String::from_utf8_lossy(&b).to_string())
            })
            .unwrap_or_default();

        if name.is_empty() {
            continue;
        }
        let object_type = if tp.contains("VIEW") { "view" } else { "table" };
        let description: Option<String> = if object_type == "table" {
            row.try_get::<Option<String>, _>("c").unwrap_or_else(|_| {
                row.try_get::<Option<Vec<u8>>, _>("c")
                    .ok()
                    .flatten()
                    .map(|b| String::from_utf8_lossy(&b).to_string())
            })
        } else {
            None
        };
        let columns = column_map.remove(&(db.clone(), name.clone()));
        let catalog_obj = if db.is_empty() { None } else { Some(db) };
        objects.push(DbObject {
            name,
            object_type: object_type.to_string(),
            schema: None,
            catalog: catalog_obj,
            description,
            parent: None,
            columns,
        });
    }

    for row in routine_rows {
        let db: String = row
            .try_get::<String, _>("s")
            .or_else(|_| {
                row.try_get::<Vec<u8>, _>("s")
                    .map(|b| String::from_utf8_lossy(&b).to_string())
            })
            .unwrap_or_default();
        let name: String = row
            .try_get::<String, _>("n")
            .or_else(|_| {
                row.try_get::<Vec<u8>, _>("n")
                    .map(|b| String::from_utf8_lossy(&b).to_string())
            })
            .unwrap_or_default();
        let routine_type: String = row
            .try_get::<String, _>("t")
            .or_else(|_| {
                row.try_get::<Vec<u8>, _>("t")
                    .map(|b| String::from_utf8_lossy(&b).to_string())
            })
            .unwrap_or_default();

        if name.is_empty() {
            continue;
        }
        let object_type = match routine_type.to_uppercase().as_str() {
            "PROCEDURE" => "procedure",
            "FUNCTION" => "function",
            _ => continue,
        };
        let catalog_obj = if db.is_empty() { None } else { Some(db) };
        objects.push(DbObject {
            name,
            object_type: object_type.to_string(),
            schema: None,
            catalog: catalog_obj,
            description: None,
            parent: None,
            columns: None,
        });
    }

    Ok(objects)
}
