use surrealdb::types::RecordId;

use crate::executor::Executor;
use crate::models::cart::{CartRecord, NewCart};

pub async fn find_by_customer(
    ex: &Executor<'_>,
    customer: RecordId,
) -> surrealdb::Result<Option<CartRecord>> {
    let rows: Vec<CartRecord> = ex
        .query_all(
            "SELECT * FROM cart WHERE customer = $customer LIMIT 1",
            "customer",
            customer,
        )
        .await?;
    Ok(rows.into_iter().next())
}

pub async fn create(ex: &Executor<'_>, data: NewCart) -> surrealdb::Result<Option<CartRecord>> {
    ex.create("cart", data).await
}

pub async fn delete(ex: &Executor<'_>, id: RecordId) -> surrealdb::Result<Option<CartRecord>> {
    ex.delete_one(id).await
}
