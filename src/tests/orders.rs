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

pub(super) fn count(statuses: &[StatusCode], wanted: StatusCode) -> usize {
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

/// Cliente compra `quantity` unidades do item e devolve o id do pedido pendente.
pub(super) async fn place_order(
    app: &TestApp,
    token: &str,
    item: &super::Item,
    quantity: i64,
) -> String {
    assert_eq!(app.add_to_cart(token, item, quantity).await, StatusCode::OK);
    let (status, order) = app.send("POST", "/orders", Some(token), None).await;
    assert_eq!(status, StatusCode::CREATED);
    order["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn paying_marks_the_order_paid_and_leaves_the_stock_alone() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    let order_id = place_order(&app, &customer, &mug, 2).await;

    let (status, order) = app
        .send(
            "POST",
            &format!("/orders/{order_id}/pay"),
            Some(&customer),
            None,
        )
        .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(order["status"], "paid");
    assert_eq!(order["total"], 20);
    assert_eq!(order["items"].as_array().unwrap().len(), 1);
    assert_eq!(app.stock(&mug).await, 3);

    let (_, listed) = app.send("GET", "/orders", Some(&customer), None).await;
    assert_eq!(listed[0]["status"], "paid");
}

#[tokio::test]
async fn only_pending_orders_can_be_paid() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    let paid_id = place_order(&app, &customer, &mug, 1).await;
    let cancelled_id = place_order(&app, &customer, &mug, 1).await;
    app.send(
        "POST",
        &format!("/orders/{cancelled_id}/cancel"),
        Some(&customer),
        None,
    )
    .await;
    app.send(
        "POST",
        &format!("/orders/{paid_id}/pay"),
        Some(&customer),
        None,
    )
    .await;

    let pay_again = app
        .send(
            "POST",
            &format!("/orders/{paid_id}/pay"),
            Some(&customer),
            None,
        )
        .await;
    let pay_cancelled = app
        .send(
            "POST",
            &format!("/orders/{cancelled_id}/pay"),
            Some(&customer),
            None,
        )
        .await;

    assert_eq!(pay_again.0, StatusCode::CONFLICT);
    assert_eq!(pay_cancelled.0, StatusCode::CONFLICT);
    assert_eq!(pay_cancelled.1["error"], "Only pending orders can be paid");
    // O pedido cancelado continua cancelado, com o estoque devolvido.
    let (_, order) = app
        .send(
            "GET",
            &format!("/orders/{cancelled_id}"),
            Some(&customer),
            None,
        )
        .await;
    assert_eq!(order["status"], "cancelled");
    assert_eq!(app.stock(&mug).await, 4);
}

#[tokio::test]
async fn a_paid_order_cannot_be_cancelled() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    let order_id = place_order(&app, &customer, &mug, 2).await;
    app.send(
        "POST",
        &format!("/orders/{order_id}/pay"),
        Some(&customer),
        None,
    )
    .await;

    let (status, _) = app
        .send(
            "POST",
            &format!("/orders/{order_id}/cancel"),
            Some(&customer),
            None,
        )
        .await;

    assert_eq!(status, StatusCode::CONFLICT);
    // Sem reembolso, o estoque do pedido pago não volta.
    assert_eq!(app.stock(&mug).await, 3);
    let (_, order) = app
        .send("GET", &format!("/orders/{order_id}"), Some(&customer), None)
        .await;
    assert_eq!(order["status"], "paid");
}

