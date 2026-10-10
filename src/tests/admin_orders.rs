use axum::http::StatusCode;
use serde_json::{Value, json};

use super::orders::{count, place_order};
use super::{Item, TestApp, send};

async fn set_status(
    app: &TestApp,
    admin: &str,
    order_id: &str,
    status: &str,
) -> (StatusCode, Value) {
    app.send(
        "PUT",
        &format!("/admin/orders/{order_id}/status"),
        Some(admin),
        Some(json!({ "status": status })),
    )
    .await
}

/// Cria um pedido do cliente e o leva, pela API de admin, até o status pedido.
async fn order_in_state(
    app: &TestApp,
    admin: &str,
    customer: &str,
    item: &Item,
    state: &str,
) -> String {
    let order_id = place_order(app, customer, item, 1).await;
    let steps: &[&str] = match state {
        "pending" => &[],
        "paid" => &["paid"],
        "shipped" => &["paid", "shipped"],
        "delivered" => &["paid", "shipped", "delivered"],
        "cancelled" => &["cancelled"],
        other => panic!("unknown state {other}"),
    };
    for step in steps {
        let (status, _) = set_status(app, admin, &order_id, step).await;
        assert_eq!(status, StatusCode::OK, "moving to {step}");
    }
    order_id
}

#[tokio::test]
async fn admin_order_routes_require_an_admin() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    let order_id = place_order(&app, &customer, &mug, 1).await;
    let status_uri = format!("/admin/orders/{order_id}/status");
    let body = json!({ "status": "paid" });

    for uri in [
        "/admin/orders".to_string(),
        format!("/admin/orders/{order_id}"),
    ] {
        assert_eq!(
            app.send("GET", &uri, None, None).await.0,
            StatusCode::UNAUTHORIZED,
            "{uri}"
        );
        assert_eq!(
            app.send("GET", &uri, Some(&customer), None).await.0,
            StatusCode::FORBIDDEN,
            "{uri}"
        );
    }
    assert_eq!(
        app.send("PUT", &status_uri, None, Some(body.clone()))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        app.send("PUT", &status_uri, Some(&customer), Some(body))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn admin_lists_the_orders_of_every_customer_newest_first() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let ana = app.customer("ana").await;
    let bob = app.customer("bob").await;
    let mug = app.setup_product(&admin, "Mug", 10, 9).await;
    let first = place_order(&app, &ana, &mug, 1).await;
    let second = place_order(&app, &bob, &mug, 2).await;

    let (status, list) = app.send("GET", "/admin/orders", Some(&admin), None).await;

    assert_eq!(status, StatusCode::OK);
    let list = list.as_array().unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0]["id"], second);
    assert_eq!(list[1]["id"], first);
    assert_eq!(list[0]["items"][0]["quantity"], 2);
    assert_eq!(list[0]["total"], 20);

    // O dono aparece só como id, sem dados pessoais.
    let (_, ana_me) = app.send("GET", "/me", Some(&ana), None).await;
    let (_, bob_me) = app.send("GET", "/me", Some(&bob), None).await;
    assert_eq!(list[1]["customer_id"], ana_me["id"]);
    assert_eq!(list[0]["customer_id"], bob_me["id"]);
    assert!(list[0].get("username").is_none());
}

#[tokio::test]
async fn list_filters_by_status_and_paginates() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 20).await;
    let oldest = order_in_state(&app, &admin, &customer, &mug, "paid").await;
    let middle = order_in_state(&app, &admin, &customer, &mug, "pending").await;
    let newest = order_in_state(&app, &admin, &customer, &mug, "pending").await;

    let ids = |list: &Value| -> Vec<String> {
        list.as_array()
            .unwrap()
            .iter()
            .map(|o| o["id"].as_str().unwrap().to_string())
            .collect()
    };

    let (_, paid) = app
        .send("GET", "/admin/orders?status=paid", Some(&admin), None)
        .await;
    assert_eq!(ids(&paid), vec![oldest.clone()]);
    let (_, pending) = app
        .send("GET", "/admin/orders?status=pending", Some(&admin), None)
        .await;
    assert_eq!(ids(&pending), vec![newest.clone(), middle.clone()]);
    let (_, none) = app
        .send("GET", "/admin/orders?status=shipped", Some(&admin), None)
        .await;
    assert!(ids(&none).is_empty());

    let (_, page1) = app
        .send("GET", "/admin/orders?limit=1", Some(&admin), None)
        .await;
    let (_, page2) = app
        .send("GET", "/admin/orders?limit=1&offset=1", Some(&admin), None)
        .await;
    let (_, page3) = app
        .send("GET", "/admin/orders?limit=1&offset=2", Some(&admin), None)
        .await;
    let (_, past_the_end) = app
        .send("GET", "/admin/orders?offset=3", Some(&admin), None)
        .await;
    assert_eq!(ids(&page1), vec![newest]);
    assert_eq!(ids(&page2), vec![middle]);
    assert_eq!(ids(&page3), vec![oldest]);
    assert!(ids(&past_the_end).is_empty());
}

