use std::collections::HashMap;

use chrono::{DateTime, Utc};
use surrealdb::{Surreal, engine::any::Any, types::RecordId};

use crate::error::AppError;
use crate::executor::Executor;
use crate::models::customer::{
    AdminCustomerListItem, AdminCustomerView, Customer, CustomerFilter, CustomerPage,
    CustomerRecord, ListCustomersQuery, NewCustomer, UpdateCustomer,
};
use crate::models::order::search_term;
use crate::models::stats::OrderStatusRow;
use crate::models::{DEFAULT_PAGE_SIZE, id_to_string};
use crate::repositories::{
    cart as cart_repo, cart_item as cart_item_repo, customer as customer_repo, order as order_repo,
    session as session_repo, user as user_repo,
};
use crate::services::stats::summarize_orders;

/// O username mora no user de mesma chave. Cliente sem user é inconsistência de dados.
async fn username_of(ex: &Executor<'_>, id: &str) -> Result<String, AppError> {
    user_repo::find_by_id(ex, id)
        .await?
        .map(|user| user.username)
        .ok_or_else(|| AppError::Internal("customer without an associated user".to_string()))
}

async fn to_customer(ex: &Executor<'_>, record: CustomerRecord) -> Result<Customer, AppError> {
    let username = username_of(ex, &id_to_string(&record.id)).await?;
    Ok(Customer::from_parts(record, username))
}

pub async fn get_customer(db: &Surreal<Any>, id: &str) -> Result<Customer, AppError> {
    let ex = Executor::Db(db);
    let record = customer_repo::find_by_id(&ex, id)
        .await?
        .ok_or(AppError::NotFound)?;
    to_customer(&ex, record).await
}

pub async fn update_customer(
    db: &Surreal<Any>,
    id: &str,
    input: UpdateCustomer,
) -> Result<Customer, AppError> {
    let ex = Executor::Db(db);

    // Confirma antes que o cliente existe: o `update` não deve criar um registro novo.
    customer_repo::find_by_id(&ex, id)
        .await?
        .ok_or(AppError::NotFound)?;

    let data = NewCustomer {
        first_name: input.first_name,
        last_name: input.last_name,
        birthday: input.birthday,
    };
    let record = customer_repo::update(&ex, id, data)
        .await?
        .ok_or(AppError::NotFound)?;

    to_customer(&ex, record).await
}

/// Exclui o perfil, o user e as sessões. Sem isso a conta continuaria
/// conseguindo entrar mesmo depois de o cliente ser apagado.
pub async fn delete_customer(db: &Surreal<Any>, id: &str) -> Result<(), AppError> {
    customer_repo::find_by_id(&Executor::Db(db), id)
        .await?
        .ok_or(AppError::NotFound)?;

    let tx = db.clone().begin().await?;
    let result = delete_in_tx(&Executor::Tx(&tx), id).await;

    match result {
        Ok(()) => {
            tx.commit().await?;
            Ok(())
        }
        Err(e) => {
            if let Err(cancel_err) = tx.cancel().await {
                eprintln!("Failed to roll back transaction: {cancel_err:?}");
            }
            Err(e)
        }
    }
}

async fn delete_in_tx(ex: &Executor<'_>, id: &str) -> Result<(), AppError> {
    let customer = RecordId::new("customer", id.to_string());

    // Pedido é registro financeiro: o cliente dono de um não pode ser apagado.
    if order_repo::exists_by_customer(ex, customer.clone()).await? {
        return Err(AppError::Conflict(
            "A customer with orders cannot be deleted".to_string(),
        ));
    }

    if let Some(cart) = cart_repo::find_by_customer(ex, customer).await? {
        cart_item_repo::delete_by_cart(ex, cart.id.clone()).await?;
        cart_repo::delete(ex, cart.id).await?;
    }

    session_repo::delete_by_user(ex, RecordId::new("user", id.to_string())).await?;
    customer_repo::delete(ex, id).await?;
    user_repo::delete(ex, id).await?;
    Ok(())
}

/// Visão de admin de um cliente: só o username e o que ele fez. Quem não é cliente (id
/// inexistente, ou o admin, que não tem perfil) dá 404.
pub async fn admin_get_customer(
    db: &Surreal<Any>,
    id: &str,
) -> Result<AdminCustomerView, AppError> {
    let ex = Executor::Db(db);
    let record = customer_repo::find_by_id(&ex, id)
        .await?
        .ok_or(AppError::NotFound)?;
    let user = user_repo::find_by_id(&ex, id)
        .await?
        .ok_or_else(|| AppError::Internal("customer without an associated user".to_string()))?;

    let (orders, spent) =
        summarize_orders(order_repo::totals_by_customer(&ex, record.id.clone()).await?);
    let last_order_at = order_repo::last_created_at(&ex, record.id.clone()).await?;

    Ok(AdminCustomerView {
        id: id_to_string(&record.id),
        username: user.username,
        created_at: DateTime::<Utc>::from(user.created_at),
        orders,
        spent,
        last_order_at: last_order_at.map(DateTime::<Utc>::from),
    })
}

/// Clientes para a listagem do admin: uma página com username e atividade (pedidos, gasto e
/// último pedido), e o total que casa com a busca (independente de `limit` e `offset`). A
/// atividade de toda a página sai de uma consulta só. Pedidos e contas são leituras separadas,
/// então uma compra feita no meio pode deixar um número defasado.
pub async fn admin_list_customers(
    db: &Surreal<Any>,
    query: ListCustomersQuery,
) -> Result<(Vec<AdminCustomerListItem>, i64), AppError> {
    let ex = Executor::Db(db);
    let search = search_term(query.q.as_deref());
    let sort = query.sort.unwrap_or_default();
    let page = CustomerPage {
        search: search.clone(),
        limit: query.limit.unwrap_or(DEFAULT_PAGE_SIZE),
        offset: query.offset.unwrap_or(0),
    };

    let users = user_repo::find_customers_page(&ex, page, sort).await?;
    let customers: Vec<RecordId> = users
        .iter()
        .map(|user| RecordId::new("customer", id_to_string(&user.id)))
        .collect();

    let mut activity: HashMap<String, (Vec<OrderStatusRow>, Option<DateTime<Utc>>)> =
        HashMap::new();
    for row in order_repo::totals_by_customers(&ex, customers).await? {
        let last = DateTime::<Utc>::from(row.last);
        let entry = activity.entry(id_to_string(&row.customer)).or_default();
        entry.0.push(OrderStatusRow {
            status: row.status,
            count: row.count,
            revenue: row.revenue,
        });
        entry.1 = Some(entry.1.map_or(last, |seen| seen.max(last)));
    }

    let items = users
        .into_iter()
        .map(|user| {
            let id = id_to_string(&user.id);
            let (rows, last_order_at) = activity.remove(&id).unwrap_or_default();
            let (orders, spent) = summarize_orders(rows);
            AdminCustomerListItem {
                id,
                username: user.username,
                created_at: DateTime::<Utc>::from(user.created_at),
                orders: orders.total,
                spent,
                last_order_at,
            }
        })
        .collect();

    let total = user_repo::count_customers(&ex, CustomerFilter { search }).await?;
    Ok((items, total))
}
