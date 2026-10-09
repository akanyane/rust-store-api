use serde::Deserialize;
use surrealdb::types::{RecordId, SurrealValue};

#[derive(Debug, Clone, SurrealValue)]
pub struct CartItemRecord {
    pub id: RecordId,
    pub cart: RecordId,
    pub variant: RecordId,
    pub quantity: i32,
}

#[derive(Debug, Deserialize)]
pub struct AddCartItem {
    pub variant_id: String,
    pub quantity: i32,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCartItem {
    pub quantity: i32,
}

#[derive(Debug, SurrealValue)]
pub struct NewCartItem {
    pub cart: RecordId,
    pub variant: RecordId,
    pub quantity: i32,
}
