mod db;
mod error;
mod executor;
mod extractors;
mod handlers;
mod models;
mod repositories;
mod services;
mod state;

use axum::{
    Router,
    routing::{get, post, put},
};

use handlers::auth as auth_handlers;
use handlers::cart as cart_handlers;
use handlers::customer as customer_handlers;
use handlers::order as order_handlers;
use handlers::product as product_handlers;
use handlers::variant as variant_handlers;
use services::auth as auth_service;
use state::AppState;
use surrealdb::{Surreal, engine::any::Any};

#[cfg(test)]
mod tests;

/// Cria o admin a partir de ADMIN_USERNAME e ADMIN_PASSWORD (ambos ou nenhum).
/// Valor vazio conta como ausente, porque o `.env.example` traz as chaves vazias.
async fn seed_admin(db: &Surreal<Any>) {
    let read = |key: &str| std::env::var(key).ok().filter(|value| !value.is_empty());

    match (read("ADMIN_USERNAME"), read("ADMIN_PASSWORD")) {
        (Some(username), Some(password)) => {
            auth_service::ensure_admin(db, &username, password)
                .await
                .unwrap_or_else(|e| panic!("failed to prepare admin: {e:?}"));
        }
        (None, None) => {
            println!(
                "ADMIN_USERNAME and ADMIN_PASSWORD not set: starting the store without an admin"
            )
        }
        _ => panic!("set ADMIN_USERNAME and ADMIN_PASSWORD together, or neither"),
    }
}

const SESSION_CLEANUP_INTERVAL: std::time::Duration = std::time::Duration::from_secs(60 * 60);

/// Limpa sessões expiradas na partida e depois a cada hora. Falha só é logada: o
/// próximo ciclo tenta de novo e o servidor continua de pé.
fn spawn_session_cleanup(db: Surreal<Any>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(SESSION_CLEANUP_INTERVAL);
        loop {
            // O primeiro tick é imediato, então a primeira limpeza roda na partida.
            interval.tick().await;
            match auth_service::purge_expired_sessions(&db).await {
                Ok(count) => println!("Purged {count} expired session(s)"),
                Err(e) => eprintln!("Session cleanup failed: {e:?}"),
            }
        }
    });
}

fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/auth/sign-up", post(auth_handlers::sign_up))
        .route("/auth/sign-in", post(auth_handlers::sign_in))
        .route("/auth/sign-out", post(auth_handlers::sign_out))
        .route("/auth/refresh", post(auth_handlers::refresh))
        .route(
            "/products",
            get(product_handlers::list_products).post(product_handlers::create_product),
        )
        .route(
            "/products/{id}",
            get(product_handlers::get_product)
                .put(product_handlers::update_product)
                .delete(product_handlers::delete_product),
        )
        .route(
            "/products/{id}/variants",
            get(variant_handlers::list_variants).post(variant_handlers::create_variant),
        )
        .route(
            "/products/{id}/variants/{variant_id}",
            put(variant_handlers::update_variant).delete(variant_handlers::delete_variant),
        )
        .route(
            "/me",
            get(customer_handlers::get_customer)
                .put(customer_handlers::update_customer)
                .delete(customer_handlers::delete_customer),
        )
        .route("/cart", get(cart_handlers::get_cart))
        .route("/cart/items", post(cart_handlers::add_item))
        .route(
            "/cart/items/{variant_id}",
            put(cart_handlers::update_item).delete(cart_handlers::remove_item),
        )
        .route(
            "/orders",
            get(order_handlers::list_orders).post(order_handlers::checkout),
        )
        .route("/orders/{id}", get(order_handlers::get_order))
        .route("/orders/{id}/cancel", post(order_handlers::cancel_order))
        .route("/orders/{id}/pay", post(order_handlers::pay_order))
        .route("/admin/orders", get(order_handlers::admin_list_orders))
        .route("/admin/orders/{id}", get(order_handlers::admin_get_order))
        .route(
            "/admin/orders/{id}/status",
            put(order_handlers::admin_update_status),
        )
        .with_state(state)
}

#[tokio::main]
async fn main() {
    // Em produção o .env pode não existir; nesse caso valem as variáveis do ambiente.
    dotenvy::dotenv().ok();

    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()
        .expect("PORT must be a number between 0 and 65535");

    let db = db::connect_db().await.unwrap();
    seed_admin(&db).await;
    spawn_session_cleanup(db.clone());
    let state = AppState { db };

    let app = build_router(state);

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port))
        .await
        .unwrap();

    println!("Rust Store running at http://localhost:{port}");
    axum::serve(listener, app).await.unwrap();
}
