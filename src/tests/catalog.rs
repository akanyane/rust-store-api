use axum::http::StatusCode;
use serde_json::json;

use super::TestApp;

#[tokio::test]
async fn admin_creates_product_with_a_default_variant() {
    let app = TestApp::new().await;
    let admin = app.admin().await;

    let item = app.setup_product(&admin, "Mug", 10, 5).await;

    let (status, product) = app
        .send("GET", &format!("/products/{}", item.product_id), None, None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(product["name"], "Mug");
    let variants = product["variants"].as_array().unwrap();
    assert_eq!(variants.len(), 1);
    assert_eq!(variants[0]["price"], 10);
    assert_eq!(variants[0]["stock"], 5);
    assert!(variants[0]["sku"].as_str().unwrap().starts_with("SKU-"));

    let (_, list) = app.send("GET", "/products", None, None).await;
    assert_eq!(list.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn invalid_product_input_is_422_with_the_field_name() {
    let app = TestApp::new().await;
    let admin = app.admin().await;

    let (status, body) = app
        .send(
            "POST",
            "/products",
            Some(&admin),
            Some(json!({ "name": " ", "description": "d", "price": -1, "stock": -1 })),
        )
        .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let message = body["error"].as_str().unwrap();
    assert!(message.contains("name: must not be blank"), "{message}");
    assert!(message.contains("price: must not be negative"), "{message}");
    assert!(message.contains("stock: must not be negative"), "{message}");
}

#[tokio::test]
async fn admin_routes_reject_missing_and_customer_tokens() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let item = app.setup_product(&admin, "Mug", 10, 5).await;
    let uri = format!("/products/{}", item.product_id);
    let body = json!({ "name": "x", "description": "x", "active": true });

    assert_eq!(
        app.send("PUT", &uri, None, Some(body.clone())).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        app.send("PUT", &uri, Some(&customer), Some(body)).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        app.send("DELETE", &uri, Some(&customer), None).await.0,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn put_product_replaces_the_fields() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let item = app.setup_product(&admin, "Mug", 10, 5).await;

    let (status, body) = app
        .send(
            "PUT",
            &format!("/products/{}", item.product_id),
            Some(&admin),
            Some(json!({ "name": "Big Mug", "description": "updated", "active": true })),
        )
        .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["name"], "Big Mug");
    assert_eq!(body["description"], "updated");

    let missing = app
        .send(
            "PUT",
            "/products/nope",
            Some(&admin),
            Some(json!({ "name": "x", "description": "x", "active": true })),
        )
        .await;
    assert_eq!(missing.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn variant_sku_must_be_unique_but_can_be_kept_on_update() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    let cap = app.setup_product(&admin, "Cap", 20, 5).await;
    let uri = format!("/products/{}/variants", mug.product_id);

    let (status, created) = app
        .send(
            "POST",
            &uri,
            Some(&admin),
            Some(json!({ "name": "Blue", "sku": "MUG-BLUE", "price": 12, "stock": 3 })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let blue_id = created["id"].as_str().unwrap();

    let duplicate = app
        .send(
            "POST",
            &uri,
            Some(&admin),
            Some(json!({ "name": "Red", "sku": "MUG-BLUE", "price": 12, "stock": 3 })),
        )
        .await;
    assert_eq!(duplicate.0, StatusCode::CONFLICT);

    let keep_own = app
        .send(
            "PUT",
            &format!("{uri}/{blue_id}"),
            Some(&admin),
            Some(json!({ "name": "Blue", "sku": "MUG-BLUE", "price": 15, "stock": 4, "active": true })),
        )
        .await;
    assert_eq!(keep_own.0, StatusCode::OK);
    assert_eq!(keep_own.1["price"], 15);
    assert_eq!(keep_own.1["stock"], 4);

    let cap_sku = format!("SKU-{}", cap.product_id);
    let take_other = app
        .send(
            "PUT",
            &format!("{uri}/{blue_id}"),
            Some(&admin),
            Some(
                json!({ "name": "Blue", "sku": cap_sku, "price": 15, "stock": 4, "active": true }),
            ),
        )
        .await;
    assert_eq!(take_other.0, StatusCode::CONFLICT);
}

#[tokio::test]
async fn variant_of_another_product_is_not_found() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    let cap = app.setup_product(&admin, "Cap", 20, 5).await;

    let uri = format!("/products/{}/variants/{}", mug.product_id, cap.variant_id);
    let body = json!({ "name": "x", "sku": "Z", "price": 1, "stock": 1, "active": true });

    assert_eq!(
        app.send("PUT", &uri, Some(&admin), Some(body)).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.send("DELETE", &uri, Some(&admin), None).await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn deleting_a_product_is_logical_idempotent_and_reversible() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let item = app.setup_product(&admin, "Mug", 10, 5).await;
    let uri = format!("/products/{}", item.product_id);

    assert_eq!(
        app.send("DELETE", &uri, Some(&admin), None).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        app.send("DELETE", &uri, Some(&admin), None).await.0,
        StatusCode::NO_CONTENT
    );

    assert_eq!(
        app.send("GET", &uri, None, None).await.0,
        StatusCode::NOT_FOUND
    );
    let (_, list) = app.send("GET", "/products", None, None).await;
    assert!(list.as_array().unwrap().is_empty());
    let variants = app
        .send("GET", &format!("{uri}/variants"), None, None)
        .await;
    assert_eq!(variants.0, StatusCode::NOT_FOUND);
    // O registro continua no banco (pedidos e carrinhos apontam para ele).
    assert_eq!(app.stock(&item).await, 5);

    let (status, _) = app
        .send(
            "PUT",
            &uri,
            Some(&admin),
            Some(json!({ "name": "Mug", "description": "d", "active": true })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(app.send("GET", &uri, None, None).await.0, StatusCode::OK);
}

#[tokio::test]
async fn deleting_a_variant_hides_it_from_the_public_listing() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let item = app.setup_product(&admin, "Mug", 10, 5).await;
    let uri = format!("/products/{}/variants", item.product_id);

    let (status, _) = app
        .send(
            "DELETE",
            &format!("{uri}/{}", item.variant_id),
            Some(&admin),
            None,
        )
        .await;

    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, variants) = app.send("GET", &uri, None, None).await;
    assert!(variants.as_array().unwrap().is_empty());
}

fn product_names(list: &serde_json::Value) -> Vec<String> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|product| product["name"].as_str().unwrap().to_string())
        .collect()
}

async fn public_names(app: &TestApp, query: &str) -> Vec<String> {
    let (status, body) = app
        .send("GET", &format!("/products{query}"), None, None)
        .await;
    assert_eq!(status, StatusCode::OK, "{query}");
    product_names(&body)
}

#[tokio::test]
async fn the_public_list_is_ordered_by_name_whatever_the_creation_order() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    for name in ["Charlie", "Alpha", "Bravo"] {
        app.setup_product(&admin, name, 10, 5).await;
    }

    let first = public_names(&app, "").await;
    let second = public_names(&app, "").await;

    assert_eq!(first, ["Alpha", "Bravo", "Charlie"]);
    assert_eq!(first, second, "the order must not change between calls");
    assert_eq!(public_names(&app, "?sort=name").await, first);
}

#[tokio::test]
async fn the_public_list_can_show_the_most_recent_first() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    // Criados nesta ordem: por data é Charlie, Alpha, Bravo; por nome é outra.
    for name in ["Bravo", "Alpha", "Charlie"] {
        app.setup_product(&admin, name, 10, 5).await;
    }

    assert_eq!(
        public_names(&app, "?sort=newest").await,
        ["Charlie", "Alpha", "Bravo"]
    );
    assert_eq!(
        public_names(&app, "").await,
        ["Alpha", "Bravo", "Charlie"],
        "the default stays by name"
    );
}

#[tokio::test]
async fn products_without_a_creation_date_come_last_in_the_newest_order() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    // Produto "antigo": gravado direto no banco, sem `created_at`.
    app.db
        .query("CREATE product SET name = 'Legacy', description = 'old', active = true")
        .await
        .unwrap()
        .check()
        .unwrap();
    app.setup_product(&admin, "Newer", 10, 5).await;

    assert_eq!(
        public_names(&app, "?sort=newest").await,
        ["Newer", "Legacy"]
    );
}

