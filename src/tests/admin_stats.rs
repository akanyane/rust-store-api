use axum::http::StatusCode;
use serde_json::{Value, json};

use super::orders::place_order;
use super::{Item, TestApp};

async fn stats(app: &TestApp, admin: &str) -> Value {
    let (status, body) = app.send("GET", "/admin/stats", Some(admin), None).await;
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

fn low_stock_stocks(body: &Value) -> Vec<i64> {
    body["low_stock"]
        .as_array()
        .expect("low_stock list")
        .iter()
        .map(|item| item["stock"].as_i64().expect("stock"))
        .collect()
}

#[tokio::test]
async fn stats_require_an_admin() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;

    let (status, _) = app.send("GET", "/admin/stats", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = app.send("GET", "/admin/stats", Some("nope"), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = app.send("GET", "/admin/stats", Some(&customer), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = app.send("GET", "/admin/stats", Some(&admin), None).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn an_empty_store_has_zeros_and_no_low_stock() {
    let app = TestApp::new().await;
    let admin = app.admin().await;

    let body = stats(&app, &admin).await;

    assert_eq!(
        body,
        json!({
            "orders": {
                "total": 0, "pending": 0, "paid": 0,
                "shipped": 0, "delivered": 0, "cancelled": 0
            },
            "revenue": 0,
            "products": { "active": 0, "inactive": 0 },
            "low_stock": [],
        })
    );
}

#[tokio::test]
async fn orders_are_counted_per_status_and_only_paid_ones_are_revenue() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 50).await;

    // pending: 10, paid: 20, shipped: 10, delivered: 30, cancelled: 10
    let _pending = place_order(&app, &customer, &mug, 1).await;
    let paid = place_order(&app, &customer, &mug, 2).await;
    let shipped = place_order(&app, &customer, &mug, 1).await;
    let delivered = place_order(&app, &customer, &mug, 3).await;
    let cancelled = place_order(&app, &customer, &mug, 1).await;
    set_status(&app, &admin, &paid, "paid").await;
    for step in ["paid", "shipped"] {
        set_status(&app, &admin, &shipped, step).await;
    }
    for step in ["paid", "shipped", "delivered"] {
        set_status(&app, &admin, &delivered, step).await;
    }
    set_status(&app, &admin, &cancelled, "cancelled").await;

    let body = stats(&app, &admin).await;

    assert_eq!(
        body["orders"],
        json!({
            "total": 5, "pending": 1, "paid": 1,
            "shipped": 1, "delivered": 1, "cancelled": 1
        })
    );
    // 20 (paid) + 10 (shipped) + 30 (delivered); pending and cancelled are not revenue.
    assert_eq!(body["revenue"], 60);
}

#[tokio::test]
async fn several_orders_in_the_same_status_add_up() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 7, 50).await;

    for quantity in [1, 2, 3] {
        let id = place_order(&app, &customer, &mug, quantity).await;
        set_status(&app, &admin, &id, "paid").await;
    }

    let body = stats(&app, &admin).await;

    assert_eq!(body["orders"]["paid"], 3);
    assert_eq!(body["orders"]["total"], 3);
    assert_eq!(body["revenue"], 7 * (1 + 2 + 3));
}

#[tokio::test]
async fn products_are_counted_as_active_or_inactive() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let one = app.setup_product(&admin, "One", 10, 10).await;
    app.setup_product(&admin, "Two", 10, 10).await;
    app.setup_product(&admin, "Three", 10, 10).await;

    let body = stats(&app, &admin).await;
    assert_eq!(body["products"], json!({ "active": 3, "inactive": 0 }));

    let (status, _) = app
        .send(
            "DELETE",
            &format!("/products/{}", one.product_id),
            Some(&admin),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let body = stats(&app, &admin).await;
    assert_eq!(body["products"], json!({ "active": 2, "inactive": 1 }));
}

async fn deactivate_variant(app: &TestApp, admin: &str, item: &Item) {
    let (status, _) = app
        .send(
            "DELETE",
            &format!("/products/{}/variants/{}", item.product_id, item.variant_id),
            Some(admin),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn low_stock_lists_active_variants_up_to_five_units_emptiest_first() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    app.setup_product(&admin, "Six", 10, 6).await;
    app.setup_product(&admin, "Five", 10, 5).await;
    app.setup_product(&admin, "Two", 10, 2).await;
    app.setup_product(&admin, "Zero", 10, 0).await;

    let body = stats(&app, &admin).await;

    assert_eq!(low_stock_stocks(&body), vec![0, 2, 5]);
    let names: Vec<&str> = body["low_stock"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["product_name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["Zero", "Two", "Five"]);
}

#[tokio::test]
async fn low_stock_items_carry_the_ids_names_and_sku_to_find_them() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let tote = app.setup_product(&admin, "Tote", 40, 3).await;

    let body = stats(&app, &admin).await;

    let item = &body["low_stock"][0];
    assert_eq!(item["product_id"], tote.product_id);
    assert_eq!(item["variant_id"], tote.variant_id);
    assert_eq!(item["product_name"], "Tote");
    assert_eq!(item["variant_name"], "Default");
    assert_eq!(item["stock"], 3);
    assert_eq!(item["sku"], format!("SKU-{}", tote.product_id));
    let keys: Vec<&String> = item.as_object().unwrap().keys().collect();
    assert_eq!(keys.len(), 6, "only the documented fields: {keys:?}");
}

#[tokio::test]
async fn low_stock_ignores_inactive_variants_and_inactive_products() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let kept = app.setup_product(&admin, "Kept", 10, 1).await;
    let hidden_variant = app.setup_product(&admin, "Hidden variant", 10, 1).await;
    let hidden_product = app.setup_product(&admin, "Hidden product", 10, 1).await;
    deactivate_variant(&app, &admin, &hidden_variant).await;
    let (status, _) = app
        .send(
            "DELETE",
            &format!("/products/{}", hidden_product.product_id),
            Some(&admin),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let body = stats(&app, &admin).await;

    let listed = body["low_stock"].as_array().unwrap();
    assert_eq!(listed.len(), 1, "{body}");
    assert_eq!(listed[0]["variant_id"], kept.variant_id);
}

#[tokio::test]
async fn low_stock_lists_at_most_ten() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    for n in 0..12 {
        app.setup_product(&admin, &format!("Item {n}"), 10, 1).await;
    }

    let body = stats(&app, &admin).await;

    assert_eq!(low_stock_stocks(&body), vec![1; 10]);
}

#[tokio::test]
async fn a_sale_that_drains_the_stock_moves_the_item_into_low_stock() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 7).await;
    assert!(low_stock_stocks(&stats(&app, &admin).await).is_empty());

    place_order(&app, &customer, &mug, 3).await;

    assert_eq!(low_stock_stocks(&stats(&app, &admin).await), vec![4]);
}
