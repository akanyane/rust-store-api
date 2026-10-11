use axum::http::StatusCode;
use serde_json::Value;

use super::orders::place_order;
use super::{TestApp, send_full};

/// Lista de pedidos do admin com a query dada: `(X-Total-Count, pedidos)`.
async fn search(app: &TestApp, admin: &str, query: &str) -> (Option<String>, Vec<Value>) {
    let (status, headers, body) = send_full(
        &app.router,
        "GET",
        &format!("/admin/orders{query}"),
        Some(admin),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{query}: {body}");
    let total = headers
        .get("x-total-count")
        .map(|value| value.to_str().unwrap().to_string());
    (total, body.as_array().expect("list").clone())
}

fn ids(orders: &[Value]) -> Vec<String> {
    orders
        .iter()
        .map(|order| order["id"].as_str().unwrap().to_string())
        .collect()
}

struct Shop {
    app: TestApp,
    admin: String,
    /// Dois pedidos de Ana e um de Bob.
    ana_orders: Vec<String>,
    bob_order: String,
    ana_id: String,
    bob_id: String,
}

async fn shop() -> Shop {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let ana = app.customer("ana").await;
    let bob = app.customer("bob").await;
    let mug = app.setup_product(&admin, "Mug", 10, 50).await;
    let first = place_order(&app, &ana, &mug, 1).await;
    let second = place_order(&app, &ana, &mug, 2).await;
    let bob_order = place_order(&app, &bob, &mug, 1).await;

    let (_, all) = search(&app, &admin, "").await;
    let owner_of = |id: &str| {
        all.iter()
            .find(|order| order["id"] == id)
            .and_then(|order| order["customer_id"].as_str())
            .expect("owner")
            .to_string()
    };
    let ana_id = owner_of(&first);
    let bob_id = owner_of(&bob_order);
    assert_ne!(ana_id, bob_id);
    Shop {
        app,
        admin,
        ana_orders: vec![first, second],
        bob_order,
        ana_id,
        bob_id,
    }
}

#[tokio::test]
async fn search_finds_an_order_by_the_number_shown_on_screen() {
    let s = shop().await;
    let id = &s.bob_order;
    // O número na tela: 8 caracteres em maiúsculas, com `#`.
    let shown = format!("%23{}", id[..8].to_uppercase());

    let (total, found) = search(&s.app, &s.admin, &format!("?q={shown}")).await;

    assert_eq!(ids(&found), vec![id.clone()]);
    assert_eq!(total.as_deref(), Some("1"));
}

#[tokio::test]
async fn search_ignores_case_the_hash_and_spaces_and_accepts_the_whole_id() {
    let s = shop().await;
    let id = &s.bob_order;
    let upper = id.to_uppercase();
    for q in [
        id.clone(),
        upper.clone(),
        format!("%23{id}"),
        format!("%20%23%20{upper}%20"),
        id[..6].to_string(),
    ] {
        let (_, found) = search(&s.app, &s.admin, &format!("?q={q}")).await;
        assert_eq!(ids(&found), vec![id.clone()], "q={q}");
    }
}

#[tokio::test]
async fn search_finds_every_order_of_a_customer_by_id_or_prefix() {
    let s = shop().await;

    for q in [s.ana_id.clone(), s.ana_id[..6].to_uppercase()] {
        let (total, found) = search(&s.app, &s.admin, &format!("?q={q}")).await;
        let mut got = ids(&found);
        got.sort();
        let mut want = s.ana_orders.clone();
        want.sort();
        assert_eq!(got, want, "q={q}");
        assert_eq!(total.as_deref(), Some("2"), "q={q}");
    }
}

#[tokio::test]
async fn searching_one_customer_never_returns_another_customers_orders() {
    let s = shop().await;

    let (_, found) = search(&s.app, &s.admin, &format!("?q={}", s.bob_id)).await;

    assert_eq!(ids(&found), vec![s.bob_order.clone()]);
}

#[tokio::test]
async fn search_combines_with_the_status_filter() {
    let s = shop().await;
    let paid = &s.ana_orders[0];
    let (status, _) = s
        .app
        .send(
            "PUT",
            &format!("/admin/orders/{paid}/status"),
            Some(&s.admin),
            Some(serde_json::json!({ "status": "paid" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let (total, found) = search(&s.app, &s.admin, &format!("?q={}&status=paid", s.ana_id)).await;
    assert_eq!(ids(&found), vec![paid.clone()]);
    assert_eq!(total.as_deref(), Some("1"));

    let (total, found) = search(&s.app, &s.admin, &format!("?q={}&status=pending", s.ana_id)).await;
    assert_eq!(ids(&found), vec![s.ana_orders[1].clone()]);
    assert_eq!(total.as_deref(), Some("1"));

    // Bob não tem pedido pago: a busca dele com `status=paid` fica vazia.
    let (total, found) = search(&s.app, &s.admin, &format!("?q={}&status=paid", s.bob_id)).await;
    assert!(found.is_empty());
    assert_eq!(total.as_deref(), Some("0"));
}

#[tokio::test]
async fn the_total_counts_the_whole_search_not_just_the_page() {
    let s = shop().await;

    let (total, first) = search(&s.app, &s.admin, &format!("?q={}&limit=1", s.ana_id)).await;
    assert_eq!(first.len(), 1);
    assert_eq!(total.as_deref(), Some("2"));

    let (total, second) = search(
        &s.app,
        &s.admin,
        &format!("?q={}&limit=1&offset=1", s.ana_id),
    )
    .await;
    assert_eq!(second.len(), 1);
    assert_eq!(total.as_deref(), Some("2"));
    assert_ne!(ids(&first), ids(&second));
}

#[tokio::test]
async fn an_empty_or_blank_search_means_no_search() {
    let s = shop().await;
    for q in ["", "%20", "%20%20", "%23", "%20%23%20"] {
        let (total, found) = search(&s.app, &s.admin, &format!("?q={q}")).await;
        assert_eq!(found.len(), 3, "q={q:?}");
        assert_eq!(total.as_deref(), Some("3"), "q={q:?}");
    }
}

#[tokio::test]
async fn a_search_with_no_match_is_an_empty_list_with_a_zero_total() {
    let s = shop().await;

    let (total, found) = search(&s.app, &s.admin, "?q=zzzzzzzz").await;

    assert!(found.is_empty());
    assert_eq!(total.as_deref(), Some("0"));
}

#[tokio::test]
async fn the_search_is_a_prefix_not_a_substring() {
    let s = shop().await;
    let middle = &s.bob_order[2..8];

    let (_, found) = search(&s.app, &s.admin, &format!("?q={middle}")).await;

    assert!(
        !ids(&found).contains(&s.bob_order),
        "{middle} is in the middle of the id, not the start"
    );
}

#[tokio::test]
async fn wildcards_quotes_and_query_syntax_in_the_term_are_just_text() {
    let s = shop().await;
    for q in [
        "%25",
        "*",
        ".",
        "a'b",
        "%22quoted%22",
        "%3B%20DELETE%20order",
        "%24page",
        "%5C",
    ] {
        let (total, found) = search(&s.app, &s.admin, &format!("?q={q}")).await;
        assert!(found.is_empty(), "q={q}");
        assert_eq!(total.as_deref(), Some("0"), "q={q}");
    }
    // E nada foi apagado.
    let (total, _) = search(&s.app, &s.admin, "").await;
    assert_eq!(total.as_deref(), Some("3"));
}

#[tokio::test]
async fn a_term_over_64_characters_is_a_422_and_64_is_fine() {
    let s = shop().await;

    let (status, body) = s
        .app
        .send(
            "GET",
            &format!("/admin/orders?q={}", "a".repeat(65)),
            Some(&s.admin),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(body["error"].as_str().unwrap().starts_with("q:"), "{body}");

    let (total, _) = search(&s.app, &s.admin, &format!("?q={}", "a".repeat(64))).await;
    assert_eq!(total.as_deref(), Some("0"));
}
