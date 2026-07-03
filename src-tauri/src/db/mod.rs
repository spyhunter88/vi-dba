use async_trait::async_trait;
use std::collections::HashMap;
use crate::models::{QueryResult, DbObject, TableDefinition, RoutineDefinition, ViewDefinition, RowActionResult, TableIndexInfo};

/// The core trait that all database drivers must implement.
/// This provides a unified interface for the frontend to interact with different database systems.
#[async_trait]
pub trait Database: Send + Sync {
    /// Executes a SQL query and returns the result, including column metadata and rows.
    async fn execute_query(&self, query: &str, table_name: Option<String>, catalog: Option<String>, schema: Option<String>) -> Result<QueryResult, String>;
    
    /// Returns a list of all tables for the specified catalog/schema.
    async fn get_table_list(&self, catalog: Option<String>, schema: Option<String>) -> Result<QueryResult, String>;
    
    /// Returns a list of all routines (functions/procedures) for the specified catalog/schema.
    async fn get_routine_list(&self, catalog: Option<String>, schema: Option<String>) -> Result<QueryResult, String>;
    
    /// Retrieves a hierarchical list of all database objects (used for sidebar tree).
    async fn get_objects(&self) -> Result<Vec<DbObject>, String>;
    
    /// Updates a single cell value in the database.
    async fn update_row(&self, table_name: &str, pks: HashMap<String, serde_json::Value>, column: &str, value: serde_json::Value, catalog: Option<String>, schema: Option<String>) -> Result<RowActionResult, String>;
    
    /// Inserts a new row with the provided column-value data.
    async fn insert_row(&self, table_name: &str, data: HashMap<String, serde_json::Value>, catalog: Option<String>, schema: Option<String>) -> Result<RowActionResult, String>;
    
    /// Fetches the structure/metadata of a specific table.
    async fn get_table_definition(&self, table_name: &str, catalog: Option<String>, schema: Option<String>) -> Result<TableDefinition, String>;
    
    /// Creates a table based on the provided definition.
    async fn create_table(&self, definition: TableDefinition) -> Result<(), String>;
    
    /// Applies changes to a table by comparing two definitions.
    async fn alter_table(&self, old: TableDefinition, new: TableDefinition) -> Result<(), String>;
    
    /// Generates the SQL required to transform one table definition into another.
    async fn generate_table_sql(&self, old: Option<TableDefinition>, new: TableDefinition) -> Result<String, String>;
    
    /// Retrieves the source code/definition of a specific routine.
    async fn get_routine_definition(&self, name: &str, routine_type: &str, catalog: Option<String>, schema: Option<String>) -> Result<RoutineDefinition, String>;
    
    /// Saves or updates a routine's definition.
    async fn save_routine(&self, definition: RoutineDefinition) -> Result<(), String>;
    
    /// Retrieves the SQL definition of a view.
    async fn get_view_definition(&self, name: &str, catalog: Option<String>, schema: Option<String>) -> Result<ViewDefinition, String>;
    
    /// Saves or updates a view's definition.
    async fn save_view(&self, definition: ViewDefinition) -> Result<(), String>;

    /// Returns indexes and foreign keys for a specific table.
    async fn get_table_indexes(&self, _table_name: &str, _catalog: Option<String>, _schema: Option<String>) -> Result<TableIndexInfo, String> {
        Ok(TableIndexInfo { indexes: vec![], foreign_keys: vec![] })
    }

    /// Closes all active connections for this driver.
    async fn close(&self);
}

pub mod mysql;
pub mod postgres;
pub mod sqlite;
pub mod sqlserver;
#[cfg(feature = "oracle")]
pub mod oracle;
pub mod mongo;
pub mod schema_cache_manager;
pub mod utils;
