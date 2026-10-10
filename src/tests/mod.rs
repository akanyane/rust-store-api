//! Testes de integração: cada teste sobe o app completo (rotas, services, schema) com um
//! banco SurrealKV novo num diretório temporário e dispara requisições em memória
//! contra o `Router`, sem abrir porta.

mod admin_orders;
mod auth;
mod cart;
mod catalog;
mod orders;
mod sessions;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use surrealdb::{Surreal, engine::any::Any};
use tempfile::TempDir;
use tower::ServiceExt;

use crate::executor::Executor;
use crate::repositories::variant as variant_repo;
use crate::services::auth as auth_service;
use crate::state::AppState;
use crate::{build_router, db};

pub const PASSWORD: &str = "password123";
const ADMIN_USERNAME: &str = "admin";
const ADMIN_PASSWORD: &str = "supersecret1";

pub struct TestApp {
    pub router: Router,
    pub db: Surreal<Any>,
    // Mantém o diretório vivo enquanto o teste roda; apaga ao soltar.
    _dir: TempDir,
}

/// Produto criado por `setup_product`: o `POST /products` já cria a variante padrão.
pub struct Item {
    pub product_id: String,
    pub variant_id: String,
}

pub async fn send_raw(
    router: &Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<String>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    if body.is_some() {
        builder = builder.header(header::CONTENT_TYPE, "application/json");
    }
    let request = builder
        .body(Body::from(body.unwrap_or_default()))
        .expect("request");

    let response = router.clone().oneshot(request).await.expect("response");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let json = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, json)
}

pub async fn send(
    router: &Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    send_raw(router, method, uri, token, body.map(|b| b.to_string())).await
}

impl TestApp {
    pub async fn new() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let url = format!("surrealkv://{}", dir.path().join("db").display());
        let db = db::connect_db_at(&url).await.expect("connect db");
        let router = build_router(AppState { db: db.clone() });
        TestApp {
            router,
            db,
            _dir: dir,
        }
    }

    pub async fn send(
        &self,
        method: &str,
        uri: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        send(&self.router, method, uri, token, body).await
    }

    pub async fn sign_up(&self, username: &str) -> (StatusCode, Value) {
        self.send(
            "POST",
            "/auth/sign-up",
            None,
            Some(json!({
                "username": username,
                "password": PASSWORD,
                "first_name": "Test",
                "last_name": "User",
                "birthday": "1990-01-01",
            })),
        )
        .await
    }

    /// Devolve `(session_token, refresh_token)`.
    pub async fn sign_in(&self, username: &str, password: &str) -> (StatusCode, String, String) {
        let (status, body) = self
            .send(
                "POST",
                "/auth/sign-in",
                None,
                Some(json!({ "username": username, "password": password })),
            )
            .await;
        let field = |name: &str| body[name].as_str().unwrap_or_default().to_string();
        (status, field("session_token"), field("refresh_token"))
    }

    /// Cadastra e loga um cliente; devolve o session token.
    pub async fn customer(&self, username: &str) -> String {
        let (status, _) = self.sign_up(username).await;
        assert_eq!(status, StatusCode::CREATED, "sign-up of {username}");
        let (status, session, _) = self.sign_in(username, PASSWORD).await;
        assert_eq!(status, StatusCode::OK, "sign-in of {username}");
        session
    }

    /// Cria o admin (o mesmo caminho do seed da partida) e devolve o session token.
    pub async fn admin(&self) -> String {
        auth_service::ensure_admin(&self.db, ADMIN_USERNAME, ADMIN_PASSWORD.to_string())
            .await
            .expect("ensure_admin");
        let (status, session, _) = self.sign_in(ADMIN_USERNAME, ADMIN_PASSWORD).await;
        assert_eq!(status, StatusCode::OK, "admin sign-in");
        session
    }

    pub async fn setup_product(&self, admin: &str, name: &str, price: i64, stock: i64) -> Item {
        let (status, body) = self
            .send(
                "POST",
                "/products",
                Some(admin),
                Some(json!({
                    "name": name,
                    "description": "test product",
                    "price": price,
                    "stock": stock,
                })),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "create product {name}");
        let product_id = body["id"].as_str().expect("product id").to_string();

        let (status, variants) = self
            .send(
                "GET",
                &format!("/products/{product_id}/variants"),
                None,
                None,
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        let variant_id = variants[0]["id"].as_str().expect("variant id").to_string();

        Item {
            product_id,
            variant_id,
        }
    }

    /// Estoque lido direto do banco, sem passar pelos filtros do catálogo público.
    pub async fn stock(&self, item: &Item) -> i32 {
        variant_repo::find_by_id(&Executor::Db(&self.db), &item.variant_id)
            .await
            .expect("query variant")
            .expect("variant exists")
            .stock
    }

    pub async fn add_to_cart(&self, token: &str, item: &Item, quantity: i64) -> StatusCode {
        self.send(
            "POST",
            "/cart/items",
            Some(token),
            Some(json!({ "variant_id": item.variant_id, "quantity": quantity })),
        )
        .await
        .0
    }
}
