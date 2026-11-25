mod common;

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use common::{SALES_TYPE, TestServer, lines};

async fn entry(server: &TestServer, cookie: &str, body: Value) -> Value {
    let response = server
        .call(Method::POST, "/api/entries", Some(body), Some(cookie))
        .await;
    assert_eq!(response.status, StatusCode::CREATED, "{}", response.text());
    response.json()
}

async fn audit(server: &TestServer, cookie: &str) -> Vec<Value> {
    let response = server
        .call(Method::GET, "/api/audit?limit=500", None, Some(cookie))
        .await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.text());
    response.json().as_array().unwrap().clone()
}

#[tokio::test]
async fn audit_trail_records_every_data_change_and_is_admin_only() {
    let server = TestServer::start().await;
    let (admin, employee) = server.employee_with_code("Asha", Some("A1")).await;
    let id = employee["id"].as_str().unwrap().to_string();

    // PIN set, then employee tries (and is denied) reading the audit log.
    let set_pin = server
        .call(
            Method::PUT,
            &format!("/api/employees/{id}/pin"),
            Some(json!({ "pin": "1234" })),
            Some(&admin),
        )
        .await;
    assert_eq!(set_pin.status, StatusCode::OK);

    let employee_login = server
        .call(
            Method::POST,
            "/api/auth/login/employee",
            Some(json!({ "code": "A1", "pin": "1234" })),
            None,
        )
        .await;
    let employee_cookie = employee_login
        .headers
        .get(axum::http::header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .map(str::to_string)
        .unwrap();
    let denied = server
        .call(Method::GET, "/api/audit", None, Some(&employee_cookie))
        .await;
    assert_eq!(denied.status, StatusCode::FORBIDDEN);

    // Entry create -> update -> delete.
    let created = entry(
        &server,
        &admin,
        json!({
            "employee_id": id,
            "work_date": "2026-11-01",
            "lines": lines(&[(SALES_TYPE, "", 40_000)])
        }),
    )
    .await;
    let entry_id = created["id"].as_str().unwrap().to_string();
    let updated = server
        .call(
            Method::PATCH,
            &format!("/api/entries/{entry_id}"),
            Some(json!({
                "employee_id": id,
                "work_date": "2026-11-01",
                "lines": lines(&[(SALES_TYPE, "", 50_000)])
            })),
            Some(&admin),
        )
        .await;
    assert_eq!(updated.status, StatusCode::OK);
    let deleted = server
        .call(
            Method::DELETE,
            &format!("/api/entries/{entry_id}"),
            None,
            Some(&admin),
        )
        .await;
    assert_eq!(deleted.status, StatusCode::OK);

    // Earning type create -> update -> delete.
    let created_type = server
        .call(
            Method::POST,
            "/api/earning-types",
            Some(json!({ "name": "AuditType" })),
            Some(&admin),
        )
        .await;
    assert_eq!(created_type.status, StatusCode::CREATED);
    let type_id = created_type.json()["id"].as_str().unwrap().to_string();
    let updated_type = server
        .call(
            Method::PATCH,
            &format!("/api/earning-types/{type_id}"),
            Some(json!({ "name": "AuditTypeRenamed" })),
            Some(&admin),
        )
        .await;
    assert_eq!(updated_type.status, StatusCode::OK);
    let deleted_type = server
        .call(
            Method::DELETE,
            &format!("/api/earning-types/{type_id}"),
            None,
            Some(&admin),
        )
        .await;
    assert_eq!(deleted_type.status, StatusCode::OK);

    // Clear PIN.
    let cleared = server
        .call(
            Method::DELETE,
            &format!("/api/employees/{id}/pin"),
            None,
            Some(&admin),
        )
        .await;
    assert_eq!(cleared.status, StatusCode::OK);

    let rows = audit(&server, &admin).await;
    assert!(!rows.is_empty());

    // Newest first.
    assert!(
        rows.windows(2)
            .all(|pair| pair[0]["id"].as_i64().unwrap() > pair[1]["id"].as_i64().unwrap())
    );

    fn has(rows: &[Value], entity: &str, action: &str) -> bool {
        rows.iter()
            .any(|row| row["entity"] == entity && row["action"] == action)
    }

    assert!(has(&rows, "employee", "create"));
    assert!(has(&rows, "employee_pin", "set"));
    assert!(has(&rows, "employee_pin", "clear"));
    assert!(has(&rows, "entry", "create"));
    assert!(has(&rows, "entry", "update"));
    assert!(has(&rows, "entry", "delete"));
    assert!(has(&rows, "earning_type", "create"));
    assert!(has(&rows, "earning_type", "update"));
    assert!(has(&rows, "earning_type", "delete"));

    // Actor and summary are populated.
    let sample = rows.first().unwrap();
    assert_eq!(sample["actor_role"], "admin");
    assert!(!sample["summary"].as_str().unwrap().is_empty());
}
