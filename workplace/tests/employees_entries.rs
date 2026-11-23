mod common;

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use common::{SALES_TYPE, SERVICE_TYPE, TIPS_TYPE, TestServer, lines};

async fn create_entry(server: &TestServer, cookie: &str, body: Value) -> common::TestResponse {
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
                "note": "joined 2024"
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
    assert!(employee.get("rate_minor").is_none());
    assert!(employee.get("pay_basis").is_none());
    let id = employee["id"].as_str().unwrap().to_string();

    let list = server
        .call(Method::GET, "/api/employees", None, Some(&cookie))
        .await;
    assert_eq!(list.status, StatusCode::OK);
    assert_eq!(list.json().as_array().unwrap().len(), 1);

    let updated = server
        .call(
            Method::PATCH,
            &format!("/api/employees/{id}"),
            Some(json!({
                "name": "Asha K.",
                "code": "EMP-01",
                "note": "team lead",
                "active": false
            })),
            Some(&cookie),
        )
        .await;
    assert_eq!(updated.status, StatusCode::OK, "{}", updated.text());
    assert_eq!(updated.json()["name"], "Asha K.");
    assert_eq!(updated.json()["active"], false);

    let invalid = server
        .call(
            Method::POST,
            "/api/employees",
            Some(json!({ "name": "", "code": "BAD CODE" })),
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
    assert!(fields.contains(&"code".to_string()));

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

    let gone = server
        .call(
            Method::GET,
            &format!("/api/employees/{id}"),
            None,
            Some(&cookie),
        )
        .await;
    assert_eq!(gone.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn employee_codes_are_case_insensitive() {
    let server = TestServer::start().await;
    let (cookie, _first) = server.employee_with_code("First", Some("EMP-1")).await;

    let second = server
        .call(
            Method::POST,
            "/api/employees",
            Some(json!({ "name": "Second", "code": "emp-1" })),
            Some(&cookie),
        )
        .await;
    assert_eq!(second.status, StatusCode::CONFLICT);
    assert_eq!(second.error_code().as_deref(), Some("duplicate_code"));
    assert_eq!(second.json()["error"]["existing"]["name"], "First");
}

#[tokio::test]
async fn day_sheet_totals_multiple_types_and_adjustments() {
    let server = TestServer::start().await;
    let (cookie, employee) = server.employee("Asha").await;
    let employee_id = employee["id"].as_str().unwrap().to_string();

    let response = create_entry(
        &server,
        &cookie,
        json!({
            "employee_id": employee_id,
            "work_date": "2026-11-20",
            "lines": lines(&[
                (SALES_TYPE, "counter sales", 50_000),
                (SERVICE_TYPE, "repair job", 30_000)
            ]),
            "bonus_minor": 10_000,
            "advance_minor": 20_000,
            "deduction_minor": 5_000,
            "note": "busy day"
        }),
    )
    .await;
    assert_eq!(response.status, StatusCode::CREATED, "{}", response.text());
    let entry = response.json();
    assert_eq!(entry["business_minor"], 80_000);
    assert_eq!(entry["bonus_minor"], 10_000);
    assert_eq!(entry["gross_minor"], 90_000);
    assert_eq!(entry["net_minor"], 65_000);
    assert_eq!(entry["employee_name"], "Asha");
    assert_eq!(entry["note"], "busy day");

    let stored_lines = entry["lines"].as_array().unwrap();
    assert_eq!(stored_lines.len(), 2);
    assert_eq!(stored_lines[0]["type_name"], "Sales");
    assert_eq!(stored_lines[0]["description"], "counter sales");
    assert_eq!(stored_lines[1]["type_name"], "Service");

    // Duplicate employee+date is a conflict carrying the existing row.
    let duplicate = create_entry(
        &server,
        &cookie,
        json!({
            "employee_id": employee_id,
            "work_date": "2026-11-20",
            "lines": lines(&[(SALES_TYPE, "", 1_000)])
        }),
    )
    .await;
    assert_eq!(duplicate.status, StatusCode::CONFLICT);
    assert_eq!(
        duplicate.error_code().as_deref(),
        Some("duplicate_entry_date")
    );

    let listed = server
        .call(
            Method::GET,
            "/api/entries?from=2026-11-01&to=2026-11-30",
            None,
            Some(&cookie),
        )
        .await;
    assert_eq!(listed.status, StatusCode::OK);
    assert_eq!(listed.json()[0]["lines"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn line_validation_is_strict_and_server_owned() {
    let server = TestServer::start().await;
    let (cookie, employee) = server.employee("Ravi").await;
    let employee_id = employee["id"].as_str().unwrap();

    let no_lines = create_entry(
        &server,
        &cookie,
        json!({ "employee_id": employee_id, "work_date": "2026-11-21", "lines": [] }),
    )
    .await;
    assert_eq!(no_lines.status, StatusCode::BAD_REQUEST);
    assert_eq!(no_lines.json()["error"]["fields"][0]["field"], "lines");

    let duplicate_type = create_entry(
        &server,
        &cookie,
        json!({
            "employee_id": employee_id,
            "work_date": "2026-11-21",
            "lines": lines(&[(SALES_TYPE, "", 1_000), (SALES_TYPE, "", 2_000)])
        }),
    )
    .await;
    assert_eq!(duplicate_type.status, StatusCode::BAD_REQUEST);

    let unknown_type = create_entry(
        &server,
        &cookie,
        json!({
            "employee_id": employee_id,
            "work_date": "2026-11-21",
            "lines": lines(&[("00000000-0000-4000-8000-00000000ffff", "", 1_000)])
        }),
    )
    .await;
    assert_eq!(unknown_type.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        unknown_type.json()["error"]["fields"][0]["field"],
        "type_id"
    );

    let zero = create_entry(
        &server,
        &cookie,
        json!({
            "employee_id": employee_id,
            "work_date": "2026-11-21",
            "lines": lines(&[(SALES_TYPE, "", 0)])
        }),
    )
    .await;
    assert_eq!(zero.status, StatusCode::BAD_REQUEST);

    // Client totals are ignored: the server recomputes from the lines.
    let tampered = create_entry(
        &server,
        &cookie,
        json!({
            "employee_id": employee_id,
            "work_date": "2026-11-22",
            "lines": lines(&[(TIPS_TYPE, "", 5_000)]),
            "business_minor": 999_999,
            "gross_minor": 999_999,
            "net_minor": 999_999
        }),
    )
    .await;
    assert_eq!(tampered.status, StatusCode::CREATED);
    assert_eq!(tampered.json()["business_minor"], 5_000);
    assert_eq!(tampered.json()["net_minor"], 5_000);
}

#[tokio::test]
async fn entry_update_recomputes_lines_and_delete_is_soft() {
    let server = TestServer::start().await;
    let (cookie, employee) = server.employee("Meena").await;
    let employee_id = employee["id"].as_str().unwrap();

    let created = create_entry(
        &server,
        &cookie,
        json!({
            "employee_id": employee_id,
            "work_date": "2026-11-10",
            "lines": lines(&[(SALES_TYPE, "initial", 70_000)])
        }),
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
                "lines": lines(&[(SALES_TYPE, "revised", 40_000), (TIPS_TYPE, "tips", 10_000)]),
                "advance_minor": 5_000
            })),
            Some(&cookie),
        )
        .await;
    assert_eq!(updated.status, StatusCode::OK, "{}", updated.text());
    let entry = updated.json();
    assert_eq!(entry["business_minor"], 50_000);
    assert_eq!(entry["net_minor"], 45_000);
    assert_eq!(entry["lines"].as_array().unwrap().len(), 2);
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

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM entries WHERE id = ?")
        .bind(&entry_id)
        .fetch_one(&server.state.pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "tombstone retained for sync");

    let lines_left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM entry_lines WHERE entry_id = ?")
        .bind(&entry_id)
        .fetch_one(&server.state.pool)
        .await
        .unwrap();
    assert_eq!(lines_left, 2, "history remains available");
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
            "work_date": "2026-11-10",
            "lines": lines(&[(SALES_TYPE, "", 1_000)])
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
        .call(
            Method::GET,
            "/api/entries?from=2026-11-01",
            None,
            Some(&cookie),
        )
        .await;
    assert_eq!(partial_range.status, StatusCode::BAD_REQUEST);

    let default_range = server
        .call(Method::GET, "/api/entries", None, Some(&cookie))
        .await;
    assert_eq!(default_range.status, StatusCode::OK);
}

#[tokio::test]
async fn deleting_an_employee_keeps_historical_day_sheets() {
    let server = TestServer::start().await;
    let (cookie, employee) = server.employee("Sita").await;
    let employee_id = employee["id"].as_str().unwrap().to_string();

    let entry = create_entry(
        &server,
        &cookie,
        json!({
            "employee_id": employee_id,
            "work_date": "2026-11-05",
            "lines": lines(&[(SERVICE_TYPE, "service", 60_000)])
        }),
    )
    .await;
    assert_eq!(entry.status, StatusCode::CREATED);
    assert_eq!(entry.json()["business_minor"], 60_000);

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
        json!({
            "employee_id": employee_id,
            "work_date": "2026-11-06",
            "lines": lines(&[(SALES_TYPE, "", 1_000)])
        }),
    )
    .await;
    assert_eq!(new_entry.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn renaming_a_type_keeps_the_snapshot_on_old_sheets() {
    let server = TestServer::start().await;
    let (cookie, employee) = server.employee("Geeta").await;
    let employee_id = employee["id"].as_str().unwrap();

    let entry = create_entry(
        &server,
        &cookie,
        json!({
            "employee_id": employee_id,
            "work_date": "2026-11-08",
            "lines": lines(&[(SALES_TYPE, "", 10_000)])
        }),
    )
    .await
    .json();
    assert_eq!(entry["lines"][0]["type_name"], "Sales");

    let renamed = server
        .call(
            Method::PATCH,
            &format!("/api/earning-types/{SALES_TYPE}"),
            Some(json!({ "name": "Retail sales", "active": true })),
            Some(&cookie),
        )
        .await;
    assert_eq!(renamed.status, StatusCode::OK, "{}", renamed.text());

    let old = server
        .call(
            Method::GET,
            &format!("/api/entries/{}", entry["id"].as_str().unwrap()),
            None,
            Some(&cookie),
        )
        .await
        .json();
    assert_eq!(old["lines"][0]["type_name"], "Sales");

    let new_entry = create_entry(
        &server,
        &cookie,
        json!({
            "employee_id": employee_id,
            "work_date": "2026-11-09",
            "lines": lines(&[(SALES_TYPE, "", 10_000)])
        }),
    )
    .await
    .json();
    assert_eq!(new_entry["lines"][0]["type_name"], "Retail sales");
}

#[tokio::test]
async fn sequence_is_strictly_increasing_across_resource_types() {
    let server = TestServer::start().await;
    let (cookie, employee) = server.employee("Kiran").await;
    let employee_id = employee["id"].as_str().unwrap();

    let mut seqs = vec![employee["seq"].as_i64().unwrap()];
    let entry = create_entry(
        &server,
        &cookie,
        json!({
            "employee_id": employee_id,
            "work_date": "2026-11-07",
            "lines": lines(&[(SALES_TYPE, "", 1_000)])
        }),
    )
    .await
    .json();
    seqs.push(entry["seq"].as_i64().unwrap());

    let updated = server
        .call(
            Method::PATCH,
            &format!("/api/employees/{employee_id}"),
            Some(json!({ "name": "Kiran R", "active": true })),
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