#[tokio::test]
async fn paying_requires_ownership_and_authentication() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let ana = app.customer("ana").await;
    let bob = app.customer("bob").await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    let order_id = place_order(&app, &ana, &mug, 1).await;
    let pay = format!("/orders/{order_id}/pay");

    assert_eq!(
        app.send("POST", &pay, None, None).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        app.send("POST", &pay, Some(&bob), None).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.send("POST", "/orders/nope/pay", Some(&ana), None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    // As tentativas inválidas não pagaram o pedido.
    let (_, order) = app
        .send("GET", &format!("/orders/{order_id}"), Some(&ana), None)
        .await;
    assert_eq!(order["status"], "pending");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_payments_of_the_same_order_succeed_only_once() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    let order_id = place_order(&app, &customer, &mug, 1).await;
    let pay = format!("/orders/{order_id}/pay");

    let tasks: Vec<_> = (0..5)
        .map(|_| {
            let router = app.router.clone();
            let customer = customer.clone();
            let pay = pay.clone();
            tokio::spawn(async move { send(&router, "POST", &pay, Some(&customer), None).await.0 })
        })
        .collect();
    let mut statuses = Vec::new();
    for task in tasks {
        statuses.push(task.await.unwrap());
    }

    assert_eq!(count(&statuses, StatusCode::OK), 1, "{statuses:?}");
    assert_eq!(count(&statuses, StatusCode::CONFLICT), 4, "{statuses:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn paying_and_cancelling_at_the_same_time_leaves_exactly_one_outcome() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    let order_id = place_order(&app, &customer, &mug, 2).await;

    let tasks: Vec<_> = ["pay", "cancel", "pay", "cancel"]
        .into_iter()
        .map(|action| {
            let router = app.router.clone();
            let customer = customer.clone();
            let uri = format!("/orders/{order_id}/{action}");
            tokio::spawn(async move { send(&router, "POST", &uri, Some(&customer), None).await.0 })
        })
        .collect();
    let mut statuses = Vec::new();
    for task in tasks {
        statuses.push(task.await.unwrap());
    }

    assert_eq!(count(&statuses, StatusCode::OK), 1, "{statuses:?}");
    assert_eq!(count(&statuses, StatusCode::CONFLICT), 3, "{statuses:?}");
    // O estoque tem que bater com o status final: pago mantém baixado, cancelado devolve.
    let (_, order) = app
        .send("GET", &format!("/orders/{order_id}"), Some(&customer), None)
        .await;
    match order["status"].as_str().unwrap() {
        "paid" => assert_eq!(app.stock(&mug).await, 3, "{statuses:?}"),
        "cancelled" => assert_eq!(app.stock(&mug).await, 5, "{statuses:?}"),
        other => panic!("unexpected status {other}"),
    }
}

pub(super) fn parse_time(value: &serde_json::Value) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339(value.as_str().expect("timestamp string"))
        .expect("rfc3339 timestamp")
        .with_timezone(&chrono::Utc)
}

#[tokio::test]
async fn customer_payment_records_when_the_order_was_paid() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    let paid_id = place_order(&app, &customer, &mug, 1).await;
    let pending_id = place_order(&app, &customer, &mug, 1).await;
    let cancelled_id = place_order(&app, &customer, &mug, 1).await;
    app.send(
        "POST",
        &format!("/orders/{cancelled_id}/cancel"),
        Some(&customer),
        None,
    )
    .await;

    let (_, pending) = app
        .send(
            "GET",
            &format!("/orders/{pending_id}"),
            Some(&customer),
            None,
        )
        .await;
    assert!(
        pending["paid_at"].is_null(),
        "unpaid order must have no paid_at"
    );

    let (status, paid) = app
        .send(
            "POST",
            &format!("/orders/{paid_id}/pay"),
            Some(&customer),
            None,
        )
        .await;
    let after = chrono::Utc::now();

    assert_eq!(status, StatusCode::OK);
    let paid_at = parse_time(&paid["paid_at"]);
    assert!(
        paid_at >= parse_time(&paid["created_at"]),
        "paid_at before created_at"
    );
    assert!(paid_at <= after, "paid_at in the future");

    // O mesmo valor aparece ao reler e na listagem; cancelado e pendente seguem sem data.
    let (_, fetched) = app
        .send("GET", &format!("/orders/{paid_id}"), Some(&customer), None)
        .await;
    assert_eq!(fetched["paid_at"], paid["paid_at"]);
    let (_, listed) = app.send("GET", "/orders", Some(&customer), None).await;
    for order in listed.as_array().unwrap() {
        let expected_paid = order["id"] == paid_id;
        assert_eq!(order["paid_at"].is_string(), expected_paid, "{order}");
    }
    let (_, cancelled) = app
        .send(
            "GET",
            &format!("/orders/{cancelled_id}"),
            Some(&customer),
            None,
        )
        .await;
    assert!(cancelled["paid_at"].is_null());
}

