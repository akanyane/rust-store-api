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
use validator::ValidationError;

/// Rejects empty or whitespace-only text (the validator `length` does not trim).
pub fn validate_not_blank(value: &str) -> Result<(), ValidationError> {
    if value.trim().is_empty() {
        return Err(ValidationError::new("blank"));
    }
    Ok(())
}

/// A birth date in the future makes no sense.
pub fn validate_not_future(date: &chrono::NaiveDate) -> Result<(), ValidationError> {
    if *date > chrono::Utc::now().date_naive() {
        return Err(ValidationError::new("future_date"));
    }
    Ok(())
}

/// Devolve só a chave do ID (ex.: `abc123` de `product:abc123`), usada nas URLs.
pub fn id_to_string(id: &RecordId) -> String {
    match &id.key {
        RecordIdKey::String(s) => s.clone(),
        RecordIdKey::Number(n) => n.to_string(),
        outra => format!("{outra:?}"),
    }
}
