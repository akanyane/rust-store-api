use axum::{
    body::Body,
    http::{HeaderMap, Request, StatusCode, header},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use super::orders::place_order;
use super::{TestApp, send_full};
use crate::models::order::{ExportOrdersQuery, OrderStatus};
use crate::services::order::export_orders_csv;

const HEADER: &str =
    "order_id,order_number,customer_id,status,items,units,total,created_at,paid_at";

/// Baixa o CSV: `(status, cabeçalhos, corpo)`.
async fn download(
    app: &TestApp,
    query: &str,
    token: Option<&str>,
) -> (StatusCode, HeaderMap, String) {
    let mut builder = Request::builder()
        .method("GET")
        .uri(format!("/admin/orders/export.csv{query}"));
    if let Some(token) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let response = app
        .router
        .clone()
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, headers, String::from_utf8(bytes.to_vec()).unwrap())
}

/// As linhas de dados (sem o cabeçalho), cada uma em colunas. Os campos não têm vírgula.
fn rows(csv: &str) -> Vec<Vec<String>> {
    let mut lines: Vec<&str> = csv.split("\r\n").collect();
    assert_eq!(lines.pop(), Some(""), "the file ends with CRLF");
    assert_eq!(lines[0], HEADER);
    lines[1..]
        .iter()
        .map(|line| line.split(',').map(String::from).collect())
        .collect()
}

async fn admin_download(app: &TestApp, admin: &str, query: &str) -> Vec<Vec<String>> {
    let (status, _, body) = download(app, query, Some(admin)).await;
    assert_eq!(status, StatusCode::OK, "{query}: {body}");
    rows(&body)
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

#[tokio::test]
async fn the_export_requires_an_admin() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let ana = app.customer("ana").await;

    assert_eq!(download(&app, "", None).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(
        download(&app, "", Some("nope")).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        download(&app, "", Some(&ana)).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(download(&app, "", Some(&admin)).await.0, StatusCode::OK);
}

#[tokio::test]
async fn an_empty_store_exports_only_the_header_row() {
    let app = TestApp::new().await;
    let admin = app.admin().await;

    let (status, headers, body) = download(&app, "", Some(&admin)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, format!("{HEADER}\r\n"));
    assert_eq!(headers.get("x-total-count").unwrap(), "0");
}

#[tokio::test]
async fn the_response_is_a_download_with_the_right_headers() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let ana = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    place_order(&app, &ana, &mug, 1).await;

    let (_, headers, _) = download(&app, "", Some(&admin)).await;

    assert_eq!(
        headers.get(header::CONTENT_TYPE).unwrap(),
        "text/csv; charset=utf-8"
    );
    let disposition = headers
        .get(header::CONTENT_DISPOSITION)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(
        disposition.starts_with("attachment; filename=\"orders-"),
        "{disposition}"
    );
    assert!(disposition.ends_with(".csv\""), "{disposition}");
    let day = &disposition["attachment; filename=\"orders-".len()..][..10];
    assert!(
        chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d").is_ok(),
        "{day}"
    );
    assert_eq!(headers.get("x-total-count").unwrap(), "1");
    assert!(headers.get("x-export-truncated").is_none());
}

#[tokio::test]
async fn one_row_per_order_newest_first_with_the_columns_in_order() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let ana = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 50).await;
    let first = place_order(&app, &ana, &mug, 1).await;
    let second = place_order(&app, &ana, &mug, 3).await;

    let lines = admin_download(&app, &admin, "").await;

    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0][0], second, "newest first");
    assert_eq!(lines[1][0], first);
    for line in &lines {
        assert_eq!(line.len(), 9, "{line:?}");
    }
    // O pedido de 3 unidades: 1 item, 3 unidades, total 30, pendente.
    assert_eq!(lines[0][3], "pending");
    assert_eq!(lines[0][4], "1");
    assert_eq!(lines[0][5], "3");
    assert_eq!(lines[0][6], "30");
    assert_eq!(lines[0][8], "", "not paid yet");
}

#[tokio::test]
async fn the_order_number_is_the_first_eight_characters_in_capitals_with_a_hash() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let ana = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 5).await;
    let id = place_order(&app, &ana, &mug, 1).await;

    let lines = admin_download(&app, &admin, "").await;

    assert_eq!(lines[0][1], format!("#{}", id[..8].to_uppercase()));
}

