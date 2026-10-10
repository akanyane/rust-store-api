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
