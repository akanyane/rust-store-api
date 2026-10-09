use surrealdb::types::RecordId;

use crate::executor::Executor;
use crate::models::product::{NewProduct, ProductRecord};

pub async fn find_all(ex: &Executor<'_>) -> surrealdb::Result<Vec<ProductRecord>> {
    ex.select_all("product").await
}

pub async fn find_by_id(ex: &Executor<'_>, id: &str) -> surrealdb::Result<Option<ProductRecord>> {
    ex.select_one(RecordId::new("product", id.to_string()))
        .await
}

pub async fn find_by_ids(
    ex: &Executor<'_>,
    ids: Vec<RecordId>,
) -> surrealdb::Result<Vec<ProductRecord>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    ex.query_all("SELECT * FROM product WHERE id IN $ids", "ids", ids)
        .await
}

pub async fn create(
    ex: &Executor<'_>,
    data: NewProduct,
) -> surrealdb::Result<Option<ProductRecord>> {
    ex.create("product", data).await
}
