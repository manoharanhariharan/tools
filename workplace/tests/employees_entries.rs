mod common;

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use common::TestServer;

async fn create_entry(
    server: &TestServer,
    cookie: &str,
    body: Value,
) -> common::TestResponse {
    server
        .call(Method::POST, "/api/entries", Some(body), Some(cookie))
        .await
}

#[tokio::test]
async fn employee_lifecycle_and_validation() {
    let server = TestServer::start().await;
    let cookie = server.login().await;

    let created = server
        .call(
            Method::POST,
            "/api/employees",
            Some(json!({
                "name": "  Asha Kumari ",
                "code": "EMP-01",
                "pay_basis": "day_rate",
                "rate_minor": 80000,
                "ot_multiplier_bp": 17500,
                "standard_minutes_per_day": 450
            })),
            Some(&cookie),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.text());
    let employee = created.json();
    assert_eq!(employee["name"], "Asha Kumari");
    assert_eq!(employee["code"], "EMP-01");
    assert_eq!(employee["active"], true);
    assert_eq!(employee["deleted_at"], Value::Null);
    let id = employee["id"].as_str().unwrap().to_string();

    let list = server
        .call(Method::GET, "/api/employees", None, Some(&cookie))
        .await;
    assert_eq!(list.status, StatusCode::OK);
    assert_eq!(list.json().as_array().unwrap().len(), 1);

    let fetched = server
        .call(Method::GET, &format!("/api/employees/{id}"), None, Some(&cookie))
        .await;
    assert_eq!(fetched.status, StatusCode::OK);
    assert_eq!(fetched.json()["name"], "Asha Kumari");

    let updated = server
        .call(
            Method::PATCH,
            &format!("/api/employees/{id}"),
            Some(json!({
                "name": "Asha K.",
                "code": "EMP-01",
                "pay_basis": "hourly",
                "rate_minor": 15000,
                "active": false
            })),
            Some(&cookie),
        )
        .await;
    assert_eq!(updated.status, StatusCode::OK, "{}", updated.text());
    assert_eq!(updated.json()["pay_basis"], "hourly");
    assert_eq!(updated.json()["rate_minor"], 15000);
    assert_eq!(updated.json()["active"], false);

    // Reusing an active code is rejected with the conflicting record.
    let second = server
        .call(
            Method::POST,
            "/api/employees",
            Some(json!({ "name": "Other", "code": "EMP-01", "pay_basis": "fixed", "rate_minor": 50000 })),
            Some(&cookie),
        )
        .await;
    assert_eq!(second.status, StatusCode::CONFLICT);
    assert_eq!(second.error_code().as_deref(), Some("duplicate_code"));
    assert_eq!(second.json()["error"]["existing"]["id"], id);

    // Because the first employee is inactive, a second can reuse the code only
    // after the original is soft-deleted.
    let deleted = server
        .call(
            Method::DELETE,
            &format!("/api/employees/{id}"),
            None,
            Some(&cookie),
        )
        .await;
    assert_eq!(deleted.status, StatusCode::OK);
    assert!(deleted.json()["deleted_at"].is_i64());

    let list = server
        .call(Method::GET, "/api/employees", None, Some(&cookie))
        .await;
    assert!(list.json().as_array().unwrap().is_empty());

    let gone = server
        .call(Method::GET, &format!("/api/employees/{id}"), None, Some(&cookie))
        .await;
    assert_eq!(gone.status, StatusCode::NOT_FOUND);

    let invalid = server
        .call(
            Method::POST,
            "/api/employees",
            Some(json!({ "name": "", "pay_basis": "weekly", "rate_minor": -5 })),
            Some(&cookie),
        )
        .await;
    assert_eq!(invalid.status, StatusCode::BAD_REQUEST);
    let fields: Vec<String> = invalid.json()["error"]["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["field"].as_str().unwrap().to_string())
        .collect();
    assert!(fields.contains(&"name".to_string()));
    assert!(fields.contains(&"pay_basis".to_string()));
    assert!(fields.contains(&"rate_minor".to_string()));
}

#[tokio::test]
async fn day_rate_entry_computes_and_snapshots_amounts() {
    let server = TestServer::start().await;
    let (cookie, employee) = server.employee("Asha", "day_rate", 80_000).await;
    let employee_id = employee["id"].as_str().unwrap();

    let response = create_entry(
        &server,
        &cookie,
        json!({
            "employee_id": employee_id,
            "work_date": "2026-11-20",
            "bonus_minor": 10000,
            "advance_minor": 20000,
            "deduction_minor": 5000,
            "note": "Diwali bonus"
        }),
    )
    .await;
    assert_eq!(response.status, StatusCode::CREATED, "{}", response.text());
    let entry = response.json();
    assert_eq!(entry["base_minor"], 80_000);
    assert_eq!(entry["ot_minor"], 0);
    assert_eq!(entry["gross_minor"], 90_000);
    assert_eq!(entry["net_minor"], 65_000);
    assert_eq!(entry["employee_name"], "Asha");
    assert_eq!(entry["note"], "Diwali bonus");

    // Duplicate employee+date is a conflict that carries the existing row.
    let duplicate = create_entry(
        &server,
        &cookie,
        json!({ "employee_id": employee_id, "work_date": "2026-11-20" }),
    )
    .await;
    assert_eq!(duplicate.status, StatusCode::CONFLICT);
    assert_eq!(
        duplicate.error_code().as_deref(),
        Some("duplicate_entry_date")
    );
    assert_eq!(duplicate.json()["error"]["existing"]["id"], entry["id"]);

    // Rate changes do not rewrite history.
    server
        .call(
            Method::PATCH,
            &format!("/api/employees/{employee_id}"),
            Some(json!({
                "name": "Asha", "pay_basis": "day_rate", "rate_minor": 100000
            })),
            Some(&cookie),
        )
        .await;

    let listed = server
        .call(
            Method::GET,
            "/api/entries?from=2026-11-01&to=2026-11-30",
            None,
            Some(&cookie),
        )
        .await;
    assert_eq!(listed.status, StatusCode::OK);
    let rows = listed.json();
    assert_eq!(rows.as_array().unwrap().len(), 1);
    assert_eq!(rows[0]["base_minor"], 80_000);
}

#[tokio::test]
async fn hourly_and_overtime_entries_are_computed_server_side() {
    let server = TestServer::start().await;
    let (cookie, employee) = server.employee("Ravi", "hourly", 15_000).await;
    let employee_id = employee["id"].as_str().unwrap();

    let response = create_entry(
        &server,
        &cookie,
        json!({
            "employee_id": employee_id,
            "work_date": "2026-11-21",
            "regular_minutes": 450,
            "overtime_minutes": 90
        }),
    )
    .await;
    assert_eq!(response.status, StatusCode::CREATED, "{}", response.text());
    let entry = response.json();
    assert_eq!(entry["base_minor"], 112_500);
    assert_eq!(entry["ot_minor"], 33_750);
    assert_eq!(entry["gross_minor"], 146_250);

    // Client-supplied money fields are ignored: the server recomputes.
    let tampered = create_entry(
        &server,
        &cookie,
        json!({
            "employee_id": employee_id,
            "work_date": "2026-11-22",
            "regular_minutes": 60,
            "base_minor": 1,
            "net_minor": 1
        }),
    )
    .await;
    assert_eq!(tampered.status, StatusCode::CREATED);
    assert_eq!(tampered.json()["base_minor"], 15_000);
    assert_eq!(tampered.json()["net_minor"], 15_000);
}

#[tokio::test]
async fn entry_update_recomputes_and_delete_is_soft() {
    let server = TestServer::start().await;
    let (cookie, employee) = server.employee("Meena", "day_rate", 70_000).await;
    let employee_id = employee["id"].as_str().unwrap();

    let created = create_entry(
        &server,
        &cookie,
        json!({ "employee_id": employee_id, "work_date": "2026-11-10" }),
    )
    .await
    .json();
    let entry_id = created["id"].as_str().unwrap().to_string();
    let created_seq = created["seq"].as_i64().unwrap();

    let updated = server
        .call(
            Method::PATCH,
            &format!("/api/entries/{entry_id}"),
            Some(json!({
                "employee_id": employee_id,
                "work_date": "2026-11-10",
                "day_fraction_bp": 5000,
                "overtime_minutes": 60
            })),
            Some(&cookie),
        )
        .await;
    assert_eq!(updated.status, StatusCode::OK, "{}", updated.text());
    let entry = updated.json();
    // Half day = 350.00; OT = 700 * (60/480) * 1.5 = 131.25
    assert_eq!(entry["base_minor"], 35_000);
    assert_eq!(entry["ot_minor"], 13_125);
    assert!(entry["seq"].as_i64().unwrap() > created_seq);

    let deleted = server
        .call(
            Method::DELETE,
            &format!("/api/entries/{entry_id}"),
            None,
            Some(&cookie),
        )
        .await;
    assert_eq!(deleted.status, StatusCode::OK);
    assert!(deleted.json()["deleted_at"].is_i64());

    let listed = server
        .call(
            Method::GET,
            "/api/entries?from=2026-11-01&to=2026-11-30",
            None,
            Some(&cookie),
        )
        .await;
    assert!(listed.json().as_array().unwrap().is_empty());

    let gone = server
        .call(Method::GET, &format!("/api/entries/{entry_id}"), None, Some(&cookie))
        .await;
    assert_eq!(gone.status, StatusCode::NOT_FOUND);

    // The tombstone is retained in the database for sync.
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM entries WHERE id = ?")
        .bind(&entry_id)
        .fetch_one(&server.state.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);

    let deleted_again = server
        .call(
            Method::DELETE,
            &format!("/api/entries/{entry_id}"),
            None,
            Some(&cookie),
        )
        .await;
    assert_eq!(deleted_again.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn entries_require_an_existing_live_employee_and_valid_ranges() {
    let server = TestServer::start().await;
    let cookie = server.login().await;

    let missing = create_entry(
        &server,
        &cookie,
        json!({
            "employee_id": "0190f3c4-0000-7000-8000-000000000000",
            "work_date": "2026-11-10"
        }),
    )
    .await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);

    let bad_range = server
        .call(
            Method::GET,
            "/api/entries?from=2026-11-30&to=2026-11-01",
            None,
            Some(&cookie),
        )
        .await;
    assert_eq!(bad_range.status, StatusCode::BAD_REQUEST);

    let partial_range = server
        .call(Method::GET, "/api/entries?from=2026-11-01", None, Some(&cookie))
        .await;
    assert_eq!(partial_range.status, StatusCode::BAD_REQUEST);

    // Default range = current IST month, which always parses.
    let default_range = server
        .call(Method::GET, "/api/entries", None, Some(&cookie))
        .await;
    assert_eq!(default_range.status, StatusCode::OK);
}

#[tokio::test]
async fn deleting_an_employee_keeps_historical_entries() {
    let server = TestServer::start().await;
    let (cookie, employee) = server.employee("Sita", "fixed", 60_000).await;
    let employee_id = employee["id"].as_str().unwrap().to_string();

    let entry = create_entry(
        &server,
        &cookie,
        json!({ "employee_id": employee_id, "work_date": "2026-11-05" }),
    )
    .await;
    assert_eq!(entry.status, StatusCode::CREATED);
    assert_eq!(entry.json()["base_minor"], 60_000);

    server
        .call(
            Method::DELETE,
            &format!("/api/employees/{employee_id}"),
            None,
            Some(&cookie),
        )
        .await;

    let listed = server
        .call(
            Method::GET,
            &format!("/api/entries?from=2026-11-01&to=2026-11-30&employee_id={employee_id}"),
            None,
            Some(&cookie),
        )
        .await;
    assert_eq!(listed.status, StatusCode::OK);
    assert_eq!(listed.json().as_array().unwrap().len(), 1);

    let new_entry = create_entry(
        &server,
        &cookie,
        json!({ "employee_id": employee_id, "work_date": "2026-11-06" }),
    )
    .await;
    assert_eq!(new_entry.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn sequence_is_strictly_increasing_across_resource_types() {
    let server = TestServer::start().await;
    let (cookie, employee) = server.employee("Kiran", "fixed", 50_000).await;
    let employee_id = employee["id"].as_str().unwrap();

    let mut seqs = vec![employee["seq"].as_i64().unwrap()];
    let entry = create_entry(
        &server,
        &cookie,
        json!({ "employee_id": employee_id, "work_date": "2026-11-07" }),
    )
    .await
    .json();
    seqs.push(entry["seq"].as_i64().unwrap());

    let updated = server
        .call(
            Method::PATCH,
            &format!("/api/employees/{employee_id}"),
            Some(json!({ "name": "Kiran R", "pay_basis": "fixed", "rate_minor": 55000 })),
            Some(&cookie),
        )
        .await
        .json();
    seqs.push(updated["seq"].as_i64().unwrap());

    let deleted = server
        .call(
            Method::DELETE,
            &format!("/api/entries/{}", entry["id"].as_str().unwrap()),
            None,
            Some(&cookie),
        )
        .await
        .json();
    seqs.push(deleted["seq"].as_i64().unwrap());

    for pair in seqs.windows(2) {
        assert!(pair[1] > pair[0], "sequence regressed: {seqs:?}");
    }
}
