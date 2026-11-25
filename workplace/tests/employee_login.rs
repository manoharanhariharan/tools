mod common;

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use common::{SALES_TYPE, SERVICE_TYPE, TestServer, lines};

async fn entry(server: &TestServer, cookie: &str, body: Value) -> Value {
    let response = server
        .call(Method::POST, "/api/entries", Some(body), Some(cookie))
        .await;
    assert_eq!(response.status, StatusCode::CREATED, "{}", response.text());
    response.json()
}

async fn set_pin(server: &TestServer, admin: &str, employee_id: &str, pin: &str) {
    let response = server
        .call(
            Method::PUT,
            &format!("/api/employees/{employee_id}/pin"),
            Some(json!({ "pin": pin })),
            Some(admin),
        )
        .await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.text());
}

async fn employee_login(server: &TestServer, code: &str, pin: &str) -> (StatusCode, Value, String) {
    let response = server
        .call(
            Method::POST,
            "/api/auth/login/employee",
            Some(json!({ "code": code, "pin": pin })),
            None,
        )
        .await;
    let cookie = response
        .headers
        .get(axum::http::header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .map(str::to_string)
        .unwrap_or_default();
    (response.status, response.json(), cookie)
}

#[tokio::test]
async fn employee_login_is_scoped_to_their_own_data() {
    let server = TestServer::start().await;
    let (admin, asha) = server.employee_with_code("Asha", Some("A1")).await;
    let (_, ravi) = server.employee_with_code("Ravi", Some("R1")).await;
    let asha_id = asha["id"].as_str().unwrap().to_string();
    let ravi_id = ravi["id"].as_str().unwrap().to_string();
    set_pin(&server, &admin, &asha_id, "1234").await;

    // 2026-11-01 is a Sunday; 2026-11-02 is a Monday.
    entry(
        &server,
        &admin,
        json!({
            "employee_id": asha_id,
            "work_date": "2026-11-01",
            "lines": lines(&[(SALES_TYPE, "", 40_000)])
        }),
    )
    .await;
    entry(
        &server,
        &admin,
        json!({
            "employee_id": ravi_id,
            "work_date": "2026-11-01",
            "lines": lines(&[(SERVICE_TYPE, "", 60_000)])
        }),
    )
    .await;
    entry(
        &server,
        &admin,
        json!({
            "employee_id": ravi_id,
            "work_date": "2026-11-02",
            "lines": lines(&[(SALES_TYPE, "", 10_000)])
        }),
    )
    .await;

    // Wrong PIN is rejected with a field error; the right one signs in.
    let (wrong_status, wrong, _) = employee_login(&server, "A1", "0000").await;
    assert_eq!(wrong_status, StatusCode::BAD_REQUEST);
    assert_eq!(wrong["error"]["fields"][0]["field"], "pin");

    let (status, body, cookie) = employee_login(&server, "A1", "1234").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["role"], "employee");
    assert_eq!(body["employee_id"], asha_id);
    assert_eq!(body["name"], "Asha");

    // me reflects the employee identity.
    let me = server
        .call(Method::GET, "/api/auth/me", None, Some(&cookie))
        .await;
    assert_eq!(me.json()["role"], "employee");
    assert_eq!(me.json()["employee_id"], asha_id);

    // Entries are scoped to Asha, even though Ravi has more data.
    let entries = server
        .call(
            Method::GET,
            "/api/entries?from=2026-11-01&to=2026-11-30",
            None,
            Some(&cookie),
        )
        .await;
    assert_eq!(entries.status, StatusCode::OK);
    let rows = entries.json().as_array().unwrap().clone();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["employee_id"], asha_id);

    // Reports are scoped to Asha's own Sunday ledger.
    let report = server
        .call(
            Method::GET,
            "/api/reports/summary?from=2026-11-01&to=2026-11-30",
            None,
            Some(&cookie),
        )
        .await;
    assert_eq!(report.status, StatusCode::OK);
    let ledger = report.json()["ledger"].as_array().unwrap().clone();
    assert_eq!(ledger.len(), 1);
    assert_eq!(ledger[0]["name"], "Asha");
    assert_eq!(ledger[0]["sunday_minor"], 40_000);

    // Creating an entry as an employee is forced onto the employee's own id.
    let created = entry(
        &server,
        &cookie,
        json!({
            "employee_id": ravi_id,
            "work_date": "2026-11-08",
            "lines": lines(&[(SALES_TYPE, "", 5_000)])
        }),
    )
    .await;
    assert_eq!(created["employee_id"], asha_id);

    // The employee cannot read another employee's entry or the roster.
    let admin_entries = server
        .call(
            Method::GET,
            "/api/entries?from=2026-11-01&to=2026-11-30",
            None,
            Some(&admin),
        )
        .await;
    let ravi_row = admin_entries
        .json()
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["employee_id"] == ravi_id)
        .unwrap()
        .clone();
    let forbidden = server
        .call(
            Method::GET,
            &format!("/api/entries/{}", ravi_row["id"].as_str().unwrap()),
            None,
            Some(&cookie),
        )
        .await;
    assert_eq!(forbidden.status, StatusCode::NOT_FOUND);

    let roster = server
        .call(Method::GET, "/api/employees", None, Some(&cookie))
        .await;
    assert_eq!(roster.status, StatusCode::FORBIDDEN);

    let sync = server
        .call(Method::GET, "/api/sync/state", None, Some(&cookie))
        .await;
    assert_eq!(sync.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn employee_pin_management_is_admin_only_and_validates() {
    let server = TestServer::start().await;
    let (admin, employee) = server.employee_with_code("Asha", Some("A1")).await;
    let id = employee["id"].as_str().unwrap().to_string();

    let short = server
        .call(
            Method::PUT,
            &format!("/api/employees/{id}/pin"),
            Some(json!({ "pin": "12" })),
            Some(&admin),
        )
        .await;
    assert_eq!(short.status, StatusCode::BAD_REQUEST);
    assert_eq!(short.json()["error"]["fields"][0]["field"], "pin");

    set_pin(&server, &admin, &id, "1234").await;

    let status = server
        .call(
            Method::GET,
            &format!("/api/employees/{id}/pin"),
            None,
            Some(&admin),
        )
        .await;
    assert_eq!(status.status, StatusCode::OK);
    assert_eq!(status.json()["has_pin"], true);

    // An employee session cannot change their own PIN.
    let (_, _, cookie) = employee_login(&server, "A1", "1234").await;
    let set_as_employee = server
        .call(
            Method::PUT,
            &format!("/api/employees/{id}/pin"),
            Some(json!({ "pin": "9999" })),
            Some(&cookie),
        )
        .await;
    assert_eq!(set_as_employee.status, StatusCode::FORBIDDEN);

    // Clearing the PIN disables employee login for that employee.
    let cleared = server
        .call(
            Method::DELETE,
            &format!("/api/employees/{id}/pin"),
            None,
            Some(&admin),
        )
        .await;
    assert_eq!(cleared.status, StatusCode::OK);
    let (login_status, _, _) = employee_login(&server, "A1", "1234").await;
    assert_eq!(login_status, StatusCode::BAD_REQUEST);
}
