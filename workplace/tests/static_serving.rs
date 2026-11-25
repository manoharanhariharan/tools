mod common;

use axum::http::{Method, StatusCode, header};

use common::TestServer;

#[tokio::test]
async fn embedded_static_assets_are_served_with_correct_types() {
    let server = TestServer::start().await;

    let index = server.call(Method::GET, "/", None, None).await;
    assert_eq!(index.status, StatusCode::OK);
    assert!(
        index
            .headers
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .starts_with("text/html")
    );
    assert!(index.text().contains("manifest.webmanifest"));

    let manifest = server
        .call(Method::GET, "/manifest.webmanifest", None, None)
        .await;
    assert_eq!(manifest.status, StatusCode::OK);
    assert_eq!(
        manifest.headers.get(header::CONTENT_TYPE).unwrap(),
        "application/manifest+json"
    );
    assert!(manifest.text().contains("maskable"));

    let worker = server.call(Method::GET, "/sw.js", None, None).await;
    assert_eq!(worker.status, StatusCode::OK);
    assert_eq!(
        worker.headers.get(header::CONTENT_TYPE).unwrap(),
        "text/javascript; charset=utf-8"
    );
    assert_eq!(
        worker.headers.get(header::CACHE_CONTROL).unwrap(),
        "no-cache"
    );
    assert!(worker.text().contains("daybook-v4"));

    // Guards against a UI regression in the shipped view module.
    let today = server
        .call(Method::GET, "/js/views/today.js", None, None)
        .await;
    assert_eq!(today.status, StatusCode::OK);
    assert!(today.text().contains("openEmployeeDialog(ctx, {})"));
    assert!(today.text().contains("Add business"));

    let missing = server.call(Method::GET, "/js/nope.js", None, None).await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn static_assets_do_not_require_a_session() {
    let server = TestServer::start().await;
    for path in ["/css/app.css", "/icons/icon-192.png", "/js/app.js"] {
        let response = server.call(Method::GET, path, None, None).await;
        assert_eq!(response.status, StatusCode::OK, "{path}");
    }
}
