use axum::http::StatusCode;
use serde_json::{Value, json};

use super::orders::place_order;
use super::{TestApp, send_full};

/// Lista de clientes do admin com a query dada: `(X-Total-Count, clientes)`.
async fn list(app: &TestApp, admin: &str, query: &str) -> (Option<String>, Vec<Value>) {
    let (status, headers, body) = send_full(
        &app.router,
        "GET",
        &format!("/admin/customers{query}"),
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

fn names(customers: &[Value]) -> Vec<&str> {
    customers
        .iter()
        .map(|c| c["username"].as_str().unwrap())
        .collect()
}

async fn id_of(app: &TestApp, token: &str) -> String {
    let (_, me) = app.send("GET", "/auth/session", Some(token), None).await;
    me["id"].as_str().unwrap().to_string()
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
    assert_eq!(code, StatusCode::OK, "{body}");
}

/// Cria os clientes nesta ordem (a conta mais antiga primeiro).
async fn signup_all(app: &TestApp, usernames: &[&str]) -> Vec<String> {
    let mut tokens = Vec::new();
    for username in usernames {
        tokens.push(app.customer(username).await);
    }
    tokens
}

#[tokio::test]
async fn the_customer_list_requires_an_admin() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let ana = app.customer("ana").await;

    let (status, _) = app.send("GET", "/admin/customers", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = app
        .send("GET", "/admin/customers", Some("nope"), None)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = app.send("GET", "/admin/customers", Some(&ana), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = app
        .send("GET", "/admin/customers", Some(&admin), None)
        .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn an_empty_store_lists_nobody_with_a_zero_total() {
    let app = TestApp::new().await;
    let admin = app.admin().await;

    let (total, customers) = list(&app, &admin, "").await;

    assert!(customers.is_empty());
    assert_eq!(total.as_deref(), Some("0"));
}

#[tokio::test]
async fn only_customers_are_listed_never_the_admin() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    signup_all(&app, &["ana"]).await;

    let (total, customers) = list(&app, &admin, "").await;

    assert_eq!(names(&customers), vec!["ana"]);
    assert_eq!(total.as_deref(), Some("1"));
    // O username do admin também não aparece por busca.
    let (total, found) = list(&app, &admin, "?q=admin").await;
    assert!(found.is_empty());
    assert_eq!(total.as_deref(), Some("0"));
}

#[tokio::test]
async fn each_item_has_only_the_documented_fields_and_no_personal_data() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    signup_all(&app, &["ana"]).await;

    let (_, customers) = list(&app, &admin, "").await;

    let mut keys: Vec<&str> = customers[0]
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
    let text = customers[0].to_string();
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
async fn the_default_order_is_newest_account_first() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    signup_all(&app, &["ana", "bob", "cara"]).await;

    let (_, customers) = list(&app, &admin, "").await;

    assert_eq!(names(&customers), vec!["cara", "bob", "ana"]);
    let (_, same) = list(&app, &admin, "?sort=newest").await;
    assert_eq!(names(&same), vec!["cara", "bob", "ana"]);
}

#[tokio::test]
async fn sort_by_username_is_alphabetical_whatever_the_signup_order() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    signup_all(&app, &["cara", "ana", "bob"]).await;

    let (_, customers) = list(&app, &admin, "?sort=username").await;

    assert_eq!(names(&customers), vec!["ana", "bob", "cara"]);
}

#[tokio::test]
async fn search_finds_by_username_prefix_ignoring_case_and_spaces() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    signup_all(&app, &["ana", "anna", "bob", "joana"]).await;

    for q in ["an", "AN", "%20an%20", "%23an"] {
        let (total, found) = list(&app, &admin, &format!("?q={q}&sort=username")).await;
        assert_eq!(names(&found), vec!["ana", "anna"], "q={q}");
        assert_eq!(total.as_deref(), Some("2"), "q={q}");
    }
    // Prefixo, não "contém": "ana" também está dentro de "joana".
    let (_, found) = list(&app, &admin, "?q=ana&sort=username").await;
    assert_eq!(names(&found), vec!["ana"]);
}

