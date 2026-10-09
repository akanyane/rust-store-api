use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};

use super::id_to_string;
use super::variant::Variant;

#[derive(Debug, Clone, SurrealValue)]
pub struct ProductRecord {
    pub id: RecordId,
    pub name: String,
    pub description: String,
    pub active: bool,
}

#[derive(Debug, Serialize)]
pub struct Product {
    pub id: String,
    pub name: String,
    pub description: String,
    pub active: bool,
}

#[derive(Debug, Serialize)]
pub struct ProductDetail {
    pub id: String,
    pub name: String,
    pub description: String,
    pub active: bool,
    pub variants: Vec<Variant>,
}

impl ProductDetail {
    pub fn new(product: Product, variants: Vec<Variant>) -> Self {
        ProductDetail {
            id: product.id,
            name: product.name,
            description: product.description,
            active: product.active,
            variants,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateProduct {
    pub name: String,
    pub description: String,
    pub price_cents: i64,
    pub stock: i32,
}

#[derive(Debug, SurrealValue)]
pub struct NewProduct {
    pub name: String,
    pub description: String,
}

impl From<ProductRecord> for Product {
    fn from(record: ProductRecord) -> Self {
        Product {
            id: id_to_string(&record.id),
            name: record.name,
            description: record.description,
            active: record.active,
        }
    }
}
