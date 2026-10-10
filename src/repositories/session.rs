use surrealdb::types::{Datetime, RecordId};

use crate::executor::Executor;
use crate::models::session::{NewSession, SessionRecord};

pub async fn create(
    ex: &Executor<'_>,
    data: NewSession,
) -> surrealdb::Result<Option<SessionRecord>> {
    ex.create("session", data).await
}

pub async fn find_by_token_hash(
    ex: &Executor<'_>,
    token_hash: &str,
) -> surrealdb::Result<Option<SessionRecord>> {
    let rows: Vec<SessionRecord> = ex
        .query_all(
            "SELECT * FROM session WHERE token_hash = $hash LIMIT 1",
            "hash",
            token_hash.to_string(),
        )
        .await?;
    Ok(rows.into_iter().next())
}

pub async fn find_by_refresh_hash(
    ex: &Executor<'_>,
    refresh_hash: &str,
) -> surrealdb::Result<Option<SessionRecord>> {
    let rows: Vec<SessionRecord> = ex
        .query_all(
            "SELECT * FROM session WHERE refresh_hash = $hash LIMIT 1",
            "hash",
            refresh_hash.to_string(),
        )
        .await?;
    Ok(rows.into_iter().next())
}

pub async fn delete(ex: &Executor<'_>, id: RecordId) -> surrealdb::Result<Option<SessionRecord>> {
    ex.delete_one(id).await
}

/// Apaga todas as sessões de um user (usado ao excluir a conta).
pub async fn delete_by_user(
    ex: &Executor<'_>,
    user: RecordId,
) -> surrealdb::Result<Vec<SessionRecord>> {
    ex.query_all(
        "DELETE session WHERE user = $user RETURN BEFORE",
        "user",
        user,
    )
    .await
}

/// Apaga as sessões cujo refresh token já venceu (o session token vence antes, então
/// nenhum dos dois serve mais). Um único comando, sem conflito com sign-in/refresh.
pub async fn delete_expired(
    ex: &Executor<'_>,
    now: Datetime,
) -> surrealdb::Result<Vec<SessionRecord>> {
    ex.query_all(
        "DELETE session WHERE refresh_expires_at <= $now RETURN BEFORE",
        "now",
        now,
    )
    .await
}
