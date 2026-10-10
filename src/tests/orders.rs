use axum::http::StatusCode;
use serde_json::json;

use super::{TestApp, send};

#[tokio::test]
async fn checkout_creates_the_order_decrements_stock_and_empties_the_cart() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    let cap = app.setup_product(&admin, "Cap", 25, 4).await;
    app.add_to_cart(&customer, &mug, 2).await;
    app.add_to_cart(&customer, &cap, 1).await;

    let (status, order) = app.send("POST", "/orders", Some(&customer), None).await;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(order["status"], "pending");
    assert_eq!(order["total"], 2 * 10 + 25);
    assert_eq!(order["items"].as_array().unwrap().len(), 2);
    assert_eq!(app.stock(&mug).await, 3);
    assert_eq!(app.stock(&cap).await, 3);

    let (_, cart) = app.send("GET", "/cart", Some(&customer), None).await;
    assert!(cart["items"].as_array().unwrap().is_empty());

    let (_, orders) = app.send("GET", "/orders", Some(&customer), None).await;
    assert_eq!(orders.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn order_keeps_the_price_and_name_from_the_time_of_purchase() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    app.add_to_cart(&customer, &mug, 1).await;
    let (_, order) = app.send("POST", "/orders", Some(&customer), None).await;
    let order_id = order["id"].as_str().unwrap();

    app.send(
        "PUT",
        &format!("/products/{}/variants/{}", mug.product_id, mug.variant_id),
        Some(&admin),
        Some(
            json!({ "name": "Renamed", "sku": "NEW-SKU", "price": 99, "stock": 5, "active": true }),
        ),
    )
    .await;

    let (_, fetched) = app
        .send("GET", &format!("/orders/{order_id}"), Some(&customer), None)
        .await;
    assert_eq!(fetched["total"], 10);
    assert_eq!(fetched["items"][0]["unit_price"], 10);
    assert_eq!(fetched["items"][0]["name"], "Default");
}

#[tokio::test]
async fn checkout_requires_authentication_and_a_non_empty_cart() {
    let app = TestApp::new().await;
    let customer = app.customer("ana").await;

    assert_eq!(
        app.send("POST", "/orders", None, None).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        app.send("POST", "/orders", Some(&customer), None).await.0,
        StatusCode::CONFLICT
    );
}

#[tokio::test]
async fn checkout_is_all_or_nothing_when_an_item_runs_out_of_stock() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;

    // Vários itens saudáveis e um que perde o estoque depois de entrar no carrinho.
    // A ordem de leitura do carrinho não é fixa: quando o item sem estoque vem por
    // último, os anteriores já tiveram o estoque baixado e o rollback é o que o devolve.
    let mut healthy = Vec::new();
    for n in 0..4 {
        let item = app
            .setup_product(&admin, &format!("Healthy {n}"), 10, 5)
            .await;
        app.add_to_cart(&customer, &item, 2).await;
        healthy.push(item);
    }
    let doomed = app.setup_product(&admin, "Doomed", 10, 5).await;
    app.add_to_cart(&customer, &doomed, 2).await;
    app.send(
        "PUT",
        &format!(
            "/products/{}/variants/{}",
            doomed.product_id, doomed.variant_id
        ),
        Some(&admin),
        Some(
            json!({ "name": "Default", "sku": "DOOMED", "price": 10, "stock": 1, "active": true }),
        ),
    )
    .await;

    let (status, _) = app.send("POST", "/orders", Some(&customer), None).await;

    assert_eq!(status, StatusCode::CONFLICT);
    for item in &healthy {
        assert_eq!(app.stock(item).await, 5, "stock must be untouched");
    }
    assert_eq!(app.stock(&doomed).await, 1);
    let (_, orders) = app.send("GET", "/orders", Some(&customer), None).await;
    assert!(orders.as_array().unwrap().is_empty());
    let (_, cart) = app.send("GET", "/cart", Some(&customer), None).await;
    assert_eq!(cart["items"].as_array().unwrap().len(), 5);
}

#[tokio::test]
async fn checkout_fails_while_the_cart_holds_a_deactivated_item() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    let cap = app.setup_product(&admin, "Cap", 10, 5).await;
    app.add_to_cart(&customer, &mug, 1).await;
    app.add_to_cart(&customer, &cap, 1).await;
    app.send(
        "DELETE",
        &format!("/products/{}", mug.product_id),
        Some(&admin),
        None,
    )
    .await;

    let (status, _) = app.send("POST", "/orders", Some(&customer), None).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(app.stock(&cap).await, 5);

    // O cliente sai da situação removendo o item indisponível.
    app.send(
        "DELETE",
        &format!("/cart/items/{}", mug.variant_id),
        Some(&customer),
        None,
    )
    .await;
    let (status, _) = app.send("POST", "/orders", Some(&customer), None).await;
    assert_eq!(status, StatusCode::CREATED);
}