#[tokio::test]
async fn invalid_list_parameters_are_422() {
    let app = TestApp::new().await;
    let admin = app.admin().await;

    for query in [
        "limit=0",
        "limit=201",
        "limit=abc",
        "offset=-1",
        "status=refunded",
    ] {
        let (status, body) = app
            .send("GET", &format!("/admin/orders?{query}"), Some(&admin), None)
            .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{query}");
        assert!(body["error"].is_string(), "{query}");
    }

    let (_, body) = app
        .send("GET", "/admin/orders?limit=0", Some(&admin), None)
        .await;
    assert_eq!(body["error"], "limit: must be between 1 and 200");
}

#[tokio::test]
async fn admin_can_open_any_order_and_unknown_ids_are_404() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    let order_id = place_order(&app, &customer, &mug, 2).await;

    let (status, order) = app
        .send(
            "GET",
            &format!("/admin/orders/{order_id}"),
            Some(&admin),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(order["id"], order_id);
    assert_eq!(order["status"], "pending");
    assert!(order["customer_id"].is_string());
    assert_eq!(order["items"][0]["quantity"], 2);

    assert_eq!(
        app.send("GET", "/admin/orders/nope", Some(&admin), None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        set_status(&app, &admin, "nope", "paid").await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn an_order_goes_through_the_whole_lifecycle() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    let order_id = place_order(&app, &customer, &mug, 2).await;

    for expected in ["paid", "shipped", "delivered"] {
        let (status, order) = set_status(&app, &admin, &order_id, expected).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(order["status"], expected);
        assert_eq!(order["total"], 20);
    }

    let (_, seen_by_customer) = app
        .send("GET", &format!("/orders/{order_id}"), Some(&customer), None)
        .await;
    assert_eq!(seen_by_customer["status"], "delivered");
    // O estoque foi baixado uma vez, no checkout.
    assert_eq!(app.stock(&mug).await, 3);
}

#[tokio::test]
async fn admin_cancelling_a_pending_order_returns_the_stock() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    let order_id = place_order(&app, &customer, &mug, 3).await;
    assert_eq!(app.stock(&mug).await, 2);

    let (status, order) = set_status(&app, &admin, &order_id, "cancelled").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(order["status"], "cancelled");
    assert_eq!(app.stock(&mug).await, 5);
    // Cancelar de novo não devolve o estoque duas vezes.
    assert_eq!(
        set_status(&app, &admin, &order_id, "cancelled").await.0,
        StatusCode::CONFLICT
    );
    assert_eq!(app.stock(&mug).await, 5);
}

#[tokio::test]
async fn transitions_outside_the_lifecycle_are_409() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 50).await;

    let cases = [
        ("pending", "shipped"),
        ("pending", "delivered"),
        ("pending", "pending"),
        ("paid", "cancelled"),
        ("paid", "delivered"),
        ("paid", "pending"),
        ("paid", "paid"),
        ("shipped", "paid"),
        ("shipped", "cancelled"),
        ("delivered", "shipped"),
        ("delivered", "cancelled"),
        ("cancelled", "paid"),
        ("cancelled", "pending"),
    ];
    for (from, to) in cases {
        let order_id = order_in_state(&app, &admin, &customer, &mug, from).await;
        let stock_before = app.stock(&mug).await;

        let (status, body) = set_status(&app, &admin, &order_id, to).await;

        assert_eq!(status, StatusCode::CONFLICT, "{from} -> {to}");
        assert_eq!(
            body["error"],
            format!("Cannot change an order from {from} to {to}"),
            "{from} -> {to}"
        );
        assert_eq!(
            app.stock(&mug).await,
            stock_before,
            "{from} -> {to}: stock must not move"
        );
        let (_, order) = app
            .send(
                "GET",
                &format!("/admin/orders/{order_id}"),
                Some(&admin),
                None,
            )
            .await;
        assert_eq!(
            order["status"], from,
            "{from} -> {to}: status must not change"
        );
    }
}

