mod common;

use std::sync::Arc;

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};
use uuid::Uuid;

use common::{TestServer, TestResponse};

fn employee_change(id: &str, base_seq: i64, name: &str, code: Option<&str>) -> Value {
    json!({
        "entity": "employee",
        "id": id,
        "base_seq": base_seq,
        "data": {
            "name": name,
            "code": code,
            "pay_basis": "day_rate",
            "rate_minor": 80000
        }
    })
}

fn entry_change(id: &str, base_seq: i64, employee_id: &str, work_date: &str) -> Value {
    json!({
        "entity": "entry",
        "id": id,
        "base_seq": base_seq,
        "data": {
            "employee_id": employee_id,
            "work_date": work_date
        }
    })
}

async fn push(server: &TestServer, cookie: &str, changes: Vec<Value>) -> TestResponse {
    server
        .call(
            Method::POST,
            "/api/sync/push",
            Some(json!({ "device_id": "test-device", "changes": changes })),
            Some(cookie),
        )
        .await
}

fn result_for<'a>(response: &'a Value, id: &str) -> &'a Value {
    response["results"]
        .as_array()
        .expect("results array")
        .iter()
        .find(|r| r["id"] == id)
        .unwrap_or_else(|| panic!("no result for {id}: {response}"))
}

#[tokio::test]
async fn pull_pages_through_all_changes_in_sequence_order() {
    let server = TestServer::start().await;
    let (cookie, employee) = server.employee("Asha", "fixed", 60_000).await;
    let employee_id = employee["id"].as_str().unwrap().to_string();

    for day in ["2026-11-01", "2026-11-02"] {
        let response = server
            .call(
                Method::POST,
                "/api/entries",
                Some(json!({ "employee_id": employee_id, "work_date": day })),
                Some(&cookie),
            )
            .await;
        assert_eq!(response.status, StatusCode::CREATED);
    }

    let state = server
        .call(Method::GET, "/api/sync/state", None, Some(&cookie))
        .await
        .json();
    let latest = state["cursor"].as_i64().unwrap();
    assert!(latest >= 3);

    let mut since = 0;
    let mut collected: Vec<(i64, String)> = Vec::new();
    let mut pages = 0;
    loop {
        pages += 1;
        assert!(pages < 20, "pagination did not terminate");
        let page = server
            .call(
                Method::GET,
                &format!("/api/sync/pull?since={since}&limit=1"),
                None,
                Some(&cookie),
            )
            .await;
        assert_eq!(page.status, StatusCode::OK);
        let body = page.json();
        assert!(body["latest"].as_i64().unwrap() >= latest - 1);

        for employee in body["employees"].as_array().unwrap() {
            collected.push((employee["seq"].as_i64().unwrap(), "employee".into()));
        }
        for entry in body["entries"].as_array().unwrap() {
            collected.push((entry["seq"].as_i64().unwrap(), "entry".into()));
        }

        if body["has_more"] == false {
            break;
        }
        assert_eq!(body["cursor"].as_i64().unwrap(), since + 1);
        since = body["cursor"].as_i64().unwrap();
    }

    assert_eq!(collected.len(), 3);
    for pair in collected.windows(2) {
        assert!(pair[0].0 < pair[1].0, "out of order: {collected:?}");
    }
}

