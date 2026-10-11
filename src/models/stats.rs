use serde::Serialize;
use surrealdb::types::{RecordId, SurrealValue};

use super::id_to_string;

/// Uma linha de `GROUP BY status`: quantos pedidos e a soma dos totais.
#[derive(Debug, SurrealValue)]
pub struct OrderStatusRow {
    pub status: String,
    pub count: i64,
    pub revenue: i64,
}

/// Uma linha de `GROUP BY active`.
#[derive(Debug, SurrealValue)]
pub struct ActiveCountRow {
    pub active: bool,
    pub count: i64,
}

/// Parâmetro único da consulta de estoque baixo (o `query_all` aceita só um).
#[derive(Debug, SurrealValue)]
pub struct LowStockFilter {
    pub threshold: i64,
    pub limit: i64,
}

#[derive(Debug, SurrealValue)]
pub struct LowStockRow {
    pub id: RecordId,
    pub product: RecordId,
    pub name: String,
    pub sku: String,
    pub stock: i32,
    pub product_name: String,
}

#[derive(Debug, Default, Serialize)]
pub struct OrderCounts {
    pub total: i64,
    pub pending: i64,
    pub paid: i64,
    pub shipped: i64,
    pub delivered: i64,
    pub cancelled: i64,
}

#[derive(Debug, Default, Serialize)]
pub struct ProductCounts {
    pub active: i64,
    pub inactive: i64,
}

#[derive(Debug, Serialize)]
pub struct LowStockItem {
    pub product_id: String,
    pub product_name: String,
    pub variant_id: String,
    pub variant_name: String,
    pub sku: String,
    pub stock: i32,
}

impl From<LowStockRow> for LowStockItem {
    fn from(row: LowStockRow) -> Self {
        LowStockItem {
            product_id: id_to_string(&row.product),
            product_name: row.product_name,
            variant_id: id_to_string(&row.id),
            variant_name: row.name,
            sku: row.sku,
            stock: row.stock,
        }
    }
}

/// Os números do painel de admin.
#[derive(Debug, Serialize)]
pub struct StatsView {
    pub orders: OrderCounts,
    /// Soma do total dos pedidos pagos, enviados e entregues, em unidades.
    pub revenue: i64,
    pub products: ProductCounts,
    pub low_stock: Vec<LowStockItem>,
}
