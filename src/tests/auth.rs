use axum::http::StatusCode;
use serde_json::json;

use super::{PASSWORD, TestApp, send};

#[tokio::test]
async fn sign_up_creates_customer_and_lowercases_the_username() {
    let app = TestApp::new().await;

    let (status, body) = app.sign_up("Ana.Silva_1").await;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["username"], "ana.silva_1");
    assert!(body.get("email").is_none());
}

#[tokio::test]
async fn usernames_are_unique_regardless_of_case() {
    let app = TestApp::new().await;
    app.sign_up("ana").await;

    let (status, body) = app.sign_up("ANA").await;

    assert_eq!(status, StatusCode::CONFLICT);
    let message = body["error"].as_str().unwrap();
    assert!(message.contains("username"), "{message}");
    assert!(!message.contains("ANA"), "{message}");
}

#[tokio::test]
async fn sign_in_ignores_the_case_of_the_username() {
    let app = TestApp::new().await;
    app.sign_up("Ana").await;

    let (status, session, _) = app.sign_in("ANA", PASSWORD).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        app.send("GET", "/me", Some(&session), None).await.1["username"],
        "ana"
    );
}

#[tokio::test]
async fn invalid_usernames_are_rejected() {
    let app = TestApp::new().await;
    let too_long = "a".repeat(33);

    for bad in [
        "ab",
        "",
        &too_long,
        "ana smith",
        "ana@test.dev",
        "ana!",
        "ána",
    ] {
        let (status, body) = app.sign_up(bad).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "username {bad:?}");
        let message = body["error"].as_str().unwrap();
        assert!(
            message.contains("username: must be 3 to 32 characters"),
            "{message}"
        );
    }

    for good in ["abc", &"a".repeat(32), "ana.smith-1_x"] {
        let (status, _) = app.sign_up(good).await;
        assert_eq!(status, StatusCode::CREATED, "username {good:?}");
    }
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
                "username": "not valid!",
                "password": PASSWORD,
                "first_name": "  ",
                "last_name": "User",
                "birthday": "2999-01-01",
            })),
        )
        .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let message = body["error"].as_str().unwrap();
    assert!(
        message.contains("username: must be 3 to 32 characters"),
        "{message}"
    );
    assert!(
        message.contains("first_name: must not be blank"),
        "{message}"
    );
    assert!(
        message.contains("birthday: must not be in the future"),
        "{message}"
    );
    assert!(!message.contains("not valid!"), "{message}");
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
            Some(json!({ "username": "ana" })),
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
                "username": "ana",
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
    app.sign_up("ana").await;

    let wrong_password = app
        .send(
            "POST",
            "/auth/sign-in",
            None,
            Some(json!({ "username": "ana", "password": "wrong-password" })),
        )
        .await;
    let unknown_username = app
        .send(
            "POST",
            "/auth/sign-in",
            None,
            Some(json!({ "username": "nobody", "password": PASSWORD })),
        )
        .await;

    assert_eq!(wrong_password.0, StatusCode::UNAUTHORIZED);
    assert_eq!(wrong_password, unknown_username);
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
    let customer = app.customer("ana").await;

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
    app.sign_up("ana").await;
    let (_, old_session, old_refresh) = app.sign_in("ana", PASSWORD).await;

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
    app.sign_up("ana").await;
    let (_, _, refresh) = app.sign_in("ana", PASSWORD).await;

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
    app.sign_up("ana").await;
    let (_, session, refresh) = app.sign_in("ana", PASSWORD).await;

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

#[tokio::test]
async fn session_reports_who_is_logged_in_and_with_which_role() {
    let app = TestApp::new().await;
    let customer = app.customer("ana").await;
    let admin = app.admin().await;

    let (status, body) = app
        .send("GET", "/auth/session", Some(&customer), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["username"], "ana");
    assert_eq!(body["role"], "customer");
    // O id é o do usuário, o mesmo do perfil.
    let (_, me) = app.send("GET", "/me", Some(&customer), None).await;
    assert_eq!(body["id"], me["id"]);

    // O admin não tem perfil de cliente (`/me` dá 404), mas a sessão diz quem ele é.
    let (status, body) = app.send("GET", "/auth/session", Some(&admin), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["username"], "admin");
    assert_eq!(body["role"], "admin");
    assert!(body["id"].as_str().is_some_and(|id| !id.is_empty()));
    assert_eq!(
        app.send("GET", "/me", Some(&admin), None).await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn session_exposes_only_id_username_and_role() {
    let app = TestApp::new().await;
    let customer = app.customer("ana").await;

    let (_, body) = app
        .send("GET", "/auth/session", Some(&customer), None)
        .await;

    let mut keys: Vec<&str> = body
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort();
    assert_eq!(keys, ["id", "role", "username"]);
}

#[tokio::test]
async fn session_requires_a_valid_token() {
    let app = TestApp::new().await;
    app.customer("ana").await;

    for token in [None, Some("garbage"), Some("")] {
        let (status, _) = app.send("GET", "/auth/session", token, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{token:?}");
    }
}

#[tokio::test]
async fn session_rejects_an_expired_session_and_a_signed_out_one() {
    use sha2::{Digest, Sha256};

    let app = TestApp::new().await;
    let (_, session, refresh) = {
        app.sign_up("ana").await;
        app.sign_in("ana", PASSWORD).await
    };
    assert_eq!(
        app.send("GET", "/auth/session", Some(&session), None)
            .await
            .0,
        StatusCode::OK
    );

    // Uma sessão cujo session token já venceu (o refresh ainda valeria).
    let hash: String = Sha256::digest(b"expired-token")
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    app.db
        .query(format!(
            "LET $u = (SELECT VALUE id FROM user WHERE username = 'ana')[0];
             CREATE session SET user = $u, token_hash = '{hash}', refresh_hash = 'r-expired',
                 expires_at = time::now() - 1h, refresh_expires_at = time::now() + 1d;"
        ))
        .await
        .unwrap()
        .check()
        .unwrap();
    assert_eq!(
        app.send("GET", "/auth/session", Some("expired-token"), None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );

    // Depois do sign-out, o session token também deixa de valer.
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
        app.send("GET", "/auth/session", Some(&session), None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}