async fn rename(app: &TestApp, admin: &str, product_id: &str, name: &str) {
    let (status, _) = app
        .send(
            "PUT",
            &format!("/products/{product_id}"),
            Some(admin),
            Some(json!({ "name": name, "description": "d", "active": true })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn order_items_keep_the_product_name_from_the_time_of_purchase() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let tea = app.setup_product(&admin, "Oolong Tea", 45, 20).await;
    let order_id = place_order(&app, &customer, &tea, 1).await;

    rename(&app, &admin, &tea.product_id, "Oolong Tea Reserve").await;

    // O pedido continua com o nome de quando foi comprado, em todas as visões.
    let (_, order) = app
        .send("GET", &format!("/orders/{order_id}"), Some(&customer), None)
        .await;
    assert_eq!(order["items"][0]["product_name"], "Oolong Tea");
    assert_eq!(order["items"][0]["name"], "Default");
    let (_, list) = app.send("GET", "/orders", Some(&customer), None).await;
    assert_eq!(list[0]["items"][0]["product_name"], "Oolong Tea");
    let (_, admin_order) = app
        .send(
            "GET",
            &format!("/admin/orders/{order_id}"),
            Some(&admin),
            None,
        )
        .await;
    assert_eq!(admin_order["items"][0]["product_name"], "Oolong Tea");
    let (_, admin_list) = app.send("GET", "/admin/orders", Some(&admin), None).await;
    assert_eq!(admin_list[0]["items"][0]["product_name"], "Oolong Tea");

    // Uma compra feita DEPOIS da renomeação já nasce com o nome novo.
    let later_id = place_order(&app, &customer, &tea, 1).await;
    let (_, later) = app
        .send("GET", &format!("/orders/{later_id}"), Some(&customer), None)
        .await;
    assert_eq!(later["items"][0]["product_name"], "Oolong Tea Reserve");
}

#[tokio::test]
async fn the_checkout_response_and_every_order_action_carry_the_product_name() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 20).await;
    let cap = app.setup_product(&admin, "Cap", 20, 20).await;

    // A resposta do próprio checkout, com dois produtos diferentes.
    app.add_to_cart(&customer, &mug, 1).await;
    app.add_to_cart(&customer, &cap, 2).await;
    let (status, order) = app.send("POST", "/orders", Some(&customer), None).await;
    assert_eq!(status, StatusCode::CREATED);
    let mut names: Vec<&str> = order["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["product_name"].as_str().unwrap())
        .collect();
    names.sort();
    assert_eq!(names, ["Cap", "Mug"]);

    let order_id = order["id"].as_str().unwrap();
    let (_, paid) = app
        .send(
            "POST",
            &format!("/orders/{order_id}/pay"),
            Some(&customer),
            None,
        )
        .await;
    assert!(
        paid["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["product_name"].is_string())
    );

    let cancelled_id = place_order(&app, &customer, &mug, 1).await;
    let (_, cancelled) = app
        .send(
            "POST",
            &format!("/orders/{cancelled_id}/cancel"),
            Some(&customer),
            None,
        )
        .await;
    assert_eq!(cancelled["items"][0]["product_name"], "Mug");

    let (_, shipped) = app
        .send(
            "PUT",
            &format!("/admin/orders/{order_id}/status"),
            Some(&admin),
            Some(json!({ "status": "shipped" })),
        )
        .await;
    assert!(
        shipped["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["product_name"].is_string())
    );
}

#[tokio::test]
async fn order_items_created_before_the_field_existed_read_as_null() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 20).await;
    let (_, me) = app.send("GET", "/me", Some(&customer), None).await;
    let customer_id = me["id"].as_str().unwrap();

    // Um pedido "antigo": o item é gravado direto no banco, sem `product_name`.
    app.db
        .query(format!(
            "CREATE order:legacy SET customer = customer:`{customer_id}`, total = 10;
             CREATE order_item SET order = order:legacy, variant = variant:`{}`,
                 name = 'Default', sku = 'OLD-1', unit_price = 10, quantity = 1;",
            mug.variant_id
        ))
        .await
        .unwrap()
        .check()
        .unwrap();

    let (status, order) = app
        .send("GET", "/orders/legacy", Some(&customer), None)
        .await;

    assert_eq!(status, StatusCode::OK);
    assert!(order["items"][0]["product_name"].is_null(), "{order}");
    // O resto do item segue legível, e a listagem inteira continua funcionando.
    assert_eq!(order["items"][0]["name"], "Default");
    assert_eq!(order["items"][0]["sku"], "OLD-1");
    let (status, list) = app.send("GET", "/orders", Some(&customer), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list.as_array().unwrap().len(), 1);
    let (status, _) = app
        .send("GET", "/admin/orders/legacy", Some(&admin), None)
        .await;
    assert_eq!(status, StatusCode::OK);
}
