pub mod auth;
pub mod cart;
pub mod cart_item;
pub mod customer;
pub mod order;
pub mod product;
pub mod session;
pub mod user;
pub mod variant;

use surrealdb::types::{RecordId, RecordIdKey};

/// Devolve só a chave do ID (ex.: `abc123` de `product:abc123`), usada nas URLs.
pub fn id_to_string(id: &RecordId) -> String {
    match &id.key {
        RecordIdKey::String(s) => s.clone(),
        RecordIdKey::Number(n) => n.to_string(),
        outra => format!("{outra:?}"),
    }
}