#[tokio::test]
async fn unknown_status_or_missing_field_is_422() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    let order_id = place_order(&app, &customer, &mug, 1).await;
    let uri = format!("/admin/orders/{order_id}/status");

    for body in [
        json!({ "status": "refunded" }),
        json!({ "status": "PAID" }),
        json!({}),
    ] {
        let (status, response) = app
            .send("PUT", &uri, Some(&admin), Some(body.clone()))
            .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
        assert_eq!(response["error"], "Invalid request body", "{body}");
    }
}

#[tokio::test]
async fn customers_cannot_pay_or_cancel_an_order_that_already_shipped() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    let order_id = order_in_state(&app, &admin, &customer, &mug, "shipped").await;

    let pay = app
        .send(
            "POST",
            &format!("/orders/{order_id}/pay"),
            Some(&customer),
            None,
        )
        .await;
    let cancel = app
        .send(
            "POST",
            &format!("/orders/{order_id}/cancel"),
            Some(&customer),
            None,
        )
        .await;

    assert_eq!(pay.0, StatusCode::CONFLICT);
    assert_eq!(cancel.0, StatusCode::CONFLICT);
    assert_eq!(app.stock(&mug).await, 4);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_status_changes_have_a_single_winner() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    let order_id = order_in_state(&app, &admin, &customer, &mug, "paid").await;

    let tasks: Vec<_> = (0..5)
        .map(|_| {
            let router = app.router.clone();
            let admin = admin.clone();
            let uri = format!("/admin/orders/{order_id}/status");
            tokio::spawn(async move {
                send(
                    &router,
                    "PUT",
                    &uri,
                    Some(&admin),
                    Some(json!({ "status": "shipped" })),
                )
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
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn admin_cancelling_and_customer_paying_at_the_same_time_leave_one_outcome() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    let order_id = place_order(&app, &customer, &mug, 2).await;

    let mut tasks = Vec::new();
    for n in 0..4 {
        let router = app.router.clone();
        let admin = admin.clone();
        let customer = customer.clone();
        let order_id = order_id.clone();
        tasks.push(tokio::spawn(async move {
            if n % 2 == 0 {
                let uri = format!("/admin/orders/{order_id}/status");
                send(
                    &router,
                    "PUT",
                    &uri,
                    Some(&admin),
                    Some(json!({ "status": "cancelled" })),
                )
                .await
                .0
            } else {
                let uri = format!("/orders/{order_id}/pay");
                send(&router, "POST", &uri, Some(&customer), None).await.0
            }
        }));
    }
    let mut statuses = Vec::new();
    for task in tasks {
        statuses.push(task.await.unwrap());
    }

    assert_eq!(count(&statuses, StatusCode::OK), 1, "{statuses:?}");
    assert_eq!(count(&statuses, StatusCode::CONFLICT), 3, "{statuses:?}");
    let (_, order) = app
        .send(
            "GET",
            &format!("/admin/orders/{order_id}"),
            Some(&admin),
            None,
        )
        .await;
    match order["status"].as_str().unwrap() {
        "paid" => assert_eq!(app.stock(&mug).await, 3, "{statuses:?}"),
        "cancelled" => assert_eq!(app.stock(&mug).await, 5, "{statuses:?}"),
        other => panic!("unexpected status {other}"),
    }
}

// A transação já impede dois vencedores, mas a troca de status também se protege sozinha:
// só muda se o pedido ainda estiver no status de origem. Este teste fixa essa garantia.
#[tokio::test]
async fn change_status_only_applies_when_the_order_is_still_in_the_origin_status() {
    use surrealdb::types::RecordId;

    use crate::executor::Executor;
    use crate::models::order::StatusChange;
    use crate::repositories::order as order_repo;

    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    let order_id = place_order(&app, &customer, &mug, 1).await;
    let ex = Executor::Db(&app.db);
    let change = |from: &str, to: &str| StatusChange {
        id: RecordId::new("order", order_id.clone()),
        from: from.to_string(),
        to: to.to_string(),
    };

    let wrong_origin = order_repo::change_status(&ex, change("paid", "shipped"))
        .await
        .unwrap();
    assert!(wrong_origin.is_none());
    let still_pending = order_repo::find_by_id(&ex, &order_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(still_pending.status, "pending");

    let right_origin = order_repo::change_status(&ex, change("pending", "paid"))
        .await
        .unwrap();
    assert_eq!(right_origin.unwrap().status, "paid");
}
