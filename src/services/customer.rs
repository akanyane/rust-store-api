use surrealdb::{Surreal, engine::any::Any, types::RecordId};

use crate::error::AppError;
use crate::executor::Executor;
use crate::models::customer::{Customer, CustomerRecord, NewCustomer, UpdateCustomer};
use crate::models::id_to_string;
use crate::repositories::{
    cart as cart_repo, cart_item as cart_item_repo, customer as customer_repo, order as order_repo,
    session as session_repo, user as user_repo,
};

/// O e-mail mora no user de mesma chave. Cliente sem user é inconsistência de dados.
async fn email_of(ex: &Executor<'_>, id: &str) -> Result<String, AppError> {
    user_repo::find_by_id(ex, id)
        .await?
        .map(|user| user.email)
        .ok_or_else(|| AppError::Internal("customer without an associated user".to_string()))
}

async fn to_customer(ex: &Executor<'_>, record: CustomerRecord) -> Result<Customer, AppError> {
    let email = email_of(ex, &id_to_string(&record.id)).await?;
    Ok(Customer::from_parts(record, email))
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
