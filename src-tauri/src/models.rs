use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum DbType {
    #[serde(rename = "mySQL")]
    MySQL,
    #[serde(rename = "postgreSQL")]
    PostgreSQL,
    #[serde(rename = "sqlServer")]
    SQLServer,
    #[serde(rename = "sqlite")]
    SQLite,
    #[serde(rename = "oracle")]
    Oracle,
    #[serde(rename = "mongoDB")]
    MongoDB,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionConfig {
    pub id: String,
    pub name: String,
    pub db_type: DbType,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: Option<String>,
    pub database: Option<String>,
    pub ssl_mode: Option<String>,
    pub server_version: Option<String>,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub environment: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub column_types: Vec<String>,
    pub rows: Vec<serde_json::Value>,
    pub affected_rows: u64,
    pub execution_time_ms: u128,
    pub primary_keys: Vec<String>,
    pub table_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ColumnInfo {
    pub name: String,
    #[serde(alias = "data_type")]
    pub data_type: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DbObject {
    pub name: String,
    #[serde(alias = "object_type")]
    pub object_type: String, // "table", "view", "procedure", "function"
    pub schema: Option<String>,
    pub catalog: Option<String>,
    pub description: Option<String>,
    pub parent: Option<String>, // To help build tree if needed
    pub columns: Option<Vec<ColumnInfo>>,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TableColumn {
    pub name: String,
    pub data_type: String,
    pub is_nullable: bool,
    pub is_primary_key: bool,
    pub is_auto_increment: bool,
    pub default_value: Option<String>,
    pub comment: Option<String>,
    pub length: Option<String>,
    pub collation: Option<String>,
}

fn deserialize_optional_i64<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v: serde_json::Value = serde::Deserialize::deserialize(deserializer)?;
    match v {
        serde_json::Value::Number(n) => Ok(n.as_i64()),
        serde_json::Value::String(s) if s.is_empty() => Ok(None),
        serde_json::Value::String(s) => {
            s.parse::<i64>().map(Some).map_err(serde::de::Error::custom)
        }
        serde_json::Value::Null => Ok(None),
        _ => Ok(None),
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TableDefinition {
    pub name: String,
    pub columns: Vec<TableColumn>,
    pub catalog: Option<String>,
    pub schema: Option<String>,
    pub comment: Option<String>,
    pub collation: Option<String>,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RoutineDefinition {
    pub name: String,
    pub routine_type: String, // "procedure" or "function"
    pub definition: String,
    pub catalog: Option<String>,
    pub schema: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ViewDefinition {
    pub name: String,
    pub definition: String,
    pub catalog: Option<String>,
    pub schema: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ScriptInfo {
    pub name: String,
    pub path: String,
    pub database: Option<String>,
    pub schema: Option<String>,
    pub created_at: String,  // ISO string
    pub modified_at: String, // ISO string
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilePreview {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub sheets: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportConfig {
    pub connection_id: String,
    pub file_path: String,
    pub target_table: String,
    pub catalog: Option<String>,
    pub schema: Option<String>,
    pub column_mappings: std::collections::HashMap<String, String>, // Source -> Target
    pub column_types: std::collections::HashMap<String, String>,    // Source -> Type
    pub sheet_name: Option<String>,
    pub has_header: bool,
    pub delimiter: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SheetMapping {
    pub sheet_name: String,
    pub target_table: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MultiSheetImportConfig {
    pub connection_id: String,
    pub file_path: String,
    pub catalog: Option<String>,
    pub schema: Option<String>,
    pub sheet_mappings: Vec<SheetMapping>,
    pub has_header: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExportConfig {
    pub connection_id: String,
    pub source_type: String,         // "table", "query", "current", or "multi"
    pub source_name: Option<String>, // table name
    pub catalog: Option<String>,
    pub schema: Option<String>,
    pub query: Option<String>,
    pub data: Option<Vec<serde_json::Value>>,
    pub output_format: String, // "csv", "excel", "sql"
    pub output_path: String,
    pub columns: Option<Vec<String>>,
    pub source_tables: Option<Vec<String>>, // For multi-table Excel export
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub app_data_path: Option<String>,
    pub schema_cache_ttl: Option<u32>, // in minutes
    pub ai_mode: Option<String>,       // "builtin", "integrated", "cloud"
    pub ollama_url: Option<String>,
    pub ollama_model: Option<String>,
    pub cloud_provider: Option<String>,
    pub cloud_api_key: Option<String>,
    pub cloud_model: Option<String>,
    pub cloud_base_url: Option<String>, // custom base URL for OpenAI-compatible endpoints
    // AI schema context: comma/newline separated name patterns to exclude from the schema
    // sent to the model (e.g. "backup, *_bak, pre_delete*") to reduce input tokens.
    pub ai_exclude_patterns: Option<String>,
    // Query History Retention
    pub history_max_total: Option<u32>,
    pub history_max_per_connection: Option<u32>,
    pub history_max_lifetime_days: Option<u32>,
    pub history_max_lifetime_hours: Option<u32>,
    pub history_max_lifetime_minutes: Option<u32>,
    pub enable_history_retention_total: Option<bool>,
    pub enable_history_retention_per_connection: Option<bool>,
    pub enable_history_retention_lifetime: Option<bool>,
}

/// Result of an AI SQL generation call, including token usage and timing for review.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AiSqlResult {
    pub sql: String,
    pub tokens_in: u32,
    pub tokens_out: u32,
    pub time_ms: u64,
    pub model: String,
    pub table_count: u32, // number of tables/views included in the schema context
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RowActionResult {
    pub affected_rows: u64,
    pub query: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TableIndex {
    pub name: String,
    pub index_type: String, // "PRIMARY", "UNIQUE", "INDEX"
    pub method: String,     // "BTREE", "HASH", "FULLTEXT", etc.
    pub columns: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TableForeignKey {
    pub name: String,
    pub columns: Vec<String>,
    pub referenced_table: String,
    pub referenced_columns: Vec<String>,
    pub on_update: String,
    pub on_delete: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TableIndexInfo {
    pub indexes: Vec<TableIndex>,
    pub foreign_keys: Vec<TableForeignKey>,
}
