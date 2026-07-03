use serde_json::Value;

/// Metadata about a parsed SQL query, used for context switching and result processing.
pub struct QueryInfo {
    pub q_trimmed: String,
    pub is_select: bool,
    pub detected_table_name: Option<String>,
}

/// Parses a SQL query string to determine if it's a "SELECT-like" operation 
/// and attempts to extract the table name for primary key lookups.
pub fn parse_query(query: &str, explicit_table: Option<String>) -> QueryInfo {
    let q_trimmed = query.trim().to_uppercase();
    let is_select = q_trimmed.starts_with("SELECT") || 
                   q_trimmed.starts_with("SHOW") || 
                   q_trimmed.starts_with("DESCRIBE") || 
                   q_trimmed.starts_with("EXPLAIN") ||
                   q_trimmed.starts_with("WITH") ||
                   q_trimmed.starts_with("PRAGMA");

    let detected_table_name = if let Some(table) = explicit_table {
        Some(table)
    } else if is_select {
        // Basic table name extraction for SELECT * FROM [table]
        if q_trimmed.starts_with("SELECT * FROM ") || q_trimmed.starts_with("SELECT * FROM `") || q_trimmed.starts_with("SELECT * FROM [") || q_trimmed.starts_with("SELECT * FROM \"") {
            let parts: Vec<&str> = query.split_whitespace().collect();
            if parts.len() >= 4 {
                Some(parts[3].trim_matches(|c| c == '`' || c == '"' || c == '[' || c == ']' || c == ';').to_string())
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    QueryInfo {
        q_trimmed,
        is_select,
        detected_table_name,
    }
}

/// Resolves the effective database/schema context based on priority 
/// (explicitly provided catalog/schema vs connection default).
pub fn get_effective_context(catalog: Option<String>, schema: Option<String>, default: Option<String>) -> Option<String> {
    catalog.filter(|s| !s.is_empty())
        .or(schema.filter(|s| !s.is_empty()))
        .or(default.filter(|s| !s.is_empty()))
}

/// Converts a serde_json::Value into an optional String for SQL binding.
pub fn val_to_opt_string(val: &Value) -> Option<String> {
    match val {
        Value::Null => None,
        Value::String(s) => if s.is_empty() { None } else { Some(s.clone()) },
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => Some(val.to_string()),
    }
}
