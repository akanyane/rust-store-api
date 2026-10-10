use std::collections::HashMap;
use std::time::Duration;

use chrono::{DateTime, Utc};
use surrealdb::{Surreal, engine::any::Any, types::RecordId};

use crate::error::AppError;
use crate::executor::Executor;
use crate::models::cart_item::CartItemRecord;
use crate::models::id_to_string;
use crate::models::order::{
    NewOrder, NewOrderItem, OrderItemRecord, OrderItemView, OrderRecord, OrderView,
};
use crate::models::product::ProductRecord;
use crate::models::variant::VariantRecord;
use crate::repositories::{
    cart as cart_repo, cart_item as cart_item_repo, customer as customer_repo, order as order_repo,
    order_item as order_item_repo, product as product_repo, variant as variant_repo,
};

const MAX_ATTEMPTS: u32 = 5;

/// Roda a operação e a repete quando a transação perde uma corrida de escrita (o SDK
/// avisa que ela "pode ser repetida"). Na repetição o resultado sai certo sozinho: quem
/// perdeu a última unidade recebe o 409 de estoque, e quem ainda cabe no estoque passa.
/// A pausa aleatória evita que os concorrentes colidam de novo no mesmo instante.
/// Esgotadas as tentativas, devolve 409 em vez de um 500.
async fn retry_on_conflict<T, F, Fut>(mut operation: F) -> Result<T, AppError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, AppError>>,
{
    for attempt in 1..=MAX_ATTEMPTS {
        match operation().await {
            Err(e) if e.is_write_conflict() => {
                if attempt < MAX_ATTEMPTS {
                    let pause_ms = rand::random_range(10..50_u64) * u64::from(attempt);
                    tokio::time::sleep(Duration::from_millis(pause_ms)).await;
                }
            }
            other => return other,
        }
    }
    Err(AppError::Conflict(
        "The store is busy, please try again".to_string(),
    ))
}

fn empty_cart() -> AppError {
    AppError::Conflict("The cart is empty".to_string())
}

