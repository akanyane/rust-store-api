use chrono::{DateTime, Utc};
use serde::Serialize;
use surrealdb::types::{Datetime, RecordId, SurrealValue};

use super::id_to_string;

#[derive(Debug, Clone, SurrealValue)]
pub struct OrderRecord {
    pub id: RecordId,
    pub customer: RecordId,
    pub status: String,
    pub total_cents: i64,
    pub created_at: Datetime,
}

/// O `status` fica de fora: o schema grava `pending` por padrão.
#[derive(Debug, SurrealValue)]
pub struct NewOrder {
    pub customer: RecordId,
    pub total_cents: i64,
}

#[derive(Debug, Clone, SurrealValue)]
pub struct OrderItemRecord {
    pub id: RecordId,
    pub order: RecordId,
    pub variant: RecordId,
    pub name: String,
    pub sku: String,
    pub unit_price_cents: i64,
    pub quantity: i32,
}

#[derive(Debug, SurrealValue)]
pub struct NewOrderItem {
    pub order: RecordId,
    pub variant: RecordId,
    pub name: String,
    pub sku: String,
    pub unit_price_cents: i64,
    pub quantity: i32,
}

#[derive(Debug, Serialize)]
pub struct OrderView {
    pub id: String,
    pub status: String,
    pub total_cents: i64,
    pub created_at: DateTime<Utc>,
    pub items: Vec<OrderItemView>,
}

#[derive(Debug, Serialize)]
pub struct OrderItemView {
    pub variant_id: String,
    pub name: String,
    pub sku: String,
    pub unit_price_cents: i64,
    pub quantity: i32,
    pub line_total_cents: i64,
}

impl OrderItemView {
    /// `None` só se o total da linha estourar `i64`, o que o checkout já impede.
    pub fn from_record(record: OrderItemRecord) -> Option<Self> {
        let line_total_cents = record
            .unit_price_cents
            .checked_mul(i64::from(record.quantity))?;

        Some(OrderItemView {
            variant_id: id_to_string(&record.variant),
            name: record.name,
            sku: record.sku,
            unit_price_cents: record.unit_price_cents,
            quantity: record.quantity,
            line_total_cents,
        })
    }
}
