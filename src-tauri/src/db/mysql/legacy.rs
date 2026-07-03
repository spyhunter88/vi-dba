use sqlx::MySqlPool;
use crate::models::DbObject;
use super::v8;

/// Implementation for MySQL versions older than 8.0 or MariaDB.
/// Currently defaults to common logic but can be specialized if needed.
pub async fn get_objects(pool: &MySqlPool, default_db: Option<String>) -> Result<Vec<DbObject>, String> {
    // For many older versions, the metadata queries are very similar.
    // We reuse the robust v8 logic which uses explicit aliases and parameter pass-through.
    v8::get_objects_v8(pool, default_db).await
}
