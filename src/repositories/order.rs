use surrealdb::types::RecordId;

use crate::executor::Executor;
use crate::models::order::{NewOrder, OrderRecord};

pub async fn create(ex: &Executor<'_>, data: NewOrder) -> surrealdb::Result<Option<OrderRecord>> {
    ex.create("order", data).await
}

pub async fn find_by_id(ex: &Executor<'_>, id: &str) -> surrealdb::Result<Option<OrderRecord>> {
    ex.select_one(RecordId::new("order", id.to_string())).await
}

/// Do mais recente para o mais antigo.
pub async fn find_by_customer(
    ex: &Executor<'_>,
    customer: RecordId,
) -> surrealdb::Result<Vec<OrderRecord>> {
    ex.query_all(
        "SELECT * FROM order WHERE customer = $customer ORDER BY created_at DESC",
        "customer",
        customer,
    )
    .await
}

pub async fn exists_by_customer(ex: &Executor<'_>, customer: RecordId) -> surrealdb::Result<bool> {
    let rows: Vec<OrderRecord> = ex
        .query_all(
            "SELECT * FROM order WHERE customer = $customer LIMIT 1",
            "customer",
            customer,
        )
        .await?;
    Ok(!rows.is_empty())
}

/// Marca como cancelado só se ainda estiver `pending`. A checagem e a troca são um
/// único comando, então dois cancelamentos simultâneos não passam os dois.
/// Devolve `None` quando o pedido não estava pendente (nada foi alterado).
pub async fn cancel_if_pending(
    ex: &Executor<'_>,
    id: RecordId,
) -> surrealdb::Result<Option<OrderRecord>> {
    let rows: Vec<OrderRecord> = ex
        .query_all(
            "UPDATE $id SET status = 'cancelled' WHERE status = 'pending'",
            "id",
            id,
        )
        .await?;
    Ok(rows.into_iter().next())
}