#[tokio::test]
async fn items_and_units_count_every_line_of_a_multi_item_order() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let ana = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 50).await;
    let tote = app.setup_product(&admin, "Tote", 40, 50).await;
    assert_eq!(app.add_to_cart(&ana, &mug, 2).await, StatusCode::OK);
    assert_eq!(app.add_to_cart(&ana, &tote, 3).await, StatusCode::OK);
    let (status, _) = app.send("POST", "/orders", Some(&ana), None).await;
    assert_eq!(status, StatusCode::CREATED);

    let lines = admin_download(&app, &admin, "").await;

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0][4], "2", "two lines");
    assert_eq!(lines[0][5], "5", "2 + 3 units");
    assert_eq!(lines[0][6], "140", "2 * 10 + 3 * 40");
}

#[tokio::test]
async fn paid_at_is_filled_once_the_order_is_paid_and_matches_the_api() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let ana = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 50).await;
    let paid = place_order(&app, &ana, &mug, 1).await;
    let pending = place_order(&app, &ana, &mug, 1).await;
    set_status(&app, &admin, &paid, "paid").await;

    let lines = admin_download(&app, &admin, "").await;
    let (_, order) = app
        .send("GET", &format!("/admin/orders/{paid}"), Some(&admin), None)
        .await;

    let of = |id: &str| lines.iter().find(|l| l[0] == id).unwrap().clone();
    // O arquivo corta os segundos fracionários: mesma data e hora, até o segundo.
    let api = order["paid_at"].as_str().unwrap();
    assert_eq!(of(&paid)[8], format!("{}Z", &api[..19]));
    assert_eq!(of(&pending)[8], "");
}

#[tokio::test]
async fn every_column_matches_what_the_list_endpoint_says() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let ana = app.customer("ana").await;
    let bob = app.customer("bob").await;
    let mug = app.setup_product(&admin, "Mug", 10, 50).await;
    let a = place_order(&app, &ana, &mug, 2).await;
    place_order(&app, &bob, &mug, 1).await;
    set_status(&app, &admin, &a, "paid").await;

    let lines = admin_download(&app, &admin, "").await;
    let (_, list) = app.send("GET", "/admin/orders", Some(&admin), None).await;

    let list = list.as_array().unwrap();
    assert_eq!(lines.len(), list.len());
    for (line, order) in lines.iter().zip(list) {
        assert_eq!(line[0], order["id"].as_str().unwrap());
        assert_eq!(line[2], order["customer_id"].as_str().unwrap());
        assert_eq!(line[3], order["status"].as_str().unwrap());
        assert_eq!(line[6], order["total"].as_i64().unwrap().to_string());
        let created = order["created_at"].as_str().unwrap();
        assert!(
            created.starts_with(&line[7][..19]),
            "{created} vs {}",
            line[7]
        );
        assert!(line[7].ends_with('Z'));
    }
}

#[tokio::test]
async fn the_status_filter_and_the_search_narrow_the_file_like_they_narrow_the_list() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let ana = app.customer("ana").await;
    let bob = app.customer("bob").await;
    let mug = app.setup_product(&admin, "Mug", 10, 50).await;
    let a1 = place_order(&app, &ana, &mug, 1).await;
    let a2 = place_order(&app, &ana, &mug, 2).await;
    let b1 = place_order(&app, &bob, &mug, 1).await;
    set_status(&app, &admin, &a1, "paid").await;
    set_status(&app, &admin, &b1, "paid").await;

    let ids = |lines: &[Vec<String>]| lines.iter().map(|l| l[0].clone()).collect::<Vec<_>>();
    let sorted = |mut v: Vec<String>| {
        v.sort();
        v
    };

    let paid = admin_download(&app, &admin, "?status=paid").await;
    assert_eq!(sorted(ids(&paid)), sorted(vec![a1.clone(), b1.clone()]));

    let ana_id = admin_download(&app, &admin, "")
        .await
        .iter()
        .find(|l| l[0] == a1)
        .unwrap()[2]
        .clone();
    let by_customer = admin_download(&app, &admin, &format!("?q={ana_id}")).await;
    assert_eq!(
        sorted(ids(&by_customer)),
        sorted(vec![a1.clone(), a2.clone()])
    );

    let by_number =
        admin_download(&app, &admin, &format!("?q=%23{}", a2[..8].to_uppercase())).await;
    assert_eq!(ids(&by_number), vec![a2.clone()]);

    let both = admin_download(&app, &admin, &format!("?q={ana_id}&status=paid")).await;
    assert_eq!(ids(&both), vec![a1.clone()]);

    let none = admin_download(&app, &admin, "?q=zzzzzz").await;
    assert!(none.is_empty());
}

