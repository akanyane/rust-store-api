use axum::http::StatusCode;
use serde_json::{Value, json};

use super::orders::place_order;
use super::{Item, TestApp};

async fn customer_id(app: &TestApp, token: &str) -> String {
    let (status, me) = app.send("GET", "/auth/session", Some(token), None).await;
    assert_eq!(status, StatusCode::OK);
    me["id"].as_str().expect("id").to_string()
}

async fn view(app: &TestApp, admin: &str, id: &str) -> Value {
    let (status, body) = app
        .send("GET", &format!("/admin/customers/{id}"), Some(admin), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

async fn set_status(app: &TestApp, admin: &str, order_id: &str, status: &str) {
    let (code, body) = app
        .send(
            "PUT",
            &format!("/admin/orders/{order_id}/status"),
            Some(admin),
            Some(json!({ "status": status })),
        )
        .await;
    assert_eq!(code, StatusCode::OK, "moving to {status}: {body}");
}

async fn buy(app: &TestApp, customer: &str, item: &Item, quantity: i64) -> String {
    place_order(app, customer, item, quantity).await
}

#[tokio::test]
async fn the_customer_view_requires_an_admin() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let ana = app.customer("ana").await;
    let id = customer_id(&app, &ana).await;
    let uri = format!("/admin/customers/{id}");

    let (status, _) = app.send("GET", &uri, None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = app.send("GET", &uri, Some("nope"), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = app.send("GET", &uri, Some(&ana), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = app.send("GET", &uri, Some(&admin), None).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn an_unknown_id_and_the_admins_own_id_are_404() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let admin_id = customer_id(&app, &admin).await;

    for id in ["nope", "0000000000", admin_id.as_str()] {
        let (status, _) = app
            .send("GET", &format!("/admin/customers/{id}"), Some(&admin), None)
            .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "id={id}");
    }
}

#[tokio::test]
async fn a_customer_who_never_bought_has_zeros_and_no_last_order() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let ana = app.customer("ana").await;
    let id = customer_id(&app, &ana).await;

    let body = view(&app, &admin, &id).await;

    assert_eq!(body["id"], id);
    assert_eq!(body["username"], "ana");
    assert_eq!(
        body["orders"],
        json!({
            "total": 0, "pending": 0, "paid": 0,
            "shipped": 0, "delivered": 0, "cancelled": 0
        })
    );
    assert_eq!(body["spent"], 0);
    assert_eq!(body["last_order_at"], Value::Null);
    assert!(body["created_at"].as_str().is_some(), "{body}");
}

#[tokio::test]
async fn orders_are_counted_per_status_and_only_paid_ones_are_spent() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let ana = app.customer("ana").await;
    let id = customer_id(&app, &ana).await;
    let mug = app.setup_product(&admin, "Mug", 10, 50).await;

    // pending: 10, paid: 20, shipped: 10, delivered: 30, cancelled: 10
    let _pending = buy(&app, &ana, &mug, 1).await;
    let paid = buy(&app, &ana, &mug, 2).await;
    let shipped = buy(&app, &ana, &mug, 1).await;
    let delivered = buy(&app, &ana, &mug, 3).await;
    let cancelled = buy(&app, &ana, &mug, 1).await;
    set_status(&app, &admin, &paid, "paid").await;
    for step in ["paid", "shipped"] {
        set_status(&app, &admin, &shipped, step).await;
    }
    for step in ["paid", "shipped", "delivered"] {
        set_status(&app, &admin, &delivered, step).await;
    }
    set_status(&app, &admin, &cancelled, "cancelled").await;

    let body = view(&app, &admin, &id).await;

    assert_eq!(
        body["orders"],
        json!({
            "total": 5, "pending": 1, "paid": 1,
            "shipped": 1, "delivered": 1, "cancelled": 1
        })
    );
    assert_eq!(body["spent"], 60);
}

#[tokio::test]
async fn one_customers_view_never_counts_anothers_orders() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let ana = app.customer("ana").await;
    let bob = app.customer("bob").await;
    let ana_id = customer_id(&app, &ana).await;
    let bob_id = customer_id(&app, &bob).await;
    let mug = app.setup_product(&admin, "Mug", 10, 50).await;
    let a = buy(&app, &ana, &mug, 1).await;
    set_status(&app, &admin, &a, "paid").await;
    for quantity in [2, 3] {
        let b = buy(&app, &bob, &mug, quantity).await;
        set_status(&app, &admin, &b, "paid").await;
    }

    let ana_view = view(&app, &admin, &ana_id).await;
    let bob_view = view(&app, &admin, &bob_id).await;

    assert_eq!(ana_view["orders"]["total"], 1);
    assert_eq!(ana_view["spent"], 10);
    assert_eq!(bob_view["orders"]["total"], 2);
    assert_eq!(bob_view["spent"], 50);
}

#[tokio::test]
async fn the_view_has_only_the_documented_fields_and_no_personal_data() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let ana = app.customer("ana").await;
    let id = customer_id(&app, &ana).await;

    let body = view(&app, &admin, &id).await;

    let mut keys: Vec<&str> = body
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort();
    assert_eq!(
        keys,
        vec![
            "created_at",
            "id",
            "last_order_at",
            "orders",
            "spent",
            "username"
        ]
    );
    let text = body.to_string();
    for forbidden in [
        "first_name",
        "last_name",
        "birthday",
        "password",
        "1990-01-01",
        "Test",
    ] {
        assert!(!text.contains(forbidden), "{forbidden} leaked: {text}");
    }
}

#[tokio::test]
async fn the_last_order_is_the_newest_one() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let ana = app.customer("ana").await;
    let id = customer_id(&app, &ana).await;
    let mug = app.setup_product(&admin, "Mug", 10, 50).await;
    for _ in 0..3 {
        buy(&app, &ana, &mug, 1).await;
    }

    let body = view(&app, &admin, &id).await;

    let (_, orders) = app
        .send("GET", &format!("/admin/orders?q={id}"), Some(&admin), None)
        .await;
    let newest = orders[0]["created_at"].as_str().unwrap();
    assert_eq!(body["last_order_at"], newest, "the list comes newest first");
    // A conta é mais antiga que o primeiro pedido.
    let oldest = orders[2]["created_at"].as_str().unwrap();
    assert!(body["created_at"].as_str().unwrap() <= oldest, "{body}");
}

#[tokio::test]
async fn the_username_is_shown_the_way_it_is_stored() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let ana = app.customer("Ana.Lima").await;
    let id = customer_id(&app, &ana).await;

    assert_eq!(view(&app, &admin, &id).await["username"], "ana.lima");
}

#[tokio::test]
async fn a_deleted_account_is_gone_from_the_admin_view() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let ana = app.customer("ana").await;
    let id = customer_id(&app, &ana).await;
    view(&app, &admin, &id).await;

    let (status, _) = app.send("DELETE", "/me", Some(&ana), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, _) = app
        .send("GET", &format!("/admin/customers/{id}"), Some(&admin), None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
