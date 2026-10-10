use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};
use validator::Validate;

use super::id_to_string;

#[derive(Debug, Clone, SurrealValue)]
pub struct VariantRecord {
    pub id: RecordId,
    pub product: RecordId,
    pub name: String,
    pub sku: String,
    pub price: i64,
    pub stock: i32,
    pub active: bool,
}

#[derive(Debug, Serialize)]
pub struct Variant {
    pub id: String,
    pub product_id: String,
    pub name: String,
    pub sku: String,
    pub price: i64,
    pub stock: i32,
    pub active: bool,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateVariant {
    #[validate(
        custom(
            function = "crate::models::validate_not_blank",
            message = "must not be blank"
        ),
        length(max = 200, message = "at most 200 characters")
    )]
    pub name: String,
    #[validate(
        custom(
            function = "crate::models::validate_not_blank",
            message = "must not be blank"
        ),
        length(max = 64, message = "at most 64 characters")
    )]
    pub sku: String,
    #[validate(range(min = 0, message = "must not be negative"))]
    pub price: i64,
    #[validate(range(min = 0, message = "must not be negative"))]
    pub stock: i32,
}

#[derive(Debug, SurrealValue)]
pub struct NewVariant {
    pub product: RecordId,
    pub name: String,
    pub sku: String,
    pub price: i64,
    pub stock: i32,
}

impl From<VariantRecord> for Variant {
    fn from(record: VariantRecord) -> Self {
        Variant {
            id: id_to_string(&record.id),
            product_id: id_to_string(&record.product),
            name: record.name,
            sku: record.sku,
            price: record.price,
            stock: record.stock,
            active: record.active,
        }
    }
}
