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

    assert_eq!(names(&public), ["Alpha", "Charlie"]);
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

fn created_at_of(product: &Value) -> Value {
    product["created_at"].clone()
}

#[tokio::test]
async fn a_new_product_records_when_it_was_created() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let before = chrono::Utc::now();

    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    let after = chrono::Utc::now();

    let (_, admin_view) = app
        .send(
            "GET",
            &format!("/admin/products/{}", mug.product_id),
            Some(&admin),
            None,
        )
        .await;
    let created_at = super::orders::parse_time(&admin_view["created_at"]);
    assert!(created_at >= before && created_at <= after, "{created_at}");

    // O mesmo valor aparece no detalhe público e nas duas listagens.
    let (_, public_detail) = app
        .send("GET", &format!("/products/{}", mug.product_id), None, None)
        .await;
    let (_, public_list) = app.send("GET", "/products", None, None).await;
    let (_, admin_list) = app.send("GET", "/admin/products", Some(&admin), None).await;
    assert_eq!(created_at_of(&public_detail), admin_view["created_at"]);
    assert_eq!(created_at_of(&public_list[0]), admin_view["created_at"]);
    assert_eq!(created_at_of(&admin_list[0]), admin_view["created_at"]);
}

#[tokio::test]
async fn created_at_survives_edits_deactivation_and_reactivation() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    let detail_uri = format!("/admin/products/{}", mug.product_id);
    let product_uri = format!("/products/{}", mug.product_id);
    let (_, original) = app.send("GET", &detail_uri, Some(&admin), None).await;
    assert!(original["created_at"].is_string());

    let put = |name: &'static str, active: bool| json!({ "name": name, "description": "edited", "active": active });
    let (status, edited) = app
        .send(
            "PUT",
            &product_uri,
            Some(&admin),
            Some(put("Big Mug", true)),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(edited["created_at"], original["created_at"], "after PUT");
    assert_eq!(edited["name"], "Big Mug");
    assert_eq!(edited["description"], "edited");

    deactivate_product(&app, &admin, &mug).await;
    let (_, inactive) = app.send("GET", &detail_uri, Some(&admin), None).await;
    assert_eq!(
        inactive["created_at"], original["created_at"],
        "after DELETE"
    );
    assert_eq!(
        inactive["name"], "Big Mug",
        "DELETE must keep the edited fields"
    );

    let (_, reactivated) = app
        .send(
            "PUT",
            &product_uri,
            Some(&admin),
            Some(put("Big Mug", true)),
        )
        .await;
    assert_eq!(
        reactivated["created_at"], original["created_at"],
        "after reactivation"
    );
    assert_eq!(reactivated["active"], true);
}

#[tokio::test]
async fn sort_newest_lists_the_most_recent_first_and_paginates() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    // Criados nesta ordem: a ordem por data é Charlie, Alpha, Bravo; por nome é outra.
    for name in ["Bravo", "Alpha", "Charlie"] {
        app.setup_product(&admin, name, 10, 5).await;
    }
    let list = |query: &'static str| {
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
            names(&body)
        }
    };

    assert_eq!(list("sort=newest").await, ["Charlie", "Alpha", "Bravo"]);
    assert_eq!(
        list("sort=newest&limit=2&offset=1").await,
        ["Alpha", "Bravo"]
    );
    // O padrão continua sendo por nome.
    assert_eq!(list("sort=name").await, ["Alpha", "Bravo", "Charlie"]);
    assert_eq!(list("limit=3").await, ["Alpha", "Bravo", "Charlie"]);
}

#[tokio::test]
async fn products_without_a_creation_date_come_last_and_never_get_one() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    // Produto "antigo": gravado direto no banco, sem `created_at`, como os criados antes
    // do campo existir.
    app.db
        .query("CREATE product SET name = 'Legacy', description = 'old', active = true")
        .await
        .unwrap()
        .check()
        .unwrap();
    app.setup_product(&admin, "Newer", 10, 5).await;

    let (_, list) = app
        .send("GET", "/admin/products?sort=newest", Some(&admin), None)
        .await;
    assert_eq!(names(&list), ["Newer", "Legacy"]);
    assert!(list[0]["created_at"].is_string());
    assert!(list[1]["created_at"].is_null());
    let legacy_id = list[1]["id"].as_str().unwrap().to_string();

    // Editar, desativar e reativar não carimba uma data que nunca existiu.
    let uri = format!("/products/{legacy_id}");
    let put = json!({ "name": "Legacy", "description": "edited", "active": true });
    let (status, edited) = app.send("PUT", &uri, Some(&admin), Some(put.clone())).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        edited["created_at"].is_null(),
        "PUT invented a date: {edited}"
    );
    app.send("DELETE", &uri, Some(&admin), None).await;
    let (_, after) = app.send("PUT", &uri, Some(&admin), Some(put)).await;
    assert!(after["created_at"].is_null());
    let (_, public) = app.send("GET", &uri, None, None).await;
    assert!(public["created_at"].is_null());
}

