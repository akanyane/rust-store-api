use axum::http::StatusCode;
use serde_json::{Value, json};

use super::{Item, TestApp};

fn names(list: &Value) -> Vec<String> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|product| product["name"].as_str().unwrap().to_string())
        .collect()
}

async fn deactivate_product(app: &TestApp, admin: &str, item: &Item) {
    let (status, _) = app
        .send(
            "DELETE",
            &format!("/products/{}", item.product_id),
            Some(admin),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn admin_product_routes_require_an_admin() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let customer = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;

    for uri in [
        "/admin/products".to_string(),
        format!("/admin/products/{}", mug.product_id),
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
}

#[tokio::test]
async fn admin_sees_inactive_products_that_the_public_catalog_hides() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let _alpha = app.setup_product(&admin, "Alpha", 10, 5).await;
    let bravo = app.setup_product(&admin, "Bravo", 10, 5).await;
    let _charlie = app.setup_product(&admin, "Charlie", 10, 5).await;
    deactivate_product(&app, &admin, &bravo).await;

    let (_, public) = app.send("GET", "/products", None, None).await;
    let (_, all) = app.send("GET", "/admin/products", Some(&admin), None).await;
    let (_, inactive) = app
        .send("GET", "/admin/products?active=false", Some(&admin), None)
        .await;
    let (_, active) = app
        .send("GET", "/admin/products?active=true", Some(&admin), None)
        .await;

    // A listagem pública não tem ordem definida; só o conteúdo importa aqui.
    let mut public_names = names(&public);
    public_names.sort();
    assert_eq!(public_names, ["Alpha", "Charlie"]);
    assert_eq!(names(&all), ["Alpha", "Bravo", "Charlie"]);
    assert_eq!(names(&inactive), ["Bravo"]);
    assert_eq!(inactive[0]["active"], false);
    assert_eq!(inactive[0]["id"], bravo.product_id);
    assert_eq!(names(&active), ["Alpha", "Charlie"]);
}

#[tokio::test]
async fn listing_is_ordered_by_name_and_paginates_stably() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    for name in ["Charlie", "Alpha", "Delta", "Bravo"] {
        app.setup_product(&admin, name, 10, 5).await;
    }

    let page = |query: &'static str| {
        let app = &app;
        let admin = admin.clone();
        async move {
            let (status, body) = app
                .send(
                    "GET",
                    &format!("/admin/products?{query}"),
                    Some(&admin),
                    None,
                )
                .await;
            assert_eq!(status, StatusCode::OK, "{query}");
            body
        }
    };

    assert_eq!(names(&page("limit=2").await), ["Alpha", "Bravo"]);
    assert_eq!(names(&page("limit=2&offset=2").await), ["Charlie", "Delta"]);
    assert_eq!(names(&page("limit=3&offset=3").await), ["Delta"]);
    assert!(names(&page("offset=4").await).is_empty());
}

#[tokio::test]
async fn products_with_the_same_name_keep_a_stable_order_across_pages() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let first = app.setup_product(&admin, "Same", 10, 5).await;
    let second = app.setup_product(&admin, "Same", 10, 5).await;
    let mut expected = vec![first.product_id, second.product_id];
    expected.sort();

    let mut seen = Vec::new();
    for offset in 0..2 {
        let (_, page) = app
            .send(
                "GET",
                &format!("/admin/products?limit=1&offset={offset}"),
                Some(&admin),
                None,
            )
            .await;
        seen.push(page[0]["id"].as_str().unwrap().to_string());
    }

    assert_eq!(seen, expected);
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
        "active=maybe",
    ] {
        let (status, body) = app
            .send(
                "GET",
                &format!("/admin/products?{query}"),
                Some(&admin),
                None,
            )
            .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{query}");
        assert!(body["error"].is_string(), "{query}");
    }

    let (_, body) = app
        .send("GET", "/admin/products?limit=0", Some(&admin), None)
        .await;
    assert_eq!(body["error"], "limit: must be between 1 and 200");
}

