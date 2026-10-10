use axum::http::StatusCode;
use serde_json::json;

use super::TestApp;

#[tokio::test]
async fn empty_cart_for_a_customer_without_one() {
    let app = TestApp::new().await;
    let customer = app.customer("ana").await;

    let (status, cart) = app.send("GET", "/cart", Some(&customer), None).await;

    assert_eq!(status, StatusCode::OK);
    assert!(cart["items"].as_array().unwrap().is_empty());
    assert_eq!(cart["total"], 0);
}

#[tokio::test]
async fn adding_the_same_variant_sums_the_quantity() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let item = app.setup_product(&admin, "Mug", 10, 10).await;

    assert_eq!(app.add_to_cart(&customer, &item, 2).await, StatusCode::OK);
    assert_eq!(app.add_to_cart(&customer, &item, 3).await, StatusCode::OK);

    let (_, cart) = app.send("GET", "/cart", Some(&customer), None).await;
    let items = cart["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["quantity"], 5);
    assert_eq!(items[0]["unit_price"], 10);
    assert_eq!(items[0]["line_total"], 50);
    assert_eq!(cart["total"], 50);
}

#[tokio::test]
async fn quantity_must_be_at_least_one() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let item = app.setup_product(&admin, "Mug", 10, 10).await;

    assert_eq!(
        app.add_to_cart(&customer, &item, 0).await,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        app.add_to_cart(&customer, &item, -1).await,
        StatusCode::UNPROCESSABLE_ENTITY
    );

    app.add_to_cart(&customer, &item, 1).await;
    let (status, _) = app
        .send(
            "PUT",
            &format!("/cart/items/{}", item.variant_id),
            Some(&customer),
            Some(json!({ "quantity": 0 })),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn cannot_put_more_in_the_cart_than_the_stock() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let item = app.setup_product(&admin, "Mug", 10, 3).await;

    assert_eq!(
        app.add_to_cart(&customer, &item, 4).await,
        StatusCode::CONFLICT
    );
    assert_eq!(app.add_to_cart(&customer, &item, 3).await, StatusCode::OK);
    // A soma com o que já está no carrinho também conta.
    assert_eq!(
        app.add_to_cart(&customer, &item, 1).await,
        StatusCode::CONFLICT
    );

    let (status, _) = app
        .send(
            "PUT",
            &format!("/cart/items/{}", item.variant_id),
            Some(&customer),
            Some(json!({ "quantity": 4 })),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn inactive_product_or_variant_cannot_be_added() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let product_off = app.setup_product(&admin, "Cap", 10, 5).await;
    let variant_off = app.setup_product(&admin, "Mug", 10, 5).await;

    app.send(
        "DELETE",
        &format!("/products/{}", product_off.product_id),
        Some(&admin),
        None,
    )
    .await;
    app.send(
        "DELETE",
        &format!(
            "/products/{}/variants/{}",
            variant_off.product_id, variant_off.variant_id
        ),
        Some(&admin),
        None,
    )
    .await;

    assert_eq!(
        app.add_to_cart(&customer, &product_off, 1).await,
        StatusCode::CONFLICT
    );
    assert_eq!(
        app.add_to_cart(&customer, &variant_off, 1).await,
        StatusCode::CONFLICT
    );
}

#[tokio::test]
async fn each_customer_has_their_own_cart() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let ana = app.customer("ana").await;
    let bob = app.customer("bob").await;
    let item = app.setup_product(&admin, "Mug", 10, 10).await;

    app.add_to_cart(&ana, &item, 2).await;

    let (_, bob_cart) = app.send("GET", "/cart", Some(&bob), None).await;
    assert!(bob_cart["items"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn removing_an_item_empties_the_cart_line() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let item = app.setup_product(&admin, "Mug", 10, 10).await;
    app.add_to_cart(&customer, &item, 2).await;

    let (status, cart) = app
        .send(
            "DELETE",
            &format!("/cart/items/{}", item.variant_id),
            Some(&customer),
            None,
        )
        .await;

    assert_eq!(status, StatusCode::OK);
    assert!(cart["items"].as_array().unwrap().is_empty());
}

async fn cart_of(app: &TestApp, token: &str) -> serde_json::Value {
    let (status, cart) = app.send("GET", "/cart", Some(token), None).await;
    assert_eq!(status, StatusCode::OK);
    cart
}

/// Troca estoque e `active` da variante padrão pelo admin (preço 10, como nos testes).
async fn set_variant(app: &TestApp, admin: &str, item: &super::Item, stock: i64, active: bool) {
    let (status, _) = app
        .send(
            "PUT",
            &format!("/products/{}/variants/{}", item.product_id, item.variant_id),
            Some(admin),
            Some(json!({
                "name": "Default",
                "sku": format!("SKU-{}", item.product_id),
                "price": 10,
                "stock": stock,
                "active": active,
            })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
}

async fn set_product_active(app: &TestApp, admin: &str, item: &super::Item, active: bool) {
    let (status, _) = app
        .send(
            "PUT",
            &format!("/products/{}", item.product_id),
            Some(admin),
            Some(json!({ "name": "Mug", "description": "d", "active": active })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn an_available_item_is_flagged_as_available() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    app.add_to_cart(&customer, &mug, 2).await;

    let cart = cart_of(&app, &customer).await;

    assert_eq!(cart["items"][0]["available"], true);
    assert!(cart["items"][0]["unavailable_reason"].is_null());
    assert_eq!(cart["can_checkout"], true);
}

#[tokio::test]
async fn an_empty_cart_cannot_check_out() {
    let app = TestApp::new().await;
    let customer = app.customer("ana").await;

    assert_eq!(cart_of(&app, &customer).await["can_checkout"], false);
}

#[tokio::test]
async fn a_deactivated_variant_or_product_is_flagged_inactive() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let variant_off = app.setup_product(&admin, "Mug", 10, 5).await;
    let product_off = app.setup_product(&admin, "Cap", 10, 5).await;
    app.add_to_cart(&customer, &variant_off, 1).await;
    app.add_to_cart(&customer, &product_off, 1).await;

    set_variant(&app, &admin, &variant_off, 5, false).await;
    set_product_active(&app, &admin, &product_off, false).await;

    let cart = cart_of(&app, &customer).await;
    for item in cart["items"].as_array().unwrap() {
        assert_eq!(item["available"], false, "{item}");
        assert_eq!(item["unavailable_reason"], "inactive", "{item}");
    }
    assert_eq!(cart["can_checkout"], false);
}

#[tokio::test]
async fn stock_below_the_quantity_is_flagged_and_the_exact_amount_is_fine() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    app.add_to_cart(&customer, &mug, 3).await;

    for (stock, available) in [(2, false), (0, false), (3, true), (4, true)] {
        set_variant(&app, &admin, &mug, stock, true).await;
        let cart = cart_of(&app, &customer).await;
        let item = &cart["items"][0];
        assert_eq!(item["available"], available, "stock {stock}: {item}");
        if available {
            assert!(item["unavailable_reason"].is_null(), "stock {stock}");
        } else {
            assert_eq!(
                item["unavailable_reason"], "insufficient_stock",
                "stock {stock}"
            );
        }
        assert_eq!(cart["can_checkout"], available, "stock {stock}");
    }
}

#[tokio::test]
async fn only_the_affected_items_are_flagged_and_the_total_still_sums_everything() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    let cap = app.setup_product(&admin, "Cap", 10, 5).await;
    app.add_to_cart(&customer, &mug, 2).await;
    app.add_to_cart(&customer, &cap, 3).await;
    set_variant(&app, &admin, &cap, 5, false).await;

    let cart = cart_of(&app, &customer).await;

    let flag_of = |variant_id: &str| {
        cart["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["variant_id"] == variant_id)
            .unwrap()["available"]
            .clone()
    };
    assert_eq!(flag_of(&mug.variant_id), true);
    assert_eq!(flag_of(&cap.variant_id), false);
    assert_eq!(cart["total"], 2 * 10 + 3 * 10);
    assert_eq!(cart["can_checkout"], false);
}

#[tokio::test]
async fn the_flag_clears_when_the_item_comes_back() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    app.add_to_cart(&customer, &mug, 2).await;

    set_variant(&app, &admin, &mug, 5, false).await;
    assert_eq!(cart_of(&app, &customer).await["can_checkout"], false);
    set_variant(&app, &admin, &mug, 5, true).await;
    assert_eq!(cart_of(&app, &customer).await["can_checkout"], true);

    set_variant(&app, &admin, &mug, 1, true).await;
    assert_eq!(cart_of(&app, &customer).await["can_checkout"], false);
    set_variant(&app, &admin, &mug, 9, true).await;
    assert_eq!(cart_of(&app, &customer).await["can_checkout"], true);
}

#[tokio::test]
async fn every_cart_response_carries_the_flags() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    let cap = app.setup_product(&admin, "Cap", 10, 5).await;
    app.add_to_cart(&customer, &cap, 1).await;
    set_variant(&app, &admin, &cap, 5, false).await;

    let (_, after_add) = app
        .send(
            "POST",
            "/cart/items",
            Some(&customer),
            Some(json!({ "variant_id": mug.variant_id, "quantity": 1 })),
        )
        .await;
    let (_, after_update) = app
        .send(
            "PUT",
            &format!("/cart/items/{}", mug.variant_id),
            Some(&customer),
            Some(json!({ "quantity": 2 })),
        )
        .await;
    let (_, after_remove) = app
        .send(
            "DELETE",
            &format!("/cart/items/{}", mug.variant_id),
            Some(&customer),
            None,
        )
        .await;

    for (name, view) in [
        ("add", after_add),
        ("update", after_update),
        ("remove", after_remove),
    ] {
        assert_eq!(view["can_checkout"], false, "{name}");
        let flagged = view["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["variant_id"] == cap.variant_id)
            .unwrap_or_else(|| panic!("{name}: cap missing"));
        assert_eq!(flagged["unavailable_reason"], "inactive", "{name}");
    }
}

#[tokio::test]
async fn can_checkout_agrees_with_what_the_checkout_does() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    let cap = app.setup_product(&admin, "Cap", 10, 5).await;
    app.add_to_cart(&customer, &mug, 1).await;
    app.add_to_cart(&customer, &cap, 1).await;
    set_variant(&app, &admin, &cap, 5, false).await;

    assert_eq!(cart_of(&app, &customer).await["can_checkout"], false);
    let (refused, _) = app.send("POST", "/orders", Some(&customer), None).await;
    assert_eq!(refused, StatusCode::CONFLICT);

    app.send(
        "DELETE",
        &format!("/cart/items/{}", cap.variant_id),
        Some(&customer),
        None,
    )
    .await;

    assert_eq!(cart_of(&app, &customer).await["can_checkout"], true);
    let (accepted, _) = app.send("POST", "/orders", Some(&customer), None).await;
    assert_eq!(accepted, StatusCode::CREATED);
}