#[tokio::test]
async fn an_unknown_sort_is_422() {
    let app = TestApp::new().await;
    let admin = app.admin().await;

    for query in ["sort=oldest", "sort=NAME", "sort="] {
        let (status, body) = app
            .send(
                "GET",
                &format!("/admin/products?{query}"),
                Some(&admin),
                None,
            )
            .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{query}");
        assert_eq!(body["error"], "Invalid query parameters", "{query}");
    }
}

/// Devolve o `X-Total-Count` e quantos itens vieram no corpo.
async fn total_and_len(app: &TestApp, admin: &str, query: &str) -> (Option<String>, usize) {
    let (status, headers, body) = super::send_full(
        &app.router,
        "GET",
        &format!("/admin/products{query}"),
        Some(admin),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{query}");
    let total = headers
        .get("x-total-count")
        .map(|value| value.to_str().unwrap().to_string());
    (total, body.as_array().unwrap().len())
}

#[tokio::test]
async fn the_admin_total_matches_the_filter_and_ignores_limit_and_offset() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let mut items = Vec::new();
    for n in 1..=5 {
        items.push(app.setup_product(&admin, &format!("P{n:02}"), 10, 5).await);
    }
    // 3 ativos (P01, P03, P05) e 2 inativos (P02, P04).
    deactivate_product(&app, &admin, &items[1]).await;
    deactivate_product(&app, &admin, &items[3]).await;

    // Sem filtro: tudo; com filtro: só o que casa. Sem paginar, o corpo fecha com o total.
    assert_eq!(total_and_len(&app, &admin, "").await, (Some("5".into()), 5));
    assert_eq!(
        total_and_len(&app, &admin, "?active=true").await,
        (Some("3".into()), 3)
    );
    assert_eq!(
        total_and_len(&app, &admin, "?active=false").await,
        (Some("2".into()), 2)
    );

    // `limit` e `offset` cortam o corpo, mas não o total.
    assert_eq!(
        total_and_len(&app, &admin, "?limit=2").await,
        (Some("5".into()), 2)
    );
    assert_eq!(
        total_and_len(&app, &admin, "?active=false&limit=1").await,
        (Some("2".into()), 1)
    );
    assert_eq!(
        total_and_len(&app, &admin, "?active=true&limit=1&offset=2").await,
        (Some("3".into()), 1)
    );
    assert_eq!(
        total_and_len(&app, &admin, "?active=true&offset=99").await,
        (Some("3".into()), 0)
    );
    assert_eq!(
        total_and_len(&app, &admin, "?sort=newest&limit=1").await,
        (Some("5".into()), 1)
    );

    // Reativar um produto move a contagem de um filtro para o outro.
    let (status, _) = app
        .send(
            "PUT",
            &format!("/products/{}", items[1].product_id),
            Some(&admin),
            Some(json!({ "name": "P02", "description": "d", "active": true })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        total_and_len(&app, &admin, "?active=true")
            .await
            .0
            .as_deref(),
        Some("4")
    );
    assert_eq!(
        total_and_len(&app, &admin, "?active=false")
            .await
            .0
            .as_deref(),
        Some("1")
    );
}

#[tokio::test]
async fn the_admin_total_is_zero_not_missing_when_nothing_matches() {
    let app = TestApp::new().await;
    let admin = app.admin().await;

    assert_eq!(total_and_len(&app, &admin, "").await, (Some("0".into()), 0));
    assert_eq!(
        total_and_len(&app, &admin, "?active=false").await,
        (Some("0".into()), 0)
    );

    // Só há ativos: o filtro de inativos continua dando 0.
    app.setup_product(&admin, "Mug", 10, 5).await;
    assert_eq!(
        total_and_len(&app, &admin, "?active=false").await,
        (Some("0".into()), 0)
    );
    assert_eq!(
        total_and_len(&app, &admin, "?active=true").await,
        (Some("1".into()), 1)
    );
}
