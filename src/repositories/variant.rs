use surrealdb::types::{RecordId, SurrealValue};

use crate::executor::Executor;
use crate::models::variant::{NewVariant, VariantChanges, VariantRecord};

pub async fn create(
    ex: &Executor<'_>,
    data: NewVariant,
) -> surrealdb::Result<Option<VariantRecord>> {
    ex.create("variant", data).await
}

pub async fn find_by_id(ex: &Executor<'_>, id: &str) -> surrealdb::Result<Option<VariantRecord>> {
    ex.select_one(RecordId::new("variant", id.to_string()))
        .await
}

pub async fn find_by_ids(
    ex: &Executor<'_>,
    ids: Vec<RecordId>,
) -> surrealdb::Result<Vec<VariantRecord>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    ex.query_all("SELECT * FROM variant WHERE id IN $ids", "ids", ids)
        .await
}

/// Parâmetro único do `decrement_stock` (o `query_all` aceita só um).
#[derive(SurrealValue)]
struct StockChange {
    id: RecordId,
    quantity: i32,
}

/// Baixa o estoque só se ainda houver quantidade suficiente. A checagem e a baixa
/// são um único comando, então dois checkouts simultâneos não passam do estoque.
/// Devolve `None` quando não havia estoque suficiente (nada foi alterado).
pub async fn decrement_stock(
    ex: &Executor<'_>,
    id: RecordId,
    quantity: i32,
) -> surrealdb::Result<Option<VariantRecord>> {
    let rows: Vec<VariantRecord> = ex
        .query_all(
            "UPDATE $change.id SET stock -= $change.quantity WHERE stock >= $change.quantity",
            "change",
            StockChange { id, quantity },
        )
        .await?;
    Ok(rows.into_iter().next())
}

/// Variantes do produto por nome e, em empate, por SKU (que é único, então a ordem é total).
/// É o único ponto de leitura da lista, então a lista pública, o detalhe público e o detalhe
/// de admin seguem a mesma ordem.
pub async fn find_by_product(
    ex: &Executor<'_>,
    product: RecordId,
) -> surrealdb::Result<Vec<VariantRecord>> {
    ex.query_all(
        "SELECT * FROM variant WHERE product = $product ORDER BY name, sku",
        "product",
        product,
    )
    .await
}

pub async fn exists_by_sku(ex: &Executor<'_>, sku: &str) -> surrealdb::Result<bool> {
    let rows: Vec<VariantRecord> = ex
        .query_all(
            "SELECT * FROM variant WHERE sku = $sku LIMIT 1",
            "sku",
            sku.to_string(),
        )
        .await?;
    Ok(!rows.is_empty())
}

/// Devolve unidades ao estoque (ex.: pedido cancelado).
pub async fn increment_stock(
    ex: &Executor<'_>,
    id: RecordId,
    quantity: i32,
) -> surrealdb::Result<Option<VariantRecord>> {
    let rows: Vec<VariantRecord> = ex
        .query_all(
            "UPDATE $change.id SET stock += $change.quantity",
            "change",
            StockChange { id, quantity },
        )
        .await?;
    Ok(rows.into_iter().next())
}

pub async fn find_by_sku(ex: &Executor<'_>, sku: &str) -> surrealdb::Result<Option<VariantRecord>> {
    let rows: Vec<VariantRecord> = ex
        .query_all(
            "SELECT * FROM variant WHERE sku = $sku LIMIT 1",
            "sku",
            sku.to_string(),
        )
        .await?;
    Ok(rows.into_iter().next())
}

pub async fn update(
    ex: &Executor<'_>,
    id: RecordId,
    data: VariantChanges,
) -> surrealdb::Result<Option<VariantRecord>> {
    ex.update_one(id, data).await
}