#[tokio::test]
async fn offline_bundle_creates_employee_and_entry_in_any_order() {
    let server = TestServer::start().await;
    let cookie = server.login().await;
    let employee_id = Uuid::new_v4().to_string();
    let entry_id = Uuid::new_v4().to_string();

    // Entry listed before its employee on purpose: the server reorders.
    let response = push(
        &server,
        &cookie,
        vec![
            entry_change(&entry_id, 0, &employee_id, "2026-11-03"),
            employee_change(&employee_id, 0, "Offline Asha", Some("OFF-1")),
        ],
    )
    .await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.text());
    let body = response.json();

    assert_eq!(result_for(&body, &employee_id)["status"], "applied");
    assert_eq!(result_for(&body, &entry_id)["status"], "applied");
    let server_entry = &result_for(&body, &entry_id)["server"];
    assert_eq!(server_entry["base_minor"], 80_000);
    assert_eq!(server_entry["net_minor"], 80_000);
    assert!(server_entry["seq"].as_i64().unwrap() > 0);
    assert_eq!(server_entry["deleted_at"], Value::Null);

    let pull = server
        .call(Method::GET, "/api/sync/pull?since=0", None, Some(&cookie))
        .await
        .json();
    assert_eq!(pull["employees"].as_array().unwrap().len(), 1);
    assert_eq!(pull["entries"].as_array().unwrap().len(), 1);
    assert!(pull["entries"][0].get("employee_name").is_none());
}

#[tokio::test]
async fn retries_are_idempotent_and_do_not_burn_sequence_numbers() {
    let server = TestServer::start().await;
    let cookie = server.login().await;
    let employee_id = Uuid::new_v4().to_string();

    let first = push(
        &server,
        &cookie,
        vec![employee_change(&employee_id, 0, "Asha", None)],
    )
    .await
    .json();
    let seq = result_for(&first, &employee_id)["server"]["seq"]
        .as_i64()
        .unwrap();
    let cursor_after_first = first["cursor"].as_i64().unwrap();

    // Client never received the response and retries with base_seq 0.
    let retry = push(
        &server,
        &cookie,
        vec![employee_change(&employee_id, 0, "Asha", None)],
    )
    .await
    .json();
    assert_eq!(result_for(&retry, &employee_id)["status"], "noop");
    assert_eq!(retry["cursor"].as_i64().unwrap(), cursor_after_first);

    // Delete, then retry the delete: no-op and no new sequence.
    let delete = push(
        &server,
        &cookie,
        vec![json!({ "entity": "employee", "id": employee_id, "base_seq": seq, "deleted": true })],
    )
    .await
    .json();
    assert_eq!(result_for(&delete, &employee_id)["status"], "applied");
    let after_delete = delete["cursor"].as_i64().unwrap();
    assert_eq!(after_delete, cursor_after_first + 1);

    let delete_retry = push(
        &server,
        &cookie,
        vec![json!({ "entity": "employee", "id": employee_id, "base_seq": seq, "deleted": true })],
    )
    .await
    .json();
    assert_eq!(result_for(&delete_retry, &employee_id)["status"], "noop");
    assert_eq!(delete_retry["cursor"].as_i64().unwrap(), after_delete);

    // Deleting a row the server never had is also a no-op.
    let unknown = push(
        &server,
        &cookie,
        vec![json!({ "entity": "employee", "id": Uuid::new_v4().to_string(), "base_seq": 0, "deleted": true })],
    )
    .await
    .json();
    assert_eq!(unknown["results"][0]["status"], "noop");
}