#[tokio::test]
async fn admin_detail_shows_an_inactive_product_and_all_of_its_variants() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    let (status, blue) = app
        .send(
            "POST",
            &format!("/products/{}/variants", mug.product_id),
            Some(&admin),
            Some(json!({ "name": "Blue", "sku": "MUG-BLUE", "price": 12, "stock": 3 })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let blue_id = blue["id"].as_str().unwrap();
    let detail_uri = format!("/admin/products/{}", mug.product_id);

    // Variante desativada: o público não vê, o admin vê com active=false.
    app.send(
        "DELETE",
        &format!("/products/{}/variants/{blue_id}", mug.product_id),
        Some(&admin),
        None,
    )
    .await;
    let (_, public) = app
        .send("GET", &format!("/products/{}", mug.product_id), None, None)
        .await;
    let (_, admin_view) = app.send("GET", &detail_uri, Some(&admin), None).await;
    assert_eq!(public["variants"].as_array().unwrap().len(), 1);
    let variants = admin_view["variants"].as_array().unwrap();
    assert_eq!(variants.len(), 2);
    let blue_view = variants.iter().find(|v| v["id"] == blue_id).unwrap();
    assert_eq!(blue_view["active"], false);
    assert_eq!(blue_view["sku"], "MUG-BLUE");

    // Produto desativado: 404 no público, 200 para o admin.
    deactivate_product(&app, &admin, &mug).await;
    let public_status = app
        .send("GET", &format!("/products/{}", mug.product_id), None, None)
        .await
        .0;
    let (status, inactive_view) = app.send("GET", &detail_uri, Some(&admin), None).await;
    assert_eq!(public_status, StatusCode::NOT_FOUND);
    assert_eq!(status, StatusCode::OK);
    assert_eq!(inactive_view["active"], false);
    assert_eq!(inactive_view["variants"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn unknown_product_is_404_for_the_admin() {
    let app = TestApp::new().await;
    let admin = app.admin().await;

    assert_eq!(
        app.send("GET", "/admin/products/nope", Some(&admin), None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn an_admin_can_reactivate_using_only_what_the_admin_routes_reveal() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let hat = app.setup_product(&admin, "Hat", 10, 5).await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    // Hat: produto desativado. Mug: só a variante desativada.
    deactivate_product(&app, &admin, &hat).await;
    app.send(
        "DELETE",
        &format!("/products/{}/variants/{}", mug.product_id, mug.variant_id),
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(
        app.send("GET", &format!("/products/{}", hat.product_id), None, None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert!(
        app.send(
            "GET",
            &format!("/products/{}/variants", mug.product_id),
            None,
            None
        )
        .await
        .1
        .as_array()
        .unwrap()
        .is_empty()
    );

    // O admin redescobre os dois ids sem tê-los guardado.
    let (_, inactive) = app
        .send("GET", "/admin/products?active=false", Some(&admin), None)
        .await;
    let hat_id = inactive
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "Hat")
        .expect("Hat listed as inactive")["id"]
        .as_str()
        .unwrap()
        .to_string();
    let (_, mug_detail) = app
        .send(
            "GET",
            &format!("/admin/products/{}", mug.product_id),
            Some(&admin),
            None,
        )
        .await;
    let off_variant = mug_detail["variants"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["active"] == false)
        .expect("inactive variant listed");
    let off_variant_id = off_variant["id"].as_str().unwrap().to_string();
    let off_sku = off_variant["sku"].as_str().unwrap().to_string();

    // Reativa pelas rotas que já existiam.
    let (status, _) = app
        .send(
            "PUT",
            &format!("/products/{hat_id}"),
            Some(&admin),
            Some(json!({ "name": "Hat", "description": "d", "active": true })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = app
        .send(
            "PUT",
            &format!("/products/{}/variants/{off_variant_id}", mug.product_id),
            Some(&admin),
            Some(json!({ "name": "Default", "sku": off_sku, "price": 10, "stock": 5, "active": true })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    assert_eq!(
        app.send("GET", &format!("/products/{hat_id}"), None, None)
            .await
            .0,
        StatusCode::OK
    );
    let (_, public_variants) = app
        .send(
            "GET",
            &format!("/products/{}/variants", mug.product_id),
            None,
            None,
        )
        .await;
    assert_eq!(public_variants.as_array().unwrap().len(), 1);
    let (_, none_left) = app
        .send("GET", "/admin/products?active=false", Some(&admin), None)
        .await;
    assert!(none_left.as_array().unwrap().is_empty());
}