#[tokio::test]
async fn orders_of_other_customers_look_nonexistent() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let ana = app.customer("ana").await;
    let bob = app.customer("bob").await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    app.add_to_cart(&ana, &mug, 1).await;
    let (_, order) = app.send("POST", "/orders", Some(&ana), None).await;
    let uri = format!("/orders/{}", order["id"].as_str().unwrap());

    assert_eq!(
        app.send("GET", &uri, Some(&bob), None).await.0,
        StatusCode::NOT_FOUND
    );
    let (_, bob_orders) = app.send("GET", "/orders", Some(&bob), None).await;
    assert!(bob_orders.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn cancel_returns_the_stock_and_cannot_be_repeated() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    app.add_to_cart(&customer, &mug, 3).await;
    let (_, order) = app.send("POST", "/orders", Some(&customer), None).await;
    let order_id = order["id"].as_str().unwrap();
    assert_eq!(app.stock(&mug).await, 2);
    let cancel = format!("/orders/{order_id}/cancel");

    let (status, cancelled) = app.send("POST", &cancel, Some(&customer), None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(cancelled["status"], "cancelled");
    assert_eq!(app.stock(&mug).await, 5);

    // Cancelar de novo não devolve o estoque duas vezes.
    let (again, _) = app.send("POST", &cancel, Some(&customer), None).await;
    assert_eq!(again, StatusCode::CONFLICT);
    assert_eq!(app.stock(&mug).await, 5);

    let (_, orders) = app.send("GET", "/orders", Some(&customer), None).await;
    assert_eq!(orders[0]["status"], "cancelled");
}

#[tokio::test]
async fn cancel_restores_stock_even_after_the_product_was_deactivated() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    app.add_to_cart(&customer, &mug, 2).await;
    let (_, order) = app.send("POST", "/orders", Some(&customer), None).await;
    app.send(
        "DELETE",
        &format!("/products/{}", mug.product_id),
        Some(&admin),
        None,
    )
    .await;

    let (status, _) = app
        .send(
            "POST",
            &format!("/orders/{}/cancel", order["id"].as_str().unwrap()),
            Some(&customer),
            None,
        )
        .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(app.stock(&mug).await, 5);
}

#[tokio::test]
async fn cancel_requires_ownership_and_authentication() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let ana = app.customer("ana").await;
    let bob = app.customer("bob").await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    app.add_to_cart(&ana, &mug, 1).await;
    let (_, order) = app.send("POST", "/orders", Some(&ana), None).await;
    let cancel = format!("/orders/{}/cancel", order["id"].as_str().unwrap());

    assert_eq!(
        app.send("POST", &cancel, None, None).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        app.send("POST", &cancel, Some(&bob), None).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.send("POST", "/orders/nope/cancel", Some(&ana), None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    // As tentativas inválidas não mexeram no pedido nem no estoque.
    assert_eq!(app.stock(&mug).await, 4);
    let (_, orders) = app.send("GET", "/orders", Some(&ana), None).await;
    assert_eq!(orders[0]["status"], "pending");
}

/// `buyers` clientes, cada um com 1 unidade do item no carrinho, finalizam a compra ao
/// mesmo tempo. Devolve o status de cada checkout.
async fn race_to_buy(app: &TestApp, item: &super::Item, buyers: usize) -> Vec<StatusCode> {
    let mut tokens = Vec::new();
    for n in 0..buyers {
        let token = app.customer(&format!("buyer{n}")).await;
        assert_eq!(app.add_to_cart(&token, item, 1).await, StatusCode::OK);
        tokens.push(token);
    }

    let tasks: Vec<_> = tokens
        .into_iter()
        .map(|token| {
            let router = app.router.clone();
            tokio::spawn(
                async move { send(&router, "POST", "/orders", Some(&token), None).await.0 },
            )
        })
        .collect();
    let mut statuses = Vec::new();
    for task in tasks {
        statuses.push(task.await.unwrap());
    }
    statuses
}

fn count(statuses: &[StatusCode], wanted: StatusCode) -> usize {
    statuses.iter().filter(|s| **s == wanted).count()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_checkouts_for_the_last_unit_sell_it_once_and_refuse_the_rest_with_409() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let item = app.setup_product(&admin, "Last one", 10, 1).await;

    let statuses = race_to_buy(&app, &item, 5).await;

    assert_eq!(count(&statuses, StatusCode::CREATED), 1, "{statuses:?}");
    assert_eq!(count(&statuses, StatusCode::CONFLICT), 4, "{statuses:?}");
    assert_eq!(app.stock(&item).await, 0, "{statuses:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_checkouts_with_some_stock_left_let_exactly_that_many_through() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let item = app.setup_product(&admin, "Three left", 10, 3).await;

    let statuses = race_to_buy(&app, &item, 5).await;

    assert_eq!(count(&statuses, StatusCode::CREATED), 3, "{statuses:?}");
    assert_eq!(count(&statuses, StatusCode::CONFLICT), 2, "{statuses:?}");
    assert_eq!(app.stock(&item).await, 0, "{statuses:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_cancels_of_the_same_order_return_the_stock_only_once() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let item = app.setup_product(&admin, "Mug", 10, 5).await;
    app.add_to_cart(&customer, &item, 2).await;
    let (_, order) = app.send("POST", "/orders", Some(&customer), None).await;
    let cancel = format!("/orders/{}/cancel", order["id"].as_str().unwrap());
    assert_eq!(app.stock(&item).await, 3);

    let tasks: Vec<_> = (0..5)
        .map(|_| {
            let router = app.router.clone();
            let customer = customer.clone();
            let cancel = cancel.clone();
            tokio::spawn(async move {
                send(&router, "POST", &cancel, Some(&customer), None)
                    .await
                    .0
            })
        })
        .collect();
    let mut statuses = Vec::new();
    for task in tasks {
        statuses.push(task.await.unwrap());
    }

    assert_eq!(count(&statuses, StatusCode::OK), 1, "{statuses:?}");
    assert_eq!(count(&statuses, StatusCode::CONFLICT), 4, "{statuses:?}");
    assert_eq!(app.stock(&item).await, 5, "{statuses:?}");
}