#[tokio::test]
async fn stale_writes_conflict_and_can_be_forced() {
    let server = TestServer::start().await;
    let cookie = server.login().await;
    let employee_id = Uuid::new_v4().to_string();

    let created = push(
        &server,
        &cookie,
        vec![employee_change(&employee_id, 0, "Asha", Some("A-1"))],
    )
    .await
    .json();
    let base = result_for(&created, &employee_id)["server"]["seq"]
        .as_i64()
        .unwrap();

    let applied = push(
        &server,
        &cookie,
        vec![employee_change(&employee_id, base, "Asha Updated", Some("A-1"))],
    )
    .await
    .json();
    assert_eq!(result_for(&applied, &employee_id)["status"], "applied");

    // Second device still thinks it is at `base`.
    let stale = push(
        &server,
        &cookie,
        vec![employee_change(&employee_id, base, "Asha From Phone", Some("A-1"))],
    )
    .await
    .json();
    let stale_result = result_for(&stale, &employee_id);
    assert_eq!(stale_result["status"], "conflict");
    assert_eq!(stale_result["conflict"], "stale");
    assert_eq!(stale_result["server"]["name"], "Asha Updated");

    // Same stale base but identical content: retry, not a conflict.
    let same = push(
        &server,
        &cookie,
        vec![employee_change(&employee_id, base, "Asha Updated", Some("A-1"))],
    )
    .await
    .json();
    assert_eq!(result_for(&same, &employee_id)["status"], "noop");

    // Force wins.
    let forced = push(
        &server,
        &cookie,
        vec![json!({
            "entity": "employee",
            "id": employee_id,
            "base_seq": base,
            "force": true,
            "data": { "name": "Asha Forced", "code": "A-1", "pay_basis": "day_rate", "rate_minor": 90000 }
        })],
    )
    .await
    .json();
    assert_eq!(result_for(&forced, &employee_id)["status"], "applied");
    assert_eq!(
        result_for(&forced, &employee_id)["server"]["name"],
        "Asha Forced"
    );

    // Updating a row that does not exist conflicts unless forced.
    let missing_id = Uuid::new_v4().to_string();
    let missing = push(
        &server,
        &cookie,
        vec![employee_change(&missing_id, 99, "Ghost", None)],
    )
    .await
    .json();
    assert_eq!(result_for(&missing, &missing_id)["status"], "conflict");
    assert_eq!(result_for(&missing, &missing_id)["conflict"], "missing_on_server");

    let forced_create = push(
        &server,
        &cookie,
        vec![json!({
            "entity": "employee",
            "id": missing_id,
            "base_seq": 99,
            "force": true,
            "data": { "name": "Ghost", "pay_basis": "fixed", "rate_minor": 50000 }
        })],
    )
    .await
    .json();
    assert_eq!(
        result_for(&forced_create, &missing_id)["status"],
        "applied"
    );
}

#[tokio::test]
async fn creating_over_an_existing_id_requires_force() {
    let server = TestServer::start().await;
    let cookie = server.login().await;
    let employee_id = Uuid::new_v4().to_string();

    push(
        &server,
        &cookie,
        vec![employee_change(&employee_id, 0, "Original", None)],
    )
    .await;

    let duplicate = push(
        &server,
        &cookie,
        vec![employee_change(&employee_id, 0, "Different", None)],
    )
    .await
    .json();
    assert_eq!(result_for(&duplicate, &employee_id)["status"], "conflict");
    assert_eq!(
        result_for(&duplicate, &employee_id)["conflict"],
        "already_exists"
    );
}

#[tokio::test]
async fn unique_constraints_surface_as_actionable_conflicts() {
    let server = TestServer::start().await;
    let cookie = server.login().await;

    let first_id = Uuid::new_v4().to_string();
    let second_id = Uuid::new_v4().to_string();
    push(
        &server,
        &cookie,
        vec![employee_change(&first_id, 0, "First", Some("SHARED"))],
    )
    .await;

    let duplicate_code = push(
        &server,
        &cookie,
        vec![employee_change(&second_id, 0, "Second", Some("SHARED"))],
    )
    .await
    .json();
    let code_result = result_for(&duplicate_code, &second_id);
    assert_eq!(code_result["status"], "conflict");
    assert_eq!(code_result["conflict"], "duplicate_code");
    assert_eq!(code_result["server"]["id"], first_id);

    let entry_a = Uuid::new_v4().to_string();
    let entry_b = Uuid::new_v4().to_string();
    let created_entry = push(
        &server,
        &cookie,
        vec![entry_change(&entry_a, 0, &first_id, "2026-11-10")],
    )
    .await
    .json();
    assert_eq!(result_for(&created_entry, &entry_a)["status"], "applied");

    let duplicate_date = push(
        &server,
        &cookie,
        vec![entry_change(&entry_b, 0, &first_id, "2026-11-10")],
    )
    .await
    .json();
    let date_result = result_for(&duplicate_date, &entry_b);
    assert_eq!(date_result["status"], "conflict");
    assert_eq!(date_result["conflict"], "duplicate_entry_date");
    assert_eq!(date_result["server"]["id"], entry_a);
}

