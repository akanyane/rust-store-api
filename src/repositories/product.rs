use surrealdb::types::RecordId;

use crate::executor::Executor;
use crate::models::product::{NewProduct, ProductChanges, ProductPage, ProductRecord};

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

pub async fn update(
    ex: &Executor<'_>,
    id: RecordId,
    data: ProductChanges,
) -> surrealdb::Result<Option<ProductRecord>> {
    ex.update_one(id, data).await
}

/// Produtos ativos e inativos, por nome (e id, para a ordem não oscilar em empate), com
/// filtro opcional de `active` e paginação.
pub async fn find_page(
    ex: &Executor<'_>,
    page: ProductPage,
) -> surrealdb::Result<Vec<ProductRecord>> {
    ex.query_all(
        "SELECT * FROM product WHERE ($page.active = NONE OR active = $page.active) \
         ORDER BY name, id LIMIT $page.limit START $page.offset",
        "page",
        page,
    )
    .await
}
