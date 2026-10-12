use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{Datetime, RecordId, SurrealValue};

use validator::Validate;

use super::id_to_string;

#[derive(Debug, Clone, SurrealValue)]
pub struct OrderRecord {
    pub id: RecordId,
    pub customer: RecordId,
    pub status: String,
    pub total: i64,
    pub created_at: Datetime,
    pub paid_at: Option<Datetime>,
}

/// O `status` fica de fora: o schema grava `pending` por padrão.
#[derive(Debug, SurrealValue)]
pub struct NewOrder {
    pub customer: RecordId,
    pub total: i64,
}

#[derive(Debug, Clone, SurrealValue)]
pub struct OrderItemRecord {
    pub id: RecordId,
    pub order: RecordId,
    pub variant: RecordId,
    pub name: String,
    /// `None` em itens criados antes do campo existir.
    pub product_name: Option<String>,
    pub sku: String,
    pub unit_price: i64,
    pub quantity: i32,
}

#[derive(Debug, SurrealValue)]
pub struct NewOrderItem {
    pub order: RecordId,
    pub variant: RecordId,
    pub name: String,
    pub product_name: String,
    pub sku: String,
    pub unit_price: i64,
    pub quantity: i32,
}

#[derive(Debug, Serialize)]
pub struct OrderView {
    pub id: String,
    pub status: String,
    pub total: i64,
    pub created_at: DateTime<Utc>,
    pub paid_at: Option<DateTime<Utc>>,
    pub items: Vec<OrderItemView>,
}

#[derive(Debug, Serialize)]
pub struct OrderItemView {
    pub variant_id: String,
    pub name: String,
    /// Nome do produto na hora da compra (`null` em pedidos antigos, que não o guardaram).
    pub product_name: Option<String>,
    pub sku: String,
    pub unit_price: i64,
    pub quantity: i32,
    pub line_total: i64,
}

impl OrderItemView {
    /// `None` só se o total da linha estourar `i64`, o que o checkout já impede.
    pub fn from_record(record: OrderItemRecord) -> Option<Self> {
        let line_total = record.unit_price.checked_mul(i64::from(record.quantity))?;

        Some(OrderItemView {
            variant_id: id_to_string(&record.variant),
            name: record.name,
            product_name: record.product_name,
            sku: record.sku,
            unit_price: record.unit_price,
            quantity: record.quantity,
            line_total,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderStatus {
    Pending,
    Paid,
    Cancelled,
    Shipped,
    Delivered,
}

impl OrderStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            OrderStatus::Pending => "pending",
            OrderStatus::Paid => "paid",
            OrderStatus::Cancelled => "cancelled",
            OrderStatus::Shipped => "shipped",
            OrderStatus::Delivered => "delivered",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(OrderStatus::Pending),
            "paid" => Some(OrderStatus::Paid),
            "cancelled" => Some(OrderStatus::Cancelled),
            "shipped" => Some(OrderStatus::Shipped),
            "delivered" => Some(OrderStatus::Delivered),
            _ => None,
        }
    }

    /// pending -> paid | cancelled; paid -> shipped -> delivered. Cancelado e entregue
    /// são finais. Pedido pago não volta para cancelado: não existe reembolso.
    pub fn can_become(self, next: OrderStatus) -> bool {
        matches!(
            (self, next),
            (OrderStatus::Pending, OrderStatus::Paid)
                | (OrderStatus::Pending, OrderStatus::Cancelled)
                | (OrderStatus::Paid, OrderStatus::Shipped)
                | (OrderStatus::Shipped, OrderStatus::Delivered)
        )
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateOrderStatus {
    pub status: OrderStatus,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ListOrdersQuery {
    pub status: Option<OrderStatus>,
    /// Número do pedido (com ou sem `#`) ou id do cliente, por prefixo.
    #[validate(length(max = 64, message = "must be at most 64 characters"))]
    pub q: Option<String>,
    #[validate(range(min = 1, max = 200, message = "must be between 1 and 200"))]
    pub limit: Option<i64>,
    #[validate(range(min = 0, message = "must not be negative"))]
    pub offset: Option<i64>,
}

/// Visão do admin: o pedido de sempre mais o dono (só o id, sem dados pessoais).
#[derive(Debug, Serialize)]
pub struct AdminOrderView {
    pub customer_id: String,
    #[serde(flatten)]
    pub order: OrderView,
}

/// Parâmetro único da listagem (o `query_all` aceita só um).
#[derive(Debug, SurrealValue)]
pub struct OrderPage {
    pub status: Option<String>,
    /// Já normalizado (ver `search_term`): prefixo do id do pedido ou do cliente.
    pub search: Option<String>,
    pub limit: i64,
    pub offset: i64,
}

/// Parâmetro único da contagem: o mesmo filtro da listagem, sem a página.
#[derive(Debug, SurrealValue)]
pub struct OrderFilter {
    pub status: Option<String>,
    pub search: Option<String>,
}

/// O termo da busca como o banco o compara: sem espaços nas pontas, sem o `#` do número
/// mostrado na tela e em minúsculas (os ids são minúsculos). Vazio vira `None` (sem busca).
pub fn search_term(raw: Option<&str>) -> Option<String> {
    let term = raw?.trim().trim_start_matches('#').trim().to_lowercase();
    (!term.is_empty()).then_some(term)
}

/// Parâmetro único da troca de status.
#[derive(Debug, SurrealValue)]
pub struct StatusChange {
    pub id: RecordId,
    pub from: String,
    pub to: String,
}

/// Filtros da exportação: os mesmos da listagem de admin, sem página (o arquivo traz tudo o que
/// casa, até o teto).
#[derive(Debug, Deserialize, Validate)]
pub struct ExportOrdersQuery {
    pub status: Option<OrderStatus>,
    /// Número do pedido (com ou sem `#`) ou id do cliente, por prefixo.
    #[validate(length(max = 64, message = "must be at most 64 characters"))]
    pub q: Option<String>,
}