fn build_view(order: OrderRecord, items: Vec<OrderItemRecord>) -> Result<OrderView, AppError> {
    let items = items
        .into_iter()
        .map(|item| {
            OrderItemView::from_record(item)
                .ok_or_else(|| AppError::Internal("overflow in order item total".to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(OrderView {
        id: id_to_string(&order.id),
        status: order.status,
        total: order.total,
        created_at: DateTime::<Utc>::from(order.created_at),
        items,
    })
}

fn order_total(lines: &[(VariantRecord, i32)]) -> Result<i64, AppError> {
    lines
        .iter()
        .try_fold(0_i64, |total, (variant, quantity)| {
            variant
                .price
                .checked_mul(i64::from(*quantity))
                .and_then(|line| total.checked_add(line))
        })
        .ok_or_else(|| AppError::Internal("overflow in order total".to_string()))
}

/// Transforma o carrinho do cliente em pedido. Se outra compra mexer no mesmo estoque
/// ao mesmo tempo, a transação é repetida (ver `retry_on_conflict`).
pub async fn checkout(db: &Surreal<Any>, customer_id: &str) -> Result<OrderView, AppError> {
    retry_on_conflict(|| checkout_once(db, customer_id)).await
}

/// Uma tentativa de checkout. Tudo numa transação: se qualquer item falhar, o estoque
/// já baixado dos anteriores volta e nada é criado.
async fn checkout_once(db: &Surreal<Any>, customer_id: &str) -> Result<OrderView, AppError> {
    let tx = db.clone().begin().await?;
    let result = checkout_in_tx(&Executor::Tx(&tx), customer_id).await;

    match result {
        Ok(view) => {
            tx.commit().await?;
            Ok(view)
        }
        Err(e) => {
            if let Err(cancel_err) = tx.cancel().await {
                eprintln!("Failed to roll back transaction: {cancel_err:?}");
            }
            Err(e)
        }
    }
}

async fn checkout_in_tx(ex: &Executor<'_>, customer_id: &str) -> Result<OrderView, AppError> {
    let customer = customer_repo::find_by_id(ex, customer_id)
        .await?
        .ok_or(AppError::NotFound)?;

    let cart = cart_repo::find_by_customer(ex, customer.id.clone())
        .await?
        .ok_or_else(empty_cart)?;
    let cart_items = cart_item_repo::find_by_cart(ex, cart.id.clone()).await?;
    if cart_items.is_empty() {
        return Err(empty_cart());
    }

    let lines = reserve_stock(ex, &cart_items).await?;
    let total = order_total(&lines)?;

    let order = order_repo::create(
        ex,
        NewOrder {
            customer: customer.id,
            total,
        },
    )
    .await?
    .ok_or_else(|| AppError::Internal("failed to create order".to_string()))?;

    let mut items = Vec::with_capacity(lines.len());
    for (variant, quantity) in lines {
        let item = order_item_repo::create(
            ex,
            NewOrderItem {
                order: order.id.clone(),
                variant: variant.id,
                name: variant.name,
                sku: variant.sku,
                unit_price: variant.price,
                quantity,
            },
        )
        .await?
        .ok_or_else(|| AppError::Internal("failed to create order item".to_string()))?;
        items.push(item);
    }

    // O carrinho cumpriu o papel; esvaziar evita comprar tudo de novo sem querer.
    cart_item_repo::delete_by_cart(ex, cart.id.clone()).await?;
    cart_repo::delete(ex, cart.id).await?;

    build_view(order, items)
}

/// Confere cada item (variante e produto ativos) e baixa o estoque. Devolve cada
/// variante já atualizada, junto com a quantidade comprada.
async fn reserve_stock(
    ex: &Executor<'_>,
    cart_items: &[CartItemRecord],
) -> Result<Vec<(VariantRecord, i32)>, AppError> {
    let variant_ids = cart_items.iter().map(|item| item.variant.clone()).collect();
    let variants: HashMap<String, VariantRecord> = variant_repo::find_by_ids(ex, variant_ids)
        .await?
        .into_iter()
        .map(|variant| (id_to_string(&variant.id), variant))
        .collect();

    let product_ids = variants
        .values()
        .map(|variant| variant.product.clone())
        .collect();
    let products: HashMap<String, ProductRecord> = product_repo::find_by_ids(ex, product_ids)
        .await?
        .into_iter()
        .map(|product| (id_to_string(&product.id), product))
        .collect();

    let mut lines = Vec::with_capacity(cart_items.len());
    for item in cart_items {
        let variant = variants
            .get(&id_to_string(&item.variant))
            .ok_or_else(|| AppError::Internal("cart variant not found".to_string()))?;

        let product_active = products
            .get(&id_to_string(&variant.product))
            .is_some_and(|product| product.active);
        if !variant.active || !product_active {
            return Err(AppError::Conflict(format!(
                "Item unavailable: {}",
                variant.sku
            )));
        }

        let updated = variant_repo::decrement_stock(ex, item.variant.clone(), item.quantity)
            .await?
            .ok_or_else(|| AppError::Conflict(format!("Insufficient stock for {}", variant.sku)))?;
        lines.push((updated, item.quantity));
    }

    Ok(lines)
}

pub async fn list_orders(db: &Surreal<Any>, customer_id: &str) -> Result<Vec<OrderView>, AppError> {
    let ex = Executor::Db(db);
    let customer = RecordId::new("customer", customer_id.to_string());

    let orders = order_repo::find_by_customer(&ex, customer).await?;
    let order_ids = orders.iter().map(|order| order.id.clone()).collect();

    let mut items_by_order: HashMap<String, Vec<OrderItemRecord>> = HashMap::new();
    for item in order_item_repo::find_by_orders(&ex, order_ids).await? {
        items_by_order
            .entry(id_to_string(&item.order))
            .or_default()
            .push(item);
    }

    orders
        .into_iter()
        .map(|order| {
            let items = items_by_order
                .remove(&id_to_string(&order.id))
                .unwrap_or_default();
            build_view(order, items)
        })
        .collect()
}

pub async fn get_order(
    db: &Surreal<Any>,
    customer_id: &str,
    order_id: &str,
) -> Result<OrderView, AppError> {
    let ex = Executor::Db(db);

    let order = order_repo::find_by_id(&ex, order_id)
        .await?
        .ok_or(AppError::NotFound)?;

    // Pedido de outro cliente aparece como inexistente, para não revelar que existe.
    if id_to_string(&order.customer) != customer_id {
        return Err(AppError::NotFound);
    }

    let items = order_item_repo::find_by_orders(&ex, vec![order.id.clone()]).await?;
    build_view(order, items)
}

/// Cancela um pedido pendente e devolve o estoque dos itens. Repete a transação em caso
/// de conflito de escrita (ver `retry_on_conflict`).
pub async fn cancel_order(
    db: &Surreal<Any>,
    customer_id: &str,
    order_id: &str,
) -> Result<OrderView, AppError> {
    retry_on_conflict(|| cancel_order_once(db, customer_id, order_id)).await
}

/// Uma tentativa de cancelamento, numa transação.
async fn cancel_order_once(
    db: &Surreal<Any>,
    customer_id: &str,
    order_id: &str,
) -> Result<OrderView, AppError> {
    let tx = db.clone().begin().await?;
    let result = cancel_order_in_tx(&Executor::Tx(&tx), customer_id, order_id).await;

    match result {
        Ok(view) => {
            tx.commit().await?;
            Ok(view)
        }
        Err(e) => {
            if let Err(cancel_err) = tx.cancel().await {
                eprintln!("Failed to roll back transaction: {cancel_err:?}");
            }
            Err(e)
        }
    }
}

async fn cancel_order_in_tx(
    ex: &Executor<'_>,
    customer_id: &str,
    order_id: &str,
) -> Result<OrderView, AppError> {
    let order = order_repo::find_by_id(ex, order_id)
        .await?
        .ok_or(AppError::NotFound)?;

    // Pedido de outro cliente aparece como inexistente, para não revelar que existe.
    if id_to_string(&order.customer) != customer_id {
        return Err(AppError::NotFound);
    }

    let cancelled = order_repo::cancel_if_pending(ex, order.id.clone())
        .await?
        .ok_or_else(|| AppError::Conflict("Only pending orders can be cancelled".to_string()))?;

    let items = order_item_repo::find_by_orders(ex, vec![cancelled.id.clone()]).await?;
    for item in &items {
        // Devolve o estoque mesmo se a variante estiver inativa: as unidades existem.
        variant_repo::increment_stock(ex, item.variant.clone(), item.quantity)
            .await?
            .ok_or_else(|| AppError::Internal("order variant not found".to_string()))?;
    }

    build_view(cancelled, items)
}