#[tokio::test]
async fn deactivated_products_stay_out_of_the_public_list_in_both_orders() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    app.setup_product(&admin, "Alpha", 10, 5).await;
    let bravo = app.setup_product(&admin, "Bravo", 10, 5).await;
    app.setup_product(&admin, "Charlie", 10, 5).await;
    app.send(
        "DELETE",
        &format!("/products/{}", bravo.product_id),
        Some(&admin),
        None,
    )
    .await;

    assert_eq!(public_names(&app, "").await, ["Alpha", "Charlie"]);
    assert_eq!(
        public_names(&app, "?sort=newest").await,
        ["Charlie", "Alpha"]
    );

    app.send(
        "PUT",
        &format!("/products/{}", bravo.product_id),
        Some(&admin),
        Some(json!({ "name": "Bravo", "description": "d", "active": true })),
    )
    .await;
    assert_eq!(public_names(&app, "").await, ["Alpha", "Bravo", "Charlie"]);
}

#[tokio::test]
async fn products_with_the_same_name_keep_a_stable_public_order() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let first = app.setup_product(&admin, "Same", 10, 5).await;
    let second = app.setup_product(&admin, "Same", 10, 5).await;
    let mut expected = vec![first.product_id, second.product_id];
    expected.sort();

    for query in ["", "?sort=name"] {
        let (_, list) = app
            .send("GET", &format!("/products{query}"), None, None)
            .await;
        let ids: Vec<String> = list
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["id"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(ids, expected, "{query}");
    }
}

