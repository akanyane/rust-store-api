pub mod auth;
pub mod cart;
pub mod customer;
pub mod order;
pub mod product;
pub mod variant;

use axum::http::HeaderName;

/// Cabeçalho `X-Total-Count` das listagens paginadas: o corpo segue sendo a lista simples.
pub fn total_count(total: i64) -> [(HeaderName, String); 1] {
    [(HeaderName::from_static("x-total-count"), total.to_string())]
}