#[tokio::test]
async fn search_finds_by_id_prefix() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let tokens = signup_all(&app, &["ana", "bob"]).await;
    let bob_id = id_of(&app, &tokens[1]).await;

    for q in [bob_id.clone(), bob_id[..6].to_uppercase()] {
        let (total, found) = list(&app, &admin, &format!("?q={q}")).await;
        assert_eq!(names(&found), vec!["bob"], "q={q}");
        assert_eq!(found[0]["id"], bob_id);
        assert_eq!(total.as_deref(), Some("1"), "q={q}");
    }
}

#[tokio::test]
async fn a_blank_search_lists_everyone_and_no_match_lists_nobody() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    signup_all(&app, &["ana", "bob"]).await;

    for q in ["", "%20", "%23"] {
        let (total, found) = list(&app, &admin, &format!("?q={q}")).await;
        assert_eq!(found.len(), 2, "q={q:?}");
        assert_eq!(total.as_deref(), Some("2"), "q={q:?}");
    }
    let (total, found) = list(&app, &admin, "?q=zzzz").await;
    assert!(found.is_empty());
    assert_eq!(total.as_deref(), Some("0"));
}

#[tokio::test]
async fn wildcards_quotes_and_query_syntax_in_the_term_are_just_text() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    signup_all(&app, &["ana"]).await;
    for q in [
        "%25",
        "*",
        ".",
        "a'b",
        "%22q%22",
        "%3B%20DELETE%20user",
        "%24page",
        "%5C",
    ] {
        let (total, found) = list(&app, &admin, &format!("?q={q}")).await;
        assert!(found.is_empty(), "q={q}");
        assert_eq!(total.as_deref(), Some("0"), "q={q}");
    }
    let (total, _) = list(&app, &admin, "").await;
    assert_eq!(total.as_deref(), Some("1"));
}

#[tokio::test]
async fn pages_follow_the_order_and_the_total_counts_everyone() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    signup_all(
        &app,
        &["e", "d", "c", "b", "a"]
            .map(|n| format!("user.{n}"))
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
    )
    .await;

    let (total, first) = list(&app, &admin, "?sort=username&limit=2").await;
    assert_eq!(names(&first), vec!["user.a", "user.b"]);
    assert_eq!(total.as_deref(), Some("5"));
    let (_, second) = list(&app, &admin, "?sort=username&limit=2&offset=2").await;
    assert_eq!(names(&second), vec!["user.c", "user.d"]);
    let (_, last) = list(&app, &admin, "?sort=username&limit=2&offset=4").await;
    assert_eq!(names(&last), vec!["user.e"]);
    let (total, beyond) = list(&app, &admin, "?sort=username&limit=2&offset=10").await;
    assert!(beyond.is_empty());
    assert_eq!(total.as_deref(), Some("5"));
}

#[tokio::test]
async fn a_search_page_total_counts_the_whole_search() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    signup_all(&app, &["ana1", "ana2", "ana3", "bob"]).await;

    let (total, page) = list(&app, &admin, "?q=ana&sort=username&limit=2").await;

    assert_eq!(names(&page), vec!["ana1", "ana2"]);
    assert_eq!(total.as_deref(), Some("3"));
}

