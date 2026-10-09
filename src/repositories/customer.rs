use surrealdb::types::RecordId;

use crate::executor::Executor;
use crate::models::customer::{CustomerRecord, NewCustomer};

pub async fn find_by_id(ex: &Executor<'_>, id: &str) -> surrealdb::Result<Option<CustomerRecord>> {
    ex.select_one(RecordId::new("customer", id.to_string()))
        .await
}

/// `id` é a chave do user dono deste perfil (user:abc -> customer:abc).
pub async fn create(
    ex: &Executor<'_>,
    id: &str,
    data: NewCustomer,
) -> surrealdb::Result<Option<CustomerRecord>> {
    ex.create_with_id(RecordId::new("customer", id.to_string()), data)
        .await
}

pub async fn update(
    ex: &Executor<'_>,
    id: &str,
    data: NewCustomer,
) -> surrealdb::Result<Option<CustomerRecord>> {
    ex.update_one(RecordId::new("customer", id.to_string()), data)
        .await
}

pub async fn delete(ex: &Executor<'_>, id: &str) -> surrealdb::Result<Option<CustomerRecord>> {
    ex.delete_one(RecordId::new("customer", id.to_string()))
        .await
}
