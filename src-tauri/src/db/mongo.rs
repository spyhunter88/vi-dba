use async_trait::async_trait;
use bson::Document;
use futures::stream::TryStreamExt;
use mongodb::{Client, options::ClientOptions};
use std::collections::HashMap;
use crate::models::{DbObject, QueryResult, RoutineDefinition, RowActionResult, TableDefinition, ViewDefinition};
use super::Database;

pub struct MongoDatabase {
    client: Client,
    db_name: String,
}

impl MongoDatabase {
    pub async fn new(uri: &str, db_name: &str) -> Result<Self, String> {
        let mut client_options = ClientOptions::parse(uri).await.map_err(|e| e.to_string())?;
        client_options.app_name = Some("ViDBConnect".to_string());
        let client = Client::with_options(client_options).map_err(|e| e.to_string())?;
        Ok(Self { client, db_name: db_name.to_string() })
    }

    fn doc_to_json(doc: Document) -> serde_json::Value {
        match serde_json::to_value(doc) {
            Ok(val) => val,
            Err(e) => {
                log::error!("Failed to convert BSON document to JSON: {}", e);
                serde_json::Value::String(format!("[Serialization Error: {}]", e))
            }
        }
    }

    fn json_to_bson(val: &serde_json::Value) -> Result<bson::Bson, String> {
        bson::to_bson(val).map_err(|e| e.to_string())
    }

    fn infer_bson_type(val: &bson::Bson) -> &'static str {
        match val {
            bson::Bson::String(_) => "String",
            bson::Bson::Int32(_) => "Int32",
            bson::Bson::Int64(_) => "Int64",
            bson::Bson::Double(_) => "Double",
            bson::Bson::Boolean(_) => "Boolean",
            bson::Bson::DateTime(_) => "DateTime",
            bson::Bson::ObjectId(_) => "ObjectId",
            bson::Bson::Array(_) => "Array",
            bson::Bson::Document(_) => "Document",
            bson::Bson::Null => "Null",
            _ => "BSON",
        }
    }
}

