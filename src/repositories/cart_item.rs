use surrealdb::types::RecordId;

use crate::executor::Executor;
use crate::models::cart_item::{CartItemRecord, NewCartItem};

pub async fn find_by_cart(
    ex: &Executor<'_>,
    cart: RecordId,
) -> surrealdb::Result<Vec<CartItemRecord>> {
    ex.query_all("SELECT * FROM cart_item WHERE cart = $cart", "cart", cart)
        .await
}

/// O item de uma variante específica no carrinho. Filtra em memória: um carrinho
/// tem poucos itens e o `query_all` aceita só um parâmetro.
pub async fn find_one(
    ex: &Executor<'_>,
    cart: RecordId,
    variant: &RecordId,
) -> surrealdb::Result<Option<CartItemRecord>> {
    let items = find_by_cart(ex, cart).await?;
    Ok(items.into_iter().find(|item| &item.variant == variant))
}

pub async fn create(
    ex: &Executor<'_>,
    data: NewCartItem,
) -> surrealdb::Result<Option<CartItemRecord>> {
    ex.create("cart_item", data).await
}

pub async fn update(
    ex: &Executor<'_>,
    id: RecordId,
    data: NewCartItem,
) -> surrealdb::Result<Option<CartItemRecord>> {
    ex.update_one(id, data).await
}

pub async fn delete(ex: &Executor<'_>, id: RecordId) -> surrealdb::Result<Option<CartItemRecord>> {
    ex.delete_one(id).await
}

pub async fn delete_by_cart(
    ex: &Executor<'_>,
    cart: RecordId,
) -> surrealdb::Result<Vec<CartItemRecord>> {
    ex.query_all(
        "DELETE cart_item WHERE cart = $cart RETURN BEFORE",
        "cart",
        cart,
    )
    .await
}
