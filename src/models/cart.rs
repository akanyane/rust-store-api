use serde::Serialize;
use surrealdb::types::{RecordId, SurrealValue};

#[derive(Debug, Clone, SurrealValue)]
pub struct CartRecord {
    pub id: RecordId,
    pub customer: RecordId,
}

#[derive(Debug, SurrealValue)]
pub struct NewCart {
    pub customer: RecordId,
}

/// Como o carrinho sai na API: itens com o preço ATUAL da variante e o total.
#[derive(Debug, Serialize)]
pub struct CartView {
    pub items: Vec<CartItemView>,
    pub total_cents: i64,
}

#[derive(Debug, Serialize)]
pub struct CartItemView {
    pub variant_id: String,
    pub product_id: String,
    pub name: String,
    pub sku: String,
    pub unit_price_cents: i64,
    pub quantity: i32,
    pub line_total_cents: i64,
}

impl CartView {
    pub fn empty() -> Self {
        CartView {
            items: Vec::new(),
            total_cents: 0,
        }
    }
}
