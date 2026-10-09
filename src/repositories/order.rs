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
