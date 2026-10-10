use axum::http::StatusCode;
use serde_json::json;

use super::{PASSWORD, TestApp, send};

#[tokio::test]
async fn sign_up_creates_customer_and_normalizes_email() {
    let app = TestApp::new().await;

    let (status, body) = app.sign_up("Ana@Test.DEV").await;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["email"], "ana@test.dev");
}

#[tokio::test]
async fn sign_up_with_existing_email_conflicts_without_echoing_it() {
    let app = TestApp::new().await;
    app.sign_up("ana@test.dev").await;

    let (status, body) = app.sign_up("ANA@test.dev").await;

    assert_eq!(status, StatusCode::CONFLICT);
    assert!(!body["error"].as_str().unwrap().contains("ana@test.dev"));
}

#[tokio::test]
async fn invalid_input_is_422_and_never_echoes_the_submitted_value() {
    let app = TestApp::new().await;

    let (status, body) = app
        .send(
            "POST",
            "/auth/sign-up",
            None,
            Some(json!({
                "email": "not-an-email",
                "password": PASSWORD,
                "first_name": "  ",
                "last_name": "User",
                "birthday": "2999-01-01",
            })),
        )
        .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let message = body["error"].as_str().unwrap();
    assert!(message.contains("email: invalid format"), "{message}");
    assert!(
        message.contains("first_name: must not be blank"),
        "{message}"
    );
    assert!(
        message.contains("birthday: must not be in the future"),
        "{message}"
    );
    assert!(!message.contains("not-an-email"), "{message}");
}

#[tokio::test]
async fn malformed_or_incomplete_json_is_a_generic_422() {
    let app = TestApp::new().await;

    let raw = super::send_raw(
        &app.router,
        "POST",
        "/auth/sign-up",
        None,
        Some("{nope".into()),
    )
    .await;
    let missing = app
        .send(
            "POST",
            "/auth/sign-up",
            None,
            Some(json!({ "email": "a@b.co" })),
        )
        .await;

    for (status, body) in [raw, missing] {
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["error"], "Invalid request body");
    }
}

#[tokio::test]
async fn short_password_is_rejected() {
    let app = TestApp::new().await;

    let (status, _) = app
        .send(
            "POST",
            "/auth/sign-up",
            None,
            Some(json!({
                "email": "ana@test.dev",
                "password": "short",
                "first_name": "Ana",
                "last_name": "Silva",
                "birthday": "1990-01-01",
            })),
        )
        .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn sign_in_failures_are_indistinguishable() {
    let app = TestApp::new().await;
    app.sign_up("ana@test.dev").await;

    let wrong_password = app
        .send(
            "POST",
            "/auth/sign-in",
            None,
            Some(json!({ "email": "ana@test.dev", "password": "wrong-password" })),
        )
        .await;
    let unknown_email = app
        .send(
            "POST",
            "/auth/sign-in",
            None,
            Some(json!({ "email": "nobody@test.dev", "password": PASSWORD })),
        )
        .await;

    assert_eq!(wrong_password.0, StatusCode::UNAUTHORIZED);
    assert_eq!(wrong_password, unknown_email);
}

#[tokio::test]
async fn protected_routes_require_a_valid_token() {
    let app = TestApp::new().await;

    let (no_token, _) = app.send("GET", "/me", None, None).await;
    let (bad_token, _) = app.send("GET", "/me", Some("garbage"), None).await;

    assert_eq!(no_token, StatusCode::UNAUTHORIZED);
    assert_eq!(bad_token, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn customer_cannot_use_admin_routes() {
    let app = TestApp::new().await;
    let customer = app.customer("ana@test.dev").await;

    let (status, _) = app
        .send(
            "POST",
            "/products",
            Some(&customer),
            Some(json!({ "name": "x", "description": "x", "price": 1, "stock": 1 })),
        )
        .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn refresh_rotates_the_tokens_and_old_ones_stop_working() {
    let app = TestApp::new().await;
    app.sign_up("ana@test.dev").await;
    let (_, old_session, old_refresh) = app.sign_in("ana@test.dev", PASSWORD).await;

    let (status, body) = app
        .send(
            "POST",
            "/auth/refresh",
            None,
            Some(json!({ "refresh_token": old_refresh })),
        )
        .await;

    assert_eq!(status, StatusCode::OK);
    let new_session = body["session_token"].as_str().unwrap();
    assert_ne!(new_session, old_session);
    assert_ne!(body["refresh_token"].as_str().unwrap(), old_refresh);
    assert_eq!(body["token_type"], "Bearer");

    assert_eq!(
        app.send("GET", "/me", Some(new_session), None).await.0,
        StatusCode::OK
    );
    assert_eq!(
        app.send("GET", "/me", Some(&old_session), None).await.0,
        StatusCode::UNAUTHORIZED
    );
    // O refresh token só vale uma vez.
    let (reuse, _) = app
        .send(
            "POST",
            "/auth/refresh",
            None,
            Some(json!({ "refresh_token": old_refresh })),
        )
        .await;
    assert_eq!(reuse, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn refresh_with_unknown_or_empty_token_is_rejected() {
    let app = TestApp::new().await;

    let unknown = app
        .send(
            "POST",
            "/auth/refresh",
            None,
            Some(json!({ "refresh_token": "abc" })),
        )
        .await;
    let empty = app
        .send(
            "POST",
            "/auth/refresh",
            None,
            Some(json!({ "refresh_token": "" })),
        )
        .await;

    assert_eq!(unknown.0, StatusCode::UNAUTHORIZED);
    assert_eq!(empty.0, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_refreshes_with_the_same_token_succeed_only_once() {
    let app = TestApp::new().await;
    app.sign_up("ana@test.dev").await;
    let (_, _, refresh) = app.sign_in("ana@test.dev", PASSWORD).await;

    let tasks: Vec<_> = (0..5)
        .map(|_| {
            let router = app.router.clone();
            let refresh = refresh.clone();
            tokio::spawn(async move {
                send(
                    &router,
                    "POST",
                    "/auth/refresh",
                    None,
                    Some(json!({ "refresh_token": refresh })),
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

    let ok = statuses.iter().filter(|s| **s == StatusCode::OK).count();
    let unauthorized = statuses
        .iter()
        .filter(|s| **s == StatusCode::UNAUTHORIZED)
        .count();
    assert_eq!(ok, 1, "{statuses:?}");
    assert_eq!(unauthorized, 4, "{statuses:?}");
}

#[tokio::test]
async fn sign_out_kills_the_whole_session() {
    let app = TestApp::new().await;
    app.sign_up("ana@test.dev").await;
    let (_, session, refresh) = app.sign_in("ana@test.dev", PASSWORD).await;

    let (status, _) = app
        .send(
            "POST",
            "/auth/sign-out",
            None,
            Some(json!({ "refresh_token": refresh })),
        )
        .await;

    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        app.send("GET", "/me", Some(&session), None).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (again, _) = app
        .send(
            "POST",
            "/auth/refresh",
            None,
            Some(json!({ "refresh_token": refresh })),
        )
        .await;
    assert_eq!(again, StatusCode::UNAUTHORIZED);
}
