mod common;

use axum::http::{Method, StatusCode, header};
use serde_json::json;

use common::{TEST_PASSWORD, TestServer};

#[tokio::test]
async fn health_is_public_and_reports_ok() {
    let server = TestServer::start().await;
    let response = server.call(Method::GET, "/api/health", None, None).await;
    assert_eq!(response.status, StatusCode::OK);
    let body = response.json();
    assert_eq!(body["status"], "ok");
    assert!(body["version"].is_string());
}

#[tokio::test]
async fn static_shell_is_served_with_security_headers() {
    let server = TestServer::start().await;
    let response = server.call(Method::GET, "/", None, None).await;
    assert_eq!(response.status, StatusCode::OK);
    assert!(response.text().contains("Daybook"));
    assert_eq!(
        response.headers.get("x-content-type-options").unwrap(),
        "nosniff"
    );
    assert!(response.headers.contains_key(header::CONTENT_SECURITY_POLICY));

    let missing = server.call(Method::GET, "/nope.js", None, None).await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);

    let traversal = server.call(Method::GET, "/../Cargo.toml", None, None).await;
    assert_eq!(traversal.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn protected_endpoints_require_a_session() {
    let server = TestServer::start().await;
    let response = server.call(Method::GET, "/api/employees", None, None).await;
    assert_eq!(response.status, StatusCode::UNAUTHORIZED);
    assert_eq!(response.error_code().as_deref(), Some("unauthorized"));
}

#[tokio::test]
async fn login_me_logout_round_trip() {
    let server = TestServer::start().await;

    let me = server.call(Method::GET, "/api/auth/me", None, None).await;
    assert_eq!(me.status, StatusCode::OK);
    assert_eq!(me.json()["authenticated"], false);

    let cookie = server.login().await;
    let me = server
        .call(Method::GET, "/api/auth/me", None, Some(&cookie))
        .await;
    assert_eq!(me.json()["authenticated"], true);

    let logout = server
        .call(Method::POST, "/api/auth/logout", None, Some(&cookie))
        .await;
    assert_eq!(logout.status, StatusCode::OK);
    let cleared = logout
        .headers
        .get(header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert!(cleared.contains("Max-Age=0"));

    let after = server.call(Method::GET, "/api/employees", None, None).await;
    assert_eq!(after.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn wrong_password_is_rejected_and_throttled() {
    let server = TestServer::start().await;
    for _ in 0..5 {
        let response = server
            .call(
                Method::POST,
                "/api/auth/login",
                Some(json!({ "password": TEST_PASSWORD.replace("123", "999") })),
                None,
            )
            .await;
        assert_eq!(response.status, StatusCode::UNAUTHORIZED);
    }

    // Blocked even with the correct password until the window passes.
    let blocked = server
        .call(
            Method::POST,
            "/api/auth/login",
            Some(json!({ "password": TEST_PASSWORD })),
            None,
        )
        .await;
    assert_eq!(blocked.status, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn mutating_api_calls_require_the_client_header() {
    let server = TestServer::start().await;
    let cookie = server.login().await;
    let response = server
        .call_without_client_header(
            Method::POST,
            "/api/employees",
            Some(json!({ "name": "Asha", "pay_basis": "day_rate", "rate_minor": 80000 })),
            Some(&cookie),
        )
        .await;
    assert_eq!(response.status, StatusCode::FORBIDDEN);
    assert_eq!(response.error_code().as_deref(), Some("forbidden"));
}

#[tokio::test]
async fn malformed_json_returns_our_error_envelope() {
    let server = TestServer::start().await;
    let cookie = server.login().await;
    let request = axum::http::Request::builder()
        .method(Method::POST)
        .uri("/api/employees")
        .header("x-daybook-client", "test")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .body(axum::body::Body::from("{not json"))
        .unwrap();
    let response = tower::ServiceExt::oneshot(server.app.clone(), request)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn expired_sessions_are_rejected() {
    let server = TestServer::start().await;
    let cookie = server.login().await;

    sqlx::query("UPDATE sessions SET expires_at = 0")
        .execute(&server.state.pool)
        .await
        .expect("expire sessions");

    let response = server
        .call(Method::GET, "/api/employees", None, Some(&cookie))
        .await;
    assert_eq!(response.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn admin_password_reset_revokes_sessions() {
    let server = TestServer::start().await;
    let cookie = server.login().await;

    daybook::db::set_admin_password(&server.state.pool, "another-password-456")
        .await
        .expect("reset password");

    let response = server
        .call(Method::GET, "/api/employees", None, Some(&cookie))
        .await;
    assert_eq!(response.status, StatusCode::UNAUTHORIZED);

    let login = server
        .call(
            Method::POST,
            "/api/auth/login",
            Some(json!({ "password": "another-password-456" })),
            None,
        )
        .await;
    assert_eq!(login.status, StatusCode::OK);
}
