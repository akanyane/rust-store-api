use axum::http::StatusCode;

use super::{PASSWORD, TestApp};
use crate::services::auth as auth_service;

#[tokio::test]
async fn purge_removes_only_sessions_whose_refresh_token_expired() {
    let app = TestApp::new().await;
    app.sign_up("ana@test.dev").await;
    let (_, session, _) = app.sign_in("ana@test.dev", PASSWORD).await;

    // x1: refresh vencido (deve sumir). x3: só o session token venceu, o refresh
    // ainda vale (deve ficar, porque ainda dá para renovar).
    app.db
        .query(
            "LET $u = (SELECT VALUE id FROM user WHERE role = 'customer' LIMIT 1)[0];
             CREATE session SET user = $u, token_hash = 'x1', refresh_hash = 'x2',
                 expires_at = time::now() - 2d, refresh_expires_at = time::now() - 1d;
             CREATE session SET user = $u, token_hash = 'x3', refresh_hash = 'x4',
                 expires_at = time::now() - 1h, refresh_expires_at = time::now() + 5d;",
        )
        .await
        .unwrap()
        .check()
        .unwrap();

    let purged = auth_service::purge_expired_sessions(&app.db).await.unwrap();

    assert_eq!(purged, 1);
    let mut result = app
        .db
        .query("SELECT VALUE token_hash FROM session")
        .await
        .unwrap();
    let hashes: Vec<String> = result.take(0).unwrap();
    assert_eq!(hashes.len(), 2);
    assert!(hashes.iter().any(|h| h == "x3"));
    assert!(!hashes.iter().any(|h| h == "x1"));
    assert_eq!(
        app.send("GET", "/me", Some(&session), None).await.0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn purge_with_nothing_expired_removes_nothing() {
    let app = TestApp::new().await;
    app.sign_up("ana@test.dev").await;
    app.sign_in("ana@test.dev", PASSWORD).await;

    assert_eq!(
        auth_service::purge_expired_sessions(&app.db).await.unwrap(),
        0
    );
}
