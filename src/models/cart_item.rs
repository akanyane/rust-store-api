use serde::Deserialize;
use surrealdb::types::{RecordId, SurrealValue};
use validator::Validate;

#[derive(Debug, Clone, SurrealValue)]
pub struct CartItemRecord {
    pub id: RecordId,
    pub cart: RecordId,
    pub variant: RecordId,
    pub quantity: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct AddCartItem {
    pub variant_id: String,
    #[validate(range(min = 1, message = "must be at least 1"))]
    pub quantity: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateCartItem {
    #[validate(range(min = 1, message = "must be at least 1"))]
    pub quantity: i32,
}

#[derive(Debug, SurrealValue)]
pub struct NewCartItem {
    pub cart: RecordId,
    pub variant: RecordId,
    pub quantity: i32,
}
