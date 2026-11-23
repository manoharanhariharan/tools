mod common;

use axum::http::{Method, StatusCode};
use serde_json::json;

use common::TestServer;

#[tokio::test]
async fn earning_types_are_listed_created_renamed_and_soft_deleted() {
    let server = TestServer::start().await;
    let cookie = server.login().await;

    let listed = server
        .call(Method::GET, "/api/earning-types", None, Some(&cookie))
        .await;
    assert_eq!(listed.status, StatusCode::OK);
    let names: Vec<String> = listed
        .json()
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["name"].as_str().unwrap().to_string())
        .collect();
    assert!(names.contains(&"Sales".to_string()));
    assert!(names.contains(&"Service".to_string()));
    assert!(names.contains(&"Tips".to_string()));

    let created = server
        .call(
            Method::POST,
            "/api/earning-types",
            Some(json!({ "name": "Delivery", "active": true })),
            Some(&cookie),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.text());
    let id = created.json()["id"].as_str().unwrap().to_string();

    let duplicate = server
        .call(
            Method::POST,
            "/api/earning-types",
            Some(json!({ "name": "delivery" })),
            Some(&cookie),
        )
        .await;
    assert_eq!(duplicate.status, StatusCode::CONFLICT);
    assert_eq!(
        duplicate.error_code().as_deref(),
        Some("duplicate_type_name")
    );
    assert_eq!(duplicate.json()["error"]["existing"]["name"], "Delivery");

    let renamed = server
        .call(
            Method::PATCH,
            &format!("/api/earning-types/{id}"),
            Some(json!({ "name": "Home delivery", "active": false })),
            Some(&cookie),
        )
        .await;
    assert_eq!(renamed.status, StatusCode::OK);
    assert_eq!(renamed.json()["name"], "Home delivery");
    assert_eq!(renamed.json()["active"], false);

    let deleted = server
        .call(
            Method::DELETE,
            &format!("/api/earning-types/{id}"),
            None,
            Some(&cookie),
        )
        .await;
    assert_eq!(deleted.status, StatusCode::OK);
    assert!(deleted.json()["deleted_at"].is_i64());

    let after = server
        .call(Method::GET, "/api/earning-types", None, Some(&cookie))
        .await
        .json();
    assert!(
        after.as_array().unwrap().iter().all(|row| row["id"] != id),
        "deleted type should disappear from the live list"
    );

    let again = server
        .call(
            Method::DELETE,
            &format!("/api/earning-types/{id}"),
            None,
            Some(&cookie),
        )
        .await;
    assert_eq!(again.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn earning_type_validation_and_auth() {
    let server = TestServer::start().await;

    let unauth = server
        .call(Method::GET, "/api/earning-types", None, None)
        .await;
    assert_eq!(unauth.status, StatusCode::UNAUTHORIZED);

    let cookie = server.login().await;
    let invalid = server
        .call(
            Method::POST,
            "/api/earning-types",
            Some(json!({ "name": "   " })),
            Some(&cookie),
        )
        .await;
    assert_eq!(invalid.status, StatusCode::BAD_REQUEST);
    assert_eq!(invalid.json()["error"]["fields"][0]["field"], "name");

    let too_long = server
        .call(
            Method::POST,
            "/api/earning-types",
            Some(json!({ "name": "x".repeat(61) })),
            Some(&cookie),
        )
        .await;
    assert_eq!(too_long.status, StatusCode::BAD_REQUEST);
}
