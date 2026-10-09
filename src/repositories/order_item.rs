use surrealdb::types::RecordId;

use crate::executor::Executor;
use crate::models::order::{NewOrderItem, OrderItemRecord};

pub async fn create(
    ex: &Executor<'_>,
    data: NewOrderItem,
) -> surrealdb::Result<Option<OrderItemRecord>> {
    ex.create("order_item", data).await
}

/// Itens de vários pedidos de uma vez, para a listagem não fazer uma consulta por pedido.
pub async fn find_by_orders(
    ex: &Executor<'_>,
    orders: Vec<RecordId>,
) -> surrealdb::Result<Vec<OrderItemRecord>> {
    if orders.is_empty() {
        return Ok(Vec::new());
    }
    ex.query_all(
        "SELECT * FROM order_item WHERE order IN $orders",
        "orders",
        orders,
    )
    .await
}
