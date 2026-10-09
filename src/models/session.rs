use surrealdb::types::{Datetime, RecordId, SurrealValue};

/// Só os hashes dos tokens ficam no banco, nunca os tokens em si.
#[derive(Debug, Clone, SurrealValue)]
pub struct SessionRecord {
    pub id: RecordId,
    pub user: RecordId,
    pub token_hash: String,
    pub refresh_hash: String,
    pub expires_at: Datetime,
    pub refresh_expires_at: Datetime,
}

#[derive(SurrealValue)]
pub struct NewSession {
    pub user: RecordId,
    pub token_hash: String,
    pub refresh_hash: String,
    pub expires_at: Datetime,
    pub refresh_expires_at: Datetime,
}
