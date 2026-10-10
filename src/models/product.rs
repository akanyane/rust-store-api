use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{Datetime, RecordId, SurrealValue};
use validator::Validate;

use super::id_to_string;
use super::variant::Variant;

#[derive(Debug, Clone, SurrealValue)]
pub struct ProductRecord {
    pub id: RecordId,
    pub name: String,
    pub description: String,
    pub active: bool,
    pub created_at: Option<Datetime>,
}

#[derive(Debug, Serialize)]
pub struct Product {
    pub id: String,
    pub name: String,
    pub description: String,
    pub active: bool,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct ProductDetail {
    pub id: String,
    pub name: String,
    pub description: String,
    pub active: bool,
    pub created_at: Option<DateTime<Utc>>,
    pub variants: Vec<Variant>,
}

impl ProductDetail {
    pub fn new(product: Product, variants: Vec<Variant>) -> Self {
        ProductDetail {
            id: product.id,
            name: product.name,
            description: product.description,
            active: product.active,
            created_at: product.created_at,
            variants,
        }
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateProduct {
    #[validate(
        custom(
            function = "crate::models::validate_not_blank",
            message = "must not be blank"
        ),
        length(max = 200, message = "at most 200 characters")
    )]
    pub name: String,
    #[validate(length(max = 2000, message = "at most 2000 characters"))]
    pub description: String,
    #[validate(range(min = 0, message = "must not be negative"))]
    pub price: i64,
    #[validate(range(min = 0, message = "must not be negative"))]
    pub stock: i32,
}

#[derive(Debug, SurrealValue)]
pub struct NewProduct {
    pub name: String,
    pub description: String,
    pub created_at: Datetime,
}

impl From<ProductRecord> for Product {
    fn from(record: ProductRecord) -> Self {
        Product {
            id: id_to_string(&record.id),
            name: record.name,
            description: record.description,
            active: record.active,
            created_at: record.created_at.map(DateTime::<Utc>::from),
        }
    }
}

/// Substituição completa (PUT). Preço e estoque vivem nas variantes.
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateProduct {
    #[validate(
        custom(
            function = "crate::models::validate_not_blank",
            message = "must not be blank"
        ),
        length(max = 200, message = "at most 200 characters")
    )]
    pub name: String,
    #[validate(length(max = 2000, message = "at most 2000 characters"))]
    pub description: String,
    pub active: bool,
}

#[derive(Debug, SurrealValue)]
pub struct ProductChanges {
    pub name: String,
    pub description: String,
    pub active: bool,
}

/// Ordem da listagem de admin: `name` (padrão) ou `newest` (mais recentes primeiro; os
/// produtos sem data de criação ficam no fim).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProductSort {
    #[default]
    Name,
    Newest,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ListProductsQuery {
    pub active: Option<bool>,
    pub sort: Option<ProductSort>,
    #[validate(range(min = 1, max = 200, message = "must be between 1 and 200"))]
    pub limit: Option<i64>,
    #[validate(range(min = 0, message = "must not be negative"))]
    pub offset: Option<i64>,
}

/// Parâmetro único da listagem (o `query_all` aceita só um).
#[derive(Debug, SurrealValue)]
pub struct ProductPage {
    pub active: Option<bool>,
    pub limit: i64,
    pub offset: i64,
}
