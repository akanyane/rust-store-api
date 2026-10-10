use surrealdb::types::RecordId;

use crate::executor::Executor;
use crate::models::CountRow;
use crate::models::order::{NewOrder, OrderPage, OrderRecord, StatusChange};

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

/// Marca como pago só se ainda estiver `pending`. Mesmo padrão do cancelamento: a
/// checagem e a troca são um único comando, então só um dos concorrentes vence.
/// Devolve `None` quando o pedido não estava pendente (nada foi alterado).
pub async fn pay_if_pending(
    ex: &Executor<'_>,
    id: RecordId,
) -> surrealdb::Result<Option<OrderRecord>> {
    let rows: Vec<OrderRecord> = ex
        .query_all(
            "UPDATE $id SET status = 'paid', paid_at = time::now() WHERE status = 'pending'",
            "id",
            id,
        )
        .await?;
    Ok(rows.into_iter().next())
}

/// Pedidos de todos os clientes, do mais recente para o mais antigo, com filtro opcional
/// de status e paginação.
pub async fn find_page(ex: &Executor<'_>, page: OrderPage) -> surrealdb::Result<Vec<OrderRecord>> {
    ex.query_all(
        "SELECT * FROM order WHERE ($page.status = NONE OR status = $page.status) \
         ORDER BY created_at DESC LIMIT $page.limit START $page.offset",
        "page",
        page,
    )
    .await
}

/// Troca o status só se o pedido ainda estiver em `from`. A checagem e a troca são um
/// único comando, então duas mudanças simultâneas não passam as duas.
/// Devolve `None` quando o pedido não estava em `from` (nada foi alterado).
pub async fn change_status(
    ex: &Executor<'_>,
    change: StatusChange,
) -> surrealdb::Result<Option<OrderRecord>> {
    let rows: Vec<OrderRecord> = ex
        .query_all(
            "UPDATE $change.id SET status = $change.to, \
         paid_at = IF $change.to = 'paid' { time::now() } ELSE { paid_at } \
         WHERE status = $change.from",
            "change",
            change,
        )
        .await?;
    Ok(rows.into_iter().next())
}

/// Quantos pedidos casam com o filtro de status (sem filtro, todos), independente de
/// página. É o total da listagem de admin.
pub async fn count_matching(ex: &Executor<'_>, status: Option<String>) -> surrealdb::Result<i64> {
    let rows: Vec<CountRow> = ex
        .query_all(
            "SELECT count() AS count FROM order \
             WHERE ($filter = NONE OR status = $filter) GROUP ALL",
            "filter",
            status,
        )
        .await?;
    // Sem nenhum registro o `GROUP ALL` pode não devolver linha nenhuma.
    Ok(rows.first().map_or(0, |row| row.count))
}
