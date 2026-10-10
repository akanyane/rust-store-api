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
