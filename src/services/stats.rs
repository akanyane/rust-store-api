use surrealdb::{Surreal, engine::any::Any};

use crate::error::AppError;
use crate::executor::Executor;
use crate::models::stats::{LowStockItem, OrderCounts, OrderStatusRow, ProductCounts, StatsView};
use crate::repositories::stats as stats_repo;

/// Variantes com estoque até este valor entram no aviso de estoque baixo.
const LOW_STOCK_THRESHOLD: i64 = 5;
/// No máximo quantas variantes o aviso lista.
const LOW_STOCK_LIMIT: i64 = 10;

/// Soma as linhas `GROUP BY status` em contagens por status e na receita (pedidos pagos,
/// enviados e entregues). Serve ao painel e à visão de cada cliente.
pub fn summarize_orders(rows: Vec<OrderStatusRow>) -> (OrderCounts, i64) {
    let mut orders = OrderCounts::default();
    let mut revenue = 0;
    for row in rows {
        orders.total += row.count;
        match row.status.as_str() {
            "pending" => orders.pending = row.count,
            "paid" => orders.paid = row.count,
            "shipped" => orders.shipped = row.count,
            "delivered" => orders.delivered = row.count,
            "cancelled" => orders.cancelled = row.count,
            _ => {}
        }
        // Pedido cancelado ou ainda pendente não é receita.
        if matches!(row.status.as_str(), "paid" | "shipped" | "delivered") {
            revenue += row.revenue;
        }
    }
    (orders, revenue)
}

/// Os números do painel de admin. São leituras separadas, então uma venda feita no meio pode
/// deixar um número defasado em uma unidade em relação a outro.
pub async fn stats(db: &Surreal<Any>) -> Result<StatsView, AppError> {
    let ex = Executor::Db(db);

    let (orders, revenue) = summarize_orders(stats_repo::order_totals(&ex).await?);

    let mut products = ProductCounts::default();
    for row in stats_repo::product_counts(&ex).await? {
        if row.active {
            products.active = row.count;
        } else {
            products.inactive = row.count;
        }
    }

    let low_stock = stats_repo::low_stock(&ex, LOW_STOCK_THRESHOLD, LOW_STOCK_LIMIT)
        .await?
        .into_iter()
        .map(LowStockItem::from)
        .collect();

    Ok(StatsView {
        orders,
        revenue,
        products,
        low_stock,
    })
}
