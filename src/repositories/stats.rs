use crate::executor::Executor;
use crate::models::stats::{ActiveCountRow, LowStockFilter, LowStockRow, OrderStatusRow};

/// Quantos pedidos e que soma de totais há em cada status. Status sem pedidos não vêm.
pub async fn order_totals(ex: &Executor<'_>) -> surrealdb::Result<Vec<OrderStatusRow>> {
    ex.query_rows(
        "SELECT status, count() AS count, math::sum(total) AS revenue \
         FROM order GROUP BY status",
    )
    .await
}

/// Quantos produtos ativos e inativos existem. Grupo sem produtos não vem.
pub async fn product_counts(ex: &Executor<'_>) -> surrealdb::Result<Vec<ActiveCountRow>> {
    ex.query_rows("SELECT active, count() AS count FROM product GROUP BY active")
        .await
}

/// Variantes ATIVAS de produtos ATIVOS com estoque até `threshold`, das mais vazias para as
/// mais cheias (o SKU, que é único, desempata).
pub async fn low_stock(
    ex: &Executor<'_>,
    threshold: i64,
    limit: i64,
) -> surrealdb::Result<Vec<LowStockRow>> {
    ex.query_all(
        "SELECT id, product, name, sku, stock, product.name AS product_name FROM variant \
         WHERE active = true AND product.active = true AND stock <= $filter.threshold \
         ORDER BY stock, sku LIMIT $filter.limit",
        "filter",
        LowStockFilter { threshold, limit },
    )
    .await
}