#[async_trait]
impl Database for MongoDatabase {
    /// Executes a MongoDB query. The query string is the collection name, optionally
    /// followed by a JSON filter: `users` or `users {"status":"active"}`.
    async fn execute_query(
        &self,
        query: &str,
        _table_name: Option<String>,
        catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<QueryResult, String> {
        let start = std::time::Instant::now();
        let target_db = catalog.or(schema).unwrap_or_else(|| self.db_name.clone());
        let db = self.client.database(&target_db);

        let query = query.trim();
        let (collection_name, filter) = if let Some(space_pos) = query.find(' ') {
            let name = &query[..space_pos];
            let filter_str = query[space_pos..].trim();
            let filter_doc = serde_json::from_str::<serde_json::Value>(filter_str)
                .ok()
                .and_then(|v| bson::to_document(&v).ok())
                .unwrap_or_default();
            (name, filter_doc)
        } else {
            (query, Document::new())
        };

        let collection = db.collection::<Document>(collection_name);
        let mut cursor = collection.find(filter).await.map_err(|e| e.to_string())?;

        let mut rows = Vec::new();
        let mut columns: Vec<String> = Vec::new();
        let mut column_types: Vec<String> = Vec::new();

        while let Some(doc) = cursor.try_next().await.map_err(|e| e.to_string())? {
            if rows.is_empty() {
                for (key, val) in &doc {
                    columns.push(key.clone());
                    column_types.push(Self::infer_bson_type(val).to_string());
                }
            }
            rows.push(Self::doc_to_json(doc));
        }

        Ok(QueryResult {
            columns,
            column_types,
            rows,
            affected_rows: 0,
            execution_time_ms: start.elapsed().as_millis(),
            primary_keys: vec!["_id".to_string()],
            table_name: Some(collection_name.to_string()),
        })
    }

    async fn get_table_list(
        &self,
        _catalog: Option<String>,
        _schema: Option<String>,
    ) -> Result<QueryResult, String> {
        let db = self.client.database(&self.db_name);
        let collections = db.list_collection_names().await.map_err(|e| e.to_string())?;
        let rows = collections.into_iter().map(|name| serde_json::json!({ "Name": name })).collect();
        Ok(QueryResult {
            columns: vec!["Name".to_string()],
            column_types: vec!["TEXT".to_string()],
            rows,
            affected_rows: 0,
            execution_time_ms: 0,
            primary_keys: vec![],
            table_name: None,
        })
    }

    async fn get_routine_list(
        &self,
        _catalog: Option<String>,
        _schema: Option<String>,
    ) -> Result<QueryResult, String> {
        Ok(QueryResult {
            columns: vec!["Name".to_string()],
            column_types: vec!["TEXT".to_string()],
            rows: vec![],
            affected_rows: 0,
            execution_time_ms: 0,
            primary_keys: vec![],
            table_name: None,
        })
    }

    async fn get_objects(&self) -> Result<Vec<DbObject>, String> {
        let db = self.client.database(&self.db_name);
        let collections = db.list_collection_names().await.map_err(|e| e.to_string())?;
        Ok(collections.into_iter().map(|name| DbObject {
            name,
            object_type: "collection".to_string(),
            schema: None,
            catalog: None,
            columns: None,
            parent: None,
            description: None,
        }).collect())
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
        let target_db = catalog.or(schema).unwrap_or_else(|| self.db_name.clone());
        let db = self.client.database(&target_db);
        let collection = db.collection::<Document>(table_name);

        let mut filter = Document::new();
        for (key, val) in &pks {
            filter.insert(key.clone(), Self::json_to_bson(val)?);
        }

        let bson_value = Self::json_to_bson(&value)?;
        let update = bson::doc! { "$set": { column: bson_value } };

        let result = collection
            .update_one(filter.clone(), update)
            .await
            .map_err(|e| e.to_string())?;

        let query_str = format!(
            "db.{}.updateOne({}, {{\"$set\": {{\"{}\":...}}}})",
            table_name,
            serde_json::to_string(&pks).unwrap_or_default(),
            column
        );

        Ok(RowActionResult {
            affected_rows: result.modified_count,
            query: query_str,
        })
    }

    async fn insert_row(
        &self,
        table_name: &str,
        data: HashMap<String, serde_json::Value>,
        catalog: Option<String>,
        schema: Option<String>,
    ) -> Result<RowActionResult, String> {
        let target_db = catalog.or(schema).unwrap_or_else(|| self.db_name.clone());
        let db = self.client.database(&target_db);
        let collection = db.collection::<Document>(table_name);

        let mut doc = Document::new();
        for (key, val) in &data {
            doc.insert(key.clone(), Self::json_to_bson(val)?);
        }

        let query_str = format!(
            "db.{}.insertOne({})",
            table_name,
            serde_json::to_string(&data).unwrap_or_default()
        );

        collection.insert_one(doc).await.map_err(|e| e.to_string())?;

        Ok(RowActionResult {
            affected_rows: 1,
            query: query_str,
        })
    }

    async fn get_table_definition(
        &self,
        table_name: &str,
        _catalog: Option<String>,
        _schema: Option<String>,
    ) -> Result<TableDefinition, String> {
        Ok(TableDefinition {
            name: table_name.to_string(),
            columns: vec![],
            catalog: None,
            schema: None,
            comment: None,
            collation: None,
        })
    }

    async fn create_table(&self, definition: TableDefinition) -> Result<(), String> {
        let db = self.client.database(&self.db_name);
        db.create_collection(&definition.name).await.map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn alter_table(&self, _old: TableDefinition, _new: TableDefinition) -> Result<(), String> {
        Ok(())
    }

    async fn generate_table_sql(
        &self,
        _old: Option<TableDefinition>,
        new: TableDefinition,
    ) -> Result<String, String> {
        Ok(format!("db.createCollection(\"{}\")", new.name))
    }

    async fn get_routine_definition(
        &self,
        _name: &str,
        _routine_type: &str,
        _catalog: Option<String>,
        _schema: Option<String>,
    ) -> Result<RoutineDefinition, String> {
        Err("MongoDB does not support stored routines".to_string())
    }

    async fn save_routine(&self, _definition: RoutineDefinition) -> Result<(), String> {
        Err("MongoDB does not support stored routines".to_string())
    }

    async fn get_view_definition(
        &self,
        _name: &str,
        _catalog: Option<String>,
        _schema: Option<String>,
    ) -> Result<ViewDefinition, String> {
        Err("MongoDB does not support SQL views".to_string())
    }

    async fn save_view(&self, _definition: ViewDefinition) -> Result<(), String> {
        Err("MongoDB does not support SQL views".to_string())
    }

    async fn close(&self) {
        // MongoDB client handles pooling internally
    }
}
