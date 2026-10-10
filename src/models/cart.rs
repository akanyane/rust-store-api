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

/// Como o carrinho sai na API: itens com o preço ATUAL da variante e o total (que soma
/// todos os itens, disponíveis ou não). `can_checkout` diz se o checkout tem chance de
/// passar agora; é um retrato do momento, e o checkout continua sendo quem decide.
#[derive(Debug, Serialize)]
pub struct CartView {
    pub items: Vec<CartItemView>,
    pub total: i64,
    pub can_checkout: bool,
}

/// Por que um item do carrinho não pode ser comprado agora.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnavailableReason {
    /// Produto ou variante desativados.
    Inactive,
    /// O estoque atual é menor que a quantidade no carrinho (inclui estoque zero).
    InsufficientStock,
}

#[derive(Debug, Serialize)]
pub struct CartItemView {
    pub variant_id: String,
    pub product_id: String,
    pub name: String,
    pub sku: String,
    pub unit_price: i64,
    pub quantity: i32,
    pub line_total: i64,
    pub available: bool,
    pub unavailable_reason: Option<UnavailableReason>,
}

impl CartView {
    pub fn empty() -> Self {
        CartView {
            items: Vec::new(),
            total: 0,
            can_checkout: false,
        }
    }
}
