use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};

use super::id_to_string;

#[derive(Debug, Clone, SurrealValue)]
pub struct VariantRecord {
    pub id: RecordId,
    pub product: RecordId,
    pub name: String,
    pub sku: String,
    pub price_cents: i64,
    pub stock: i32,
    pub active: bool,
}

#[derive(Debug, Serialize)]
pub struct Variant {
    pub id: String,
    pub product_id: String,
    pub name: String,
    pub sku: String,
    pub price_cents: i64,
    pub stock: i32,
    pub active: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateVariant {
    pub name: String,
    pub sku: String,
    pub price_cents: i64,
    pub stock: i32,
}

#[derive(Debug, SurrealValue)]
pub struct NewVariant {
    pub product: RecordId,
    pub name: String,
    pub sku: String,
    pub price_cents: i64,
    pub stock: i32,
}

impl From<VariantRecord> for Variant {
    fn from(record: VariantRecord) -> Self {
        Variant {
            id: id_to_string(&record.id),
            product_id: id_to_string(&record.product),
            name: record.name,
            sku: record.sku,
            price_cents: record.price_cents,
            stock: record.stock,
            active: record.active,
        }
    }
}