#[tokio::test]
async fn an_unknown_sort_is_422_but_other_parameters_are_ignored() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    app.setup_product(&admin, "Mug", 10, 5).await;

    for query in ["?sort=oldest", "?sort=NAME", "?sort="] {
        let (status, body) = app
            .send("GET", &format!("/products{query}"), None, None)
            .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{query}");
        assert_eq!(body["error"], "Invalid query parameters", "{query}");
    }
    // Parâmetros que a rota não conhece (como os de campanha) continuam sendo ignorados.
    assert_eq!(public_names(&app, "?utm_source=newsletter").await, ["Mug"]);
}

/// Cria variantes do produto na ordem dada (nome, SKU) e devolve o id de cada uma.
async fn add_variants(
    app: &TestApp,
    admin: &str,
    product_id: &str,
    variants: &[(&str, &str)],
) -> Vec<String> {
    let mut ids = Vec::new();
    for (name, sku) in variants {
        let (status, created) = app
            .send(
                "POST",
                &format!("/products/{product_id}/variants"),
                Some(admin),
                Some(json!({ "name": name, "sku": sku, "price": 10, "stock": 5 })),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "{name}");
        ids.push(created["id"].as_str().unwrap().to_string());
    }
    ids
}

fn skus(list: &serde_json::Value) -> Vec<String> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|variant| variant["sku"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn variants_are_listed_by_name_then_sku_whatever_the_creation_order() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    // Seis variantes fora de ordem (mais a "Default" do produto), para a ordem do banco
    // dificilmente coincidir por acaso. Os SKUs não seguem a ordem dos nomes.
    add_variants(
        &app,
        &admin,
        &mug.product_id,
        &[
            ("Red", "S-1"),
            ("Blue", "S-6"),
            ("Zed", "S-2"),
            ("Alpha", "S-5"),
            ("Green", "S-3"),
            ("Black", "S-4"),
        ],
    )
    .await;
    let list_uri = format!("/products/{}/variants", mug.product_id);
    let expected_names = ["Alpha", "Black", "Blue", "Default", "Green", "Red", "Zed"];

    let (_, list) = app.send("GET", &list_uri, None, None).await;
    let (_, again) = app.send("GET", &list_uri, None, None).await;
    let (_, public_detail) = app
        .send("GET", &format!("/products/{}", mug.product_id), None, None)
        .await;
    let (_, admin_detail) = app
        .send(
            "GET",
            &format!("/admin/products/{}", mug.product_id),
            Some(&admin),
            None,
        )
        .await;

    let names_of = |variants: &serde_json::Value| -> Vec<String> {
        variants
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["name"].as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(names_of(&list), expected_names);
    assert_eq!(list, again, "the order must not change between calls");
    assert_eq!(names_of(&public_detail["variants"]), expected_names);
    assert_eq!(names_of(&admin_detail["variants"]), expected_names);
    assert_eq!(
        public_detail["variants"], list,
        "detail and list must agree"
    );
}

#[tokio::test]
async fn variants_with_the_same_name_are_ordered_by_sku() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    add_variants(
        &app,
        &admin,
        &mug.product_id,
        &[
            ("Same", "S-4"),
            ("Same", "S-1"),
            ("Same", "S-6"),
            ("Same", "S-3"),
            ("Same", "S-5"),
            ("Same", "S-2"),
        ],
    )
    .await;

    let (_, list) = app
        .send(
            "GET",
            &format!("/products/{}/variants", mug.product_id),
            None,
            None,
        )
        .await;

    let same: Vec<String> = list
        .as_array()
        .unwrap()
        .iter()
        .filter(|v| v["name"] == "Same")
        .map(|v| v["sku"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(same, ["S-1", "S-2", "S-3", "S-4", "S-5", "S-6"]);
    assert_eq!(skus(&list).len(), 7);
}

#[tokio::test]
async fn the_admin_detail_keeps_the_same_order_and_adds_the_inactive_variants() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    let ids = add_variants(
        &app,
        &admin,
        &mug.product_id,
        &[("Zed", "S-1"), ("Alpha", "S-2"), ("Mid", "S-3")],
    )
    .await;
    // Desativa a "Alpha": some do público, continua no admin, na mesma posição relativa.
    app.send(
        "DELETE",
        &format!("/products/{}/variants/{}", mug.product_id, ids[1]),
        Some(&admin),
        None,
    )
    .await;

    let (_, public) = app
        .send(
            "GET",
            &format!("/products/{}/variants", mug.product_id),
            None,
            None,
        )
        .await;
    let (_, admin_detail) = app
        .send(
            "GET",
            &format!("/admin/products/{}", mug.product_id),
            Some(&admin),
            None,
        )
        .await;

    let names = |v: &serde_json::Value| -> Vec<String> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|x| x["name"].as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(names(&public), ["Default", "Mid", "Zed"]);
    assert_eq!(
        names(&admin_detail["variants"]),
        ["Alpha", "Default", "Mid", "Zed"]
    );
    assert_eq!(admin_detail["variants"][0]["active"], false);
}

async fn page(app: &TestApp, query: &str) -> (Vec<String>, Option<String>) {
    let (status, headers, body) =
        super::send_full(&app.router, "GET", &format!("/products{query}"), None, None).await;
    assert_eq!(status, StatusCode::OK, "{query}");
    let total = headers
        .get("x-total-count")
        .map(|value| value.to_str().unwrap().to_string());
    (product_names(&body), total)
}

/// Cria produtos "P01", "P02", ... (a ordem por nome é a de criação).
async fn create_numbered(app: &TestApp, admin: &str, count: usize) {
    for n in 1..=count {
        app.setup_product(admin, &format!("P{n:02}"), 10, 5).await;
    }
}

#[tokio::test]
async fn without_a_limit_the_public_list_still_returns_every_active_product() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    // Mais que o teto de `limit` (200) e que qualquer página padrão: o contrato antigo
    // (tudo de uma vez) não pode ter mudado.
    for n in 0..205 {
        app.setup_product(&admin, &format!("Item {n:03}"), 10, 5)
            .await;
    }

    let (names, total) = page(&app, "").await;

    assert_eq!(names.len(), 205);
    assert_eq!(total.as_deref(), Some("205"));
    assert_eq!(names.first().unwrap(), "Item 000");
    assert_eq!(names.last().unwrap(), "Item 204");
}