#[tokio::test]
async fn orders_spent_and_last_order_are_each_customers_own() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let tokens = signup_all(&app, &["ana", "bob", "cara"]).await;
    let (ana, bob) = (&tokens[0], &tokens[1]);
    let mug = app.setup_product(&admin, "Mug", 10, 100).await;

    // Ana: pago 20, enviado 10, pendente 10 -> 3 pedidos, gasto 30.
    let paid = place_order(&app, ana, &mug, 2).await;
    let shipped = place_order(&app, ana, &mug, 1).await;
    let _pending = place_order(&app, ana, &mug, 1).await;
    set_status(&app, &admin, &paid, "paid").await;
    for step in ["paid", "shipped"] {
        set_status(&app, &admin, &shipped, step).await;
    }
    // Bob: um pedido pago de 30 e um cancelado.
    let big = place_order(&app, bob, &mug, 3).await;
    let cancelled = place_order(&app, bob, &mug, 1).await;
    set_status(&app, &admin, &big, "paid").await;
    set_status(&app, &admin, &cancelled, "cancelled").await;
    // Cara nunca comprou.

    let (_, customers) = list(&app, &admin, "?sort=username").await;

    let by = |name: &str| {
        customers
            .iter()
            .find(|c| c["username"] == name)
            .unwrap()
            .clone()
    };
    assert_eq!(
        (by("ana")["orders"].clone(), by("ana")["spent"].clone()),
        (json!(3), json!(30))
    );
    assert_eq!(
        (by("bob")["orders"].clone(), by("bob")["spent"].clone()),
        (json!(2), json!(30))
    );
    assert_eq!(
        (by("cara")["orders"].clone(), by("cara")["spent"].clone()),
        (json!(0), json!(0))
    );
    assert_eq!(by("cara")["last_order_at"], Value::Null);

    // O último pedido é o mais novo daquele cliente.
    let ana_id = by("ana")["id"].as_str().unwrap().to_string();
    let (_, orders) = app
        .send(
            "GET",
            &format!("/admin/orders?q={ana_id}"),
            Some(&admin),
            None,
        )
        .await;
    assert_eq!(by("ana")["last_order_at"], orders[0]["created_at"]);
}

#[tokio::test]
async fn a_deleted_account_leaves_the_list() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let tokens = signup_all(&app, &["ana", "bob"]).await;

    let (status, _) = app.send("DELETE", "/me", Some(&tokens[0]), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (total, customers) = list(&app, &admin, "").await;
    assert_eq!(names(&customers), vec!["bob"]);
    assert_eq!(total.as_deref(), Some("1"));
}

#[tokio::test]
async fn invalid_parameters_are_422() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    for query in [
        "?sort=bogus".to_string(),
        "?sort=spent".to_string(),
        "?limit=0".to_string(),
        "?limit=201".to_string(),
        "?offset=-1".to_string(),
        format!("?q={}", "a".repeat(65)),
    ] {
        let (status, body) = app
            .send(
                "GET",
                &format!("/admin/customers{query}"),
                Some(&admin),
                None,
            )
            .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{query}: {body}");
    }
    // O limite do termo e os limites de `limit` valem.
    for query in [
        format!("?q={}", "a".repeat(64)),
        "?limit=1".to_string(),
        "?limit=200".to_string(),
    ] {
        let (status, _) = app
            .send(
                "GET",
                &format!("/admin/customers{query}"),
                Some(&admin),
                None,
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{query}");
    }
}

#[tokio::test]
async fn search_works_the_same_in_either_order() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let tokens = signup_all(&app, &["ana", "anna", "bob"]).await;
    let ana_id = id_of(&app, &tokens[0]).await;

    for sort in ["newest", "username"] {
        let (total, found) = list(&app, &admin, &format!("?q=an&sort={sort}")).await;
        let mut got = names(&found);
        got.sort();
        assert_eq!(got, vec!["ana", "anna"], "sort={sort}");
        assert_eq!(total.as_deref(), Some("2"), "sort={sort}");

        let (_, by_id) = list(&app, &admin, &format!("?q={ana_id}&sort={sort}")).await;
        assert_eq!(names(&by_id), vec!["ana"], "sort={sort}");
    }
    // Sem `sort`, a busca também vale (a ordem padrão é a das contas mais novas).
    let (_, found) = list(&app, &admin, "?q=an").await;
    assert_eq!(names(&found), vec!["anna", "ana"]);
}