#[tokio::test]
async fn the_total_header_counts_what_matches() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let ana = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 50).await;
    let a = place_order(&app, &ana, &mug, 1).await;
    place_order(&app, &ana, &mug, 1).await;
    set_status(&app, &admin, &a, "paid").await;

    let (_, all, _) = download(&app, "", Some(&admin)).await;
    let (_, paid, _) = download(&app, "?status=paid", Some(&admin)).await;
    let (_, none, _) = download(&app, "?q=zzz", Some(&admin)).await;

    assert_eq!(all.get("x-total-count").unwrap(), "2");
    assert_eq!(paid.get("x-total-count").unwrap(), "1");
    assert_eq!(none.get("x-total-count").unwrap(), "0");
}

#[tokio::test]
async fn invalid_parameters_are_422_and_pagination_ones_do_not_exist() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let ana = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 50).await;
    for _ in 0..3 {
        place_order(&app, &ana, &mug, 1).await;
    }

    for query in [
        "?status=bogus".to_string(),
        format!("?q={}", "a".repeat(65)),
    ] {
        let (status, _, _) = download(&app, &query, Some(&admin)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{query}");
    }
    // `limit` e `offset` não fazem parte da exportação: são ignorados, e o arquivo traz tudo.
    let lines = admin_download(&app, &admin, "?limit=1&offset=1").await;
    assert_eq!(lines.len(), 3);
}

#[tokio::test]
async fn the_file_has_no_product_names_and_no_personal_data() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let ana = app.customer("ana").await;
    let mug = app
        .setup_product(&admin, "Distinctive Mug Name", 10, 5)
        .await;
    place_order(&app, &ana, &mug, 1).await;

    let (_, _, body) = download(&app, "", Some(&admin)).await;

    // Só strings que um id aleatório (minúsculo) ou o número do pedido (maiúsculo) nunca formam.
    for forbidden in [
        "Distinctive",
        "Mug",
        "first_name",
        "1990-01-01",
        "Test",
        "password",
    ] {
        assert!(!body.contains(forbidden), "{forbidden} leaked: {body}");
    }
    // E cada campo de texto tem a forma de um id, do número ou de um status, nada mais.
    for line in rows(&body) {
        assert!(
            line[0].len() == 20 && line[0].chars().all(|c| c.is_ascii_alphanumeric()),
            "{line:?}"
        );
        assert!(line[1].starts_with('#') && line[1].len() == 9, "{line:?}");
        assert!(
            line[2].len() == 20 && line[2].chars().all(|c| c.is_ascii_alphanumeric()),
            "{line:?}"
        );
        assert!(
            ["pending", "paid", "shipped", "delivered", "cancelled"].contains(&line[3].as_str()),
            "{line:?}"
        );
    }
}

#[tokio::test]
async fn a_cut_off_file_keeps_the_newest_orders_and_says_it_was_cut() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let ana = app.customer("ana").await;
    let mug = app.setup_product(&admin, "Mug", 10, 50).await;
    let oldest = place_order(&app, &ana, &mug, 1).await;
    let middle = place_order(&app, &ana, &mug, 1).await;
    let newest = place_order(&app, &ana, &mug, 1).await;

    let query = || ExportOrdersQuery {
        status: None::<OrderStatus>,
        q: None,
    };
    let (csv, total, truncated) = export_orders_csv(&app.db, query(), 2).await.unwrap();
    let lines = rows(&csv);
    assert_eq!(
        lines.iter().map(|l| l[0].clone()).collect::<Vec<_>>(),
        vec![newest.clone(), middle.clone()]
    );
    assert!(!csv.contains(&oldest));
    assert_eq!(total, 3);
    assert!(truncated);

    // Com o teto exato ou maior, não corta.
    for limit in [3, 4, 10_000] {
        let (csv, total, truncated) = export_orders_csv(&app.db, query(), limit).await.unwrap();
        assert_eq!(rows(&csv).len(), 3, "limit {limit}");
        assert_eq!(total, 3);
        assert!(!truncated, "limit {limit}");
    }
}

#[tokio::test]
async fn the_export_route_is_not_mistaken_for_an_order_id() {
    let app = TestApp::new().await;
    let admin = app.admin().await;

    let (status, _, _) = send_full(
        &app.router,
        "GET",
        "/admin/orders/export.csv",
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, body): (StatusCode, HeaderMap, Value) = send_full(
        &app.router,
        "GET",
        "/admin/orders/export",
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
}