#[tokio::test]
async fn invalid_changes_do_not_block_the_rest_of_the_batch() {
    let server = TestServer::start().await;
    let cookie = server.login().await;
    let good_id = Uuid::new_v4().to_string();
    let orphan_entry = Uuid::new_v4().to_string();
    let bad_employee = Uuid::new_v4().to_string();
    let bad_amount = Uuid::new_v4().to_string();

    let response = push(
        &server,
        &cookie,
        vec![
            employee_change(&good_id, 0, "Good", None),
            entry_change(&orphan_entry, 0, &Uuid::new_v4().to_string(), "2026-11-11"),
            json!({ "entity": "employee", "id": bad_employee, "base_seq": 0,
                    "data": { "name": "", "pay_basis": "monthly", "rate_minor": -1 } }),
            json!({ "entity": "entry", "id": bad_amount, "base_seq": 0,
                    "data": { "employee_id": good_id, "work_date": "2026-11-12",
                              "overtime_minutes": 99999 } }),
            json!({ "entity": "unknown", "id": Uuid::new_v4().to_string(), "base_seq": 0 }),
        ],
    )
    .await;
    assert_eq!(response.status, StatusCode::OK);
    let body = response.json();

    assert_eq!(result_for(&body, &good_id)["status"], "applied");
    assert_eq!(result_for(&body, &orphan_entry)["status"], "invalid");
    assert_eq!(
        result_for(&body, &orphan_entry)["error"]["fields"][0]["field"],
        "employee_id"
    );
    assert_eq!(result_for(&body, &bad_employee)["status"], "invalid");
    assert_eq!(result_for(&body, &bad_amount)["status"], "invalid");
    let unknown = body["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["entity"] == "unknown")
        .unwrap();
    assert_eq!(unknown["status"], "invalid");

    // Only the good employee increased the cursor by one.
    let state = server
        .call(Method::GET, "/api/sync/state", None, Some(&cookie))
        .await
        .json();
    assert_eq!(state["cursor"].as_i64().unwrap(), 1);
}

#[tokio::test]
async fn client_amounts_are_never_trusted() {
    let server = TestServer::start().await;
    let cookie = server.login().await;
    let employee_id = Uuid::new_v4().to_string();
    let entry_id = Uuid::new_v4().to_string();

    push(
        &server,
        &cookie,
        vec![employee_change(&employee_id, 0, "Asha", None)],
    )
    .await;

    let response = push(
        &server,
        &cookie,
        vec![json!({
            "entity": "entry",
            "id": entry_id,
            "base_seq": 0,
            "data": {
                "employee_id": employee_id,
                "work_date": "2026-11-13",
                "regular_minutes": 480,
                "base_minor": 1,
                "ot_minor": 1,
                "gross_minor": 1,
                "net_minor": 1,
                "advance_minor": 20000
            }
        })],
    )
    .await
    .json();

    let server_entry = &result_for(&response, &entry_id)["server"];
    assert_eq!(server_entry["base_minor"], 80_000);
    assert_eq!(server_entry["net_minor"], 60_000);
}

#[tokio::test]
async fn tombstones_are_pulled_and_force_can_resurrect() {
    let server = TestServer::start().await;
    let (cookie, employee) = server.employee("Asha", "fixed", 60_000).await;
    let employee_id = employee["id"].as_str().unwrap().to_string();
    let seq = employee["seq"].as_i64().unwrap();

    let deleted = server
        .call(
            Method::DELETE,
            &format!("/api/employees/{employee_id}"),
            None,
            Some(&cookie),
        )
        .await;
    assert_eq!(deleted.status, StatusCode::OK);
    let tombstone_seq = deleted.json()["seq"].as_i64().unwrap();

    let pull = server
        .call(
            Method::GET,
            &format!("/api/sync/pull?since={seq}"),
            None,
            Some(&cookie),
        )
        .await
        .json();
    assert_eq!(pull["employees"].as_array().unwrap().len(), 1);
    assert!(pull["employees"][0]["deleted_at"].is_i64());

    // Stale resurrection is rejected...
    let stale = push(
        &server,
        &cookie,
        vec![employee_change(&employee_id, seq, "Asha Again", None)],
    )
    .await
    .json();
    assert_eq!(result_for(&stale, &employee_id)["status"], "conflict");

    // ...but an explicit force with the current seq resurrects the row.
    let resurrect = push(
        &server,
        &cookie,
        vec![json!({
            "entity": "employee",
            "id": employee_id,
            "base_seq": tombstone_seq,
            "force": true,
            "data": { "name": "Asha Again", "pay_basis": "fixed", "rate_minor": 60000 }
        })],
    )
    .await
    .json();
    let result = result_for(&resurrect, &employee_id);
    assert_eq!(result["status"], "applied");
    assert_eq!(result["server"]["deleted_at"], Value::Null);
    assert_eq!(result["server"]["created_at"], employee["created_at"]);
}

#[tokio::test]
async fn concurrent_pushes_get_unique_monotonic_sequences() {
    let server = Arc::new(TestServer::start().await);
    let cookie = server.login().await;

    let mut handles = Vec::new();
    for index in 0..10 {
        let app = server.app.clone();
        let cookie = cookie.clone();
        handles.push(tokio::spawn(async move {
            let id = Uuid::new_v4().to_string();
            let body = json!({
                "device_id": format!("device-{index}"),
                "changes": [employee_change(&id, 0, &format!("Employee {index}"), None)]
            });
            let request = axum::http::Request::builder()
                .method(Method::POST)
                .uri("/api/sync/push")
                .header("x-daybook-client", "test")
                .header("content-type", "application/json")
                .header("cookie", cookie)
                .body(axum::body::Body::from(body.to_string()))
                .unwrap();
            let response = tower::ServiceExt::oneshot(app, request).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            let bytes = axum::body::to_bytes(response.into_body(), 1_048_576)
                .await
                .unwrap();
            let value: Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(value["results"][0]["status"], "applied");
            value["results"][0]["server"]["seq"].as_i64().unwrap()
        }));
    }

    let mut seqs = Vec::new();
    for handle in handles {
        seqs.push(handle.await.unwrap());
    }
    seqs.sort_unstable();
    seqs.dedup();
    assert_eq!(seqs.len(), 10);
    for pair in seqs.windows(2) {
        assert!(pair[1] > pair[0]);
    }

    let pull = server
        .call(Method::GET, "/api/sync/pull?since=0&limit=100", None, Some(&cookie))
        .await
        .json();
    assert_eq!(pull["employees"].as_array().unwrap().len(), 10);
    assert_eq!(pull["has_more"], false);
    assert_eq!(pull["cursor"].as_i64().unwrap(), 10);
}

#[tokio::test]
async fn push_rejects_oversized_batches_and_invalid_ids() {
    let server = TestServer::start().await;
    let cookie = server.login().await;

    let changes: Vec<Value> = (0..201)
        .map(|_| employee_change(&Uuid::new_v4().to_string(), 0, "Bulk", None))
        .collect();
    let oversized = push(&server, &cookie, changes).await;
    assert_eq!(oversized.status, StatusCode::BAD_REQUEST);

    let invalid_id = push(
        &server,
        &cookie,
        vec![employee_change("not-a-uuid", 0, "Bad", None)],
    )
    .await
    .json();
    assert_eq!(invalid_id["results"][0]["status"], "invalid");
}