#[tokio::test]
async fn limit_and_offset_cut_the_list_into_pages() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    create_numbered(&app, &admin, 5).await;

    assert_eq!(page(&app, "?limit=2").await.0, ["P01", "P02"]);
    assert_eq!(page(&app, "?limit=2&offset=2").await.0, ["P03", "P04"]);
    assert_eq!(page(&app, "?limit=2&offset=4").await.0, ["P05"]);
    assert!(page(&app, "?limit=2&offset=5").await.0.is_empty());
    assert!(page(&app, "?limit=2&offset=99").await.0.is_empty());
    assert_eq!(page(&app, "?limit=200").await.0.len(), 5);
    // Só `offset`: o resto da lista a partir dali.
    assert_eq!(page(&app, "?offset=3").await.0, ["P04", "P05"]);
    assert_eq!(page(&app, "?offset=0").await.0.len(), 5);
}

#[tokio::test]
async fn pages_follow_the_chosen_order_and_never_include_inactive_products() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    create_numbered(&app, &admin, 5).await;
    let (_, list) = app.send("GET", "/products", None, None).await;
    let p03 = list[2]["id"].as_str().unwrap().to_string();
    app.send("DELETE", &format!("/products/{p03}"), Some(&admin), None)
        .await;

    // Sem o P03, e sem deixar buraco entre as páginas.
    assert_eq!(page(&app, "?limit=2").await.0, ["P01", "P02"]);
    assert_eq!(page(&app, "?limit=2&offset=2").await.0, ["P04", "P05"]);
    // Do mais novo para o mais antigo, as páginas seguem essa ordem.
    assert_eq!(page(&app, "?sort=newest&limit=2").await.0, ["P05", "P04"]);
    assert_eq!(
        page(&app, "?sort=newest&limit=2&offset=2").await.0,
        ["P02", "P01"]
    );
}

