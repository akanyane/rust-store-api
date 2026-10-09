use std::collections::HashMap;

use surrealdb::{Surreal, engine::any::Any, types::RecordId};

use crate::error::AppError;
use crate::executor::Executor;
use crate::models::cart::{CartItemView, CartView, NewCart};
use crate::models::cart_item::{AddCartItem, NewCartItem, UpdateCartItem};
use crate::models::id_to_string;
use crate::models::variant::VariantRecord;
use crate::repositories::{
    cart as cart_repo, cart_item as cart_item_repo, customer as customer_repo,
    variant as variant_repo,
};

fn ensure_valid_quantity(quantity: i32) -> Result<(), AppError> {
    if quantity < 1 {
        return Err(AppError::Validation(
            "A quantidade deve ser pelo menos 1".to_string(),
        ));
    }
    Ok(())
}

fn ensure_purchasable(variant: &VariantRecord, quantity: i32) -> Result<(), AppError> {
    if !variant.active {
        return Err(AppError::Conflict("Variante indisponível".to_string()));
    }
    if quantity > variant.stock {
        return Err(AppError::Conflict(format!(
            "Estoque insuficiente: restam {} unidade(s)",
            variant.stock
        )));
    }
    Ok(())
}

/// Confirma que o cliente existe e devolve o `RecordId` dele.
async fn customer_record_id(ex: &Executor<'_>, customer_id: &str) -> Result<RecordId, AppError> {
    let customer = customer_repo::find_by_id(ex, customer_id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(customer.id)
}

/// Monta a visão do carrinho com o preço ATUAL de cada variante e o total.
async fn build_view(ex: &Executor<'_>, cart: RecordId) -> Result<CartView, AppError> {
    let items = cart_item_repo::find_by_cart(ex, cart).await?;

    let variant_ids: Vec<RecordId> = items.iter().map(|item| item.variant.clone()).collect();
    let variants: HashMap<String, VariantRecord> = variant_repo::find_by_ids(ex, variant_ids)
        .await?
        .into_iter()
        .map(|variant| (id_to_string(&variant.id), variant))
        .collect();

    let overflow = || AppError::Internal("overflow no cálculo do carrinho".to_string());
    let mut views = Vec::with_capacity(items.len());
    let mut total_cents: i64 = 0;

    for item in items {
        let variant = variants
            .get(&id_to_string(&item.variant))
            .ok_or(AppError::Internal(
                "variante do carrinho não encontrada".to_string(),
            ))?;

        let line_total_cents = variant
            .price_cents
            .checked_mul(i64::from(item.quantity))
            .ok_or_else(overflow)?;
        total_cents = total_cents
            .checked_add(line_total_cents)
            .ok_or_else(overflow)?;

        views.push(CartItemView {
            variant_id: id_to_string(&variant.id),
            product_id: id_to_string(&variant.product),
            name: variant.name.clone(),
            sku: variant.sku.clone(),
            unit_price_cents: variant.price_cents,
            quantity: item.quantity,
            line_total_cents,
        });
    }

    Ok(CartView {
        items: views,
        total_cents,
    })
}

pub async fn get_cart(db: &Surreal<Any>, customer_id: &str) -> Result<CartView, AppError> {
    let ex = Executor::Db(db);
    let customer = customer_record_id(&ex, customer_id).await?;

    match cart_repo::find_by_customer(&ex, customer).await? {
        Some(cart) => build_view(&ex, cart.id).await,
        None => Ok(CartView::empty()),
    }
}

pub async fn add_item(
    db: &Surreal<Any>,
    customer_id: &str,
    input: AddCartItem,
) -> Result<CartView, AppError> {
    ensure_valid_quantity(input.quantity)?;
    let customer = customer_record_id(&Executor::Db(db), customer_id).await?;

    let tx = db.clone().begin().await?;
    let result = add_item_in_tx(
        &Executor::Tx(&tx),
        customer,
        &input.variant_id,
        input.quantity,
    )
    .await;

    let cart_id = match result {
        Ok(cart_id) => {
            tx.commit().await?;
            cart_id
        }
        Err(e) => {
            if let Err(cancel_err) = tx.cancel().await {
                eprintln!("Falha ao cancelar transação: {cancel_err:?}");
            }
            return Err(e);
        }
    };

    build_view(&Executor::Db(db), cart_id).await
}

/// Acha (ou cria) o carrinho e soma a quantidade ao item da variante.
/// Roda dentro de uma transação: se algo falhar, nem o carrinho fica criado.
async fn add_item_in_tx(
    ex: &Executor<'_>,
    customer: RecordId,
    variant_id: &str,
    quantity: i32,
) -> Result<RecordId, AppError> {
    let variant = variant_repo::find_by_id(ex, variant_id)
        .await?
        .ok_or(AppError::NotFound)?;

    let cart = match cart_repo::find_by_customer(ex, customer.clone()).await? {
        Some(cart) => cart,
        None => cart_repo::create(ex, NewCart { customer })
            .await?
            .ok_or(AppError::Internal("falha ao criar carrinho".to_string()))?,
    };

    let existing = cart_item_repo::find_one(ex, cart.id.clone(), &variant.id).await?;
    let current = existing.as_ref().map_or(0, |item| item.quantity);
    let new_quantity = current
        .checked_add(quantity)
        .ok_or(AppError::Validation("Quantidade inválida".to_string()))?;

    ensure_purchasable(&variant, new_quantity)?;

    let data = NewCartItem {
        cart: cart.id.clone(),
        variant: variant.id,
        quantity: new_quantity,
    };

    match existing {
        Some(item) => cart_item_repo::update(ex, item.id, data).await?,
        None => cart_item_repo::create(ex, data).await?,
    }
    .ok_or(AppError::Internal("falha ao gravar item".to_string()))?;

    Ok(cart.id)
}

pub async fn update_item(
    db: &Surreal<Any>,
    customer_id: &str,
    variant_id: &str,
    input: UpdateCartItem,
) -> Result<CartView, AppError> {
    ensure_valid_quantity(input.quantity)?;
    let ex = Executor::Db(db);
    let customer = customer_record_id(&ex, customer_id).await?;

    let cart = cart_repo::find_by_customer(&ex, customer)
        .await?
        .ok_or(AppError::NotFound)?;
    let variant = variant_repo::find_by_id(&ex, variant_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let item = cart_item_repo::find_one(&ex, cart.id.clone(), &variant.id)
        .await?
        .ok_or(AppError::NotFound)?;

    ensure_purchasable(&variant, input.quantity)?;

    let data = NewCartItem {
        cart: cart.id.clone(),
        variant: variant.id,
        quantity: input.quantity,
    };
    cart_item_repo::update(&ex, item.id, data)
        .await?
        .ok_or(AppError::NotFound)?;

    build_view(&ex, cart.id).await
}

pub async fn remove_item(
    db: &Surreal<Any>,
    customer_id: &str,
    variant_id: &str,
) -> Result<CartView, AppError> {
    let ex = Executor::Db(db);
    let customer = customer_record_id(&ex, customer_id).await?;

    let cart = cart_repo::find_by_customer(&ex, customer)
        .await?
        .ok_or(AppError::NotFound)?;
    let variant_key = RecordId::new("variant", variant_id.to_string());
    let item = cart_item_repo::find_one(&ex, cart.id.clone(), &variant_key)
        .await?
        .ok_or(AppError::NotFound)?;

    cart_item_repo::delete(&ex, item.id).await?;

    build_view(&ex, cart.id).await
}
