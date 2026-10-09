use surrealdb::types::RecordId;

use crate::executor::Executor;
use crate::models::user::{NewUser, UserRecord};

pub async fn find_by_id(ex: &Executor<'_>, id: &str) -> surrealdb::Result<Option<UserRecord>> {
    ex.select_one(RecordId::new("user", id.to_string())).await
}

pub async fn find_by_email(
    ex: &Executor<'_>,
    email: &str,
) -> surrealdb::Result<Option<UserRecord>> {
    let rows: Vec<UserRecord> = ex
        .query_all(
            "SELECT * FROM user WHERE email = $email LIMIT 1",
            "email",
            email.to_string(),
        )
        .await?;
    Ok(rows.into_iter().next())
}

pub async fn create(ex: &Executor<'_>, data: NewUser) -> surrealdb::Result<Option<UserRecord>> {
    ex.create("user", data).await
}

pub async fn delete(ex: &Executor<'_>, id: &str) -> surrealdb::Result<Option<UserRecord>> {
    ex.delete_one(RecordId::new("user", id.to_string())).await
}