#[tokio::test]
async fn invalid_pagination_parameters_are_422() {
    let app = TestApp::new().await;

    for query in [
        "?limit=0",
        "?limit=201",
        "?limit=abc",
        "?limit=-1",
        "?offset=-1",
        "?offset=x",
    ] {
        let (status, body) = app
            .send("GET", &format!("/products{query}"), None, None)
            .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{query}");
        assert!(body["error"].is_string(), "{query}");
    }
    let (_, body) = app.send("GET", "/products?limit=0", None, None).await;
    assert_eq!(body["error"], "limit: must be between 1 and 200");
    let (_, body) = app.send("GET", "/products?offset=-1", None, None).await;
    assert_eq!(body["error"], "offset: must not be negative");
}

#[tokio::test]
async fn x_total_count_is_the_number_of_active_products_whatever_the_page() {
    let app = TestApp::new().await;
    let admin = app.admin().await;

    // Catálogo vazio: o total é 0, não ausente.
    let (names, total) = page(&app, "").await;
    assert!(names.is_empty());
    assert_eq!(total.as_deref(), Some("0"));

    create_numbered(&app, &admin, 5).await;
    for query in [
        "",
        "?limit=2",
        "?limit=2&offset=4",
        "?offset=99",
        "?sort=newest&limit=1",
    ] {
        assert_eq!(page(&app, query).await.1.as_deref(), Some("5"), "{query}");
    }

    // Desativar um produto reduz o total.
    let (_, list) = app.send("GET", "/products", None, None).await;
    let first = list[0]["id"].as_str().unwrap().to_string();
    app.send("DELETE", &format!("/products/{first}"), Some(&admin), None)
        .await;
    assert_eq!(page(&app, "?limit=2").await.1.as_deref(), Some("4"));
}

/// Nomes que a ordem binária (maiúsculas antes de minúsculas, acentos no fim) erraria.
const MIXED_NAMES: [&str; 6] = ["zebra", "Zed", "alpha", "Beta", "Álamo", "charlie"];
const MIXED_SORTED: [&str; 6] = ["Álamo", "alpha", "Beta", "charlie", "zebra", "Zed"];

fn folded(names: &[String]) -> Vec<String> {
    names.iter().map(|name| name.to_lowercase()).collect()
}

#[tokio::test]
async fn the_public_product_list_sorts_text_ignoring_case_and_accents() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    for name in MIXED_NAMES {
        app.setup_product(&admin, name, 10, 5).await;
    }

    assert_eq!(public_names(&app, "").await, MIXED_SORTED);
    assert_eq!(public_names(&app, "?sort=name").await, MIXED_SORTED);
    // A paginação corta a mesma ordem, sem buracos nem repetição.
    assert_eq!(public_names(&app, "?limit=2").await, ["Álamo", "alpha"]);
    assert_eq!(
        public_names(&app, "?limit=2&offset=2").await,
        ["Beta", "charlie"]
    );
    assert_eq!(
        public_names(&app, "?limit=2&offset=4").await,
        ["zebra", "Zed"]
    );
}

#[tokio::test]
async fn names_that_differ_only_in_case_stay_together_in_a_stable_order() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    for name in ["beta", "Alpha", "BETA", "alpha", "Beta", "ALPHA"] {
        app.setup_product(&admin, name, 10, 5).await;
    }

    let first = public_names(&app, "").await;
    let second = public_names(&app, "").await;

    // Ignorando a caixa, a lista está em ordem (alphas juntos, depois betas juntos).
    assert_eq!(
        folded(&first),
        ["alpha", "alpha", "alpha", "beta", "beta", "beta"]
    );
    // Dentro de cada grupo a ordem não oscila entre chamadas.
    assert_eq!(first, second);
}

#[tokio::test]
async fn variants_sort_text_ignoring_case_and_accents() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    add_variants(
        &app,
        &admin,
        &mug.product_id,
        &[
            ("zeta", "S-1"),
            ("Alpha", "S-2"),
            ("beta", "S-3"),
            ("Álamo", "S-4"),
        ],
    )
    .await;
    let expected = ["Álamo", "Alpha", "beta", "Default", "zeta"];

    let (_, list) = app
        .send(
            "GET",
            &format!("/products/{}/variants", mug.product_id),
            None,
            None,
        )
        .await;
    let (_, detail) = app
        .send("GET", &format!("/products/{}", mug.product_id), None, None)
        .await;
    let (_, admin_detail) = app
        .send(
            "GET",
            &format!("/admin/products/{}", mug.product_id),
            Some(&admin),
            None,
        )
        .await;

    let names_of = |variants: &serde_json::Value| -> Vec<String> {
        variants
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["name"].as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(names_of(&list), expected);
    assert_eq!(names_of(&detail["variants"]), expected);
    assert_eq!(names_of(&admin_detail["variants"]), expected);
}
