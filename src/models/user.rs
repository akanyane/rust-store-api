use surrealdb::types::{Datetime, RecordId, SurrealValue};

pub const ROLE_CUSTOMER: &str = "customer";
pub const ROLE_ADMIN: &str = "admin";

/// Registro completo do banco, com o hash da senha. Nunca é serializado para a API.
#[derive(Debug, Clone, SurrealValue)]
pub struct UserRecord {
    pub id: RecordId,
    pub username: String,
    pub password_hash: String,
    pub role: String,
    pub created_at: Datetime,
}

#[derive(SurrealValue)]
pub struct NewUser {
    pub username: String,
    pub password_hash: String,
    pub role: String,
}
