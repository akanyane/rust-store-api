use surrealdb::types::RecordId;

use crate::executor::Executor;
use crate::models::CountRow;
use crate::models::customer::{CustomerFilter, CustomerPage, CustomerSort};
use crate::models::user::{NewUser, UserRecord};

pub async fn find_by_id(ex: &Executor<'_>, id: &str) -> surrealdb::Result<Option<UserRecord>> {
    ex.select_one(RecordId::new("user", id.to_string())).await
}

pub async fn find_by_username(
    ex: &Executor<'_>,
    username: &str,
) -> surrealdb::Result<Option<UserRecord>> {
    let rows: Vec<UserRecord> = ex
        .query_all(
            "SELECT * FROM user WHERE username = $username LIMIT 1",
            "username",
            username.to_string(),
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

/// Uma página de contas de CLIENTE (o admin não entra), com busca por prefixo do username ou
/// do id. O id desempata, para a ordem não oscilar entre páginas.
pub async fn find_customers_page(
    ex: &Executor<'_>,
    page: CustomerPage,
    sort: CustomerSort,
) -> surrealdb::Result<Vec<UserRecord>> {
    let query = match sort {
        CustomerSort::Newest => {
            "SELECT * FROM user WHERE role = 'customer' \
             AND ($page.search = NONE \
                  OR string::starts_with(username, $page.search) \
                  OR string::starts_with(<string> record::id(id), $page.search)) \
             ORDER BY created_at DESC, id LIMIT $page.limit START $page.offset"
        }
        CustomerSort::Username => {
            "SELECT * FROM user WHERE role = 'customer' \
             AND ($page.search = NONE \
                  OR string::starts_with(username, $page.search) \
                  OR string::starts_with(<string> record::id(id), $page.search)) \
             ORDER BY username, id LIMIT $page.limit START $page.offset"
        }
    };
    ex.query_all(query, "page", page).await
}

/// Quantas contas de cliente casam com a busca, independente de página.
pub async fn count_customers(ex: &Executor<'_>, filter: CustomerFilter) -> surrealdb::Result<i64> {
    let rows: Vec<CountRow> = ex
        .query_all(
            "SELECT count() AS count FROM user WHERE role = 'customer' \
             AND ($filter.search = NONE \
                  OR string::starts_with(username, $filter.search) \
                  OR string::starts_with(<string> record::id(id), $filter.search)) \
             GROUP ALL",
            "filter",
            filter,
        )
        .await?;
    // Sem nenhuma conta o `GROUP ALL` pode não devolver linha nenhuma.
    Ok(rows.first().map_or(0, |row| row.count))
}
