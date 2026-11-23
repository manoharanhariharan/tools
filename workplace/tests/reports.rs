mod common;

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use common::TestServer;

async fn entry(server: &TestServer, cookie: &str, body: Value) -> Value {
    let response = server
        .call(Method::POST, "/api/entries", Some(body), Some(cookie))
        .await;
    assert_eq!(response.status, StatusCode::CREATED, "{}", response.text());
    response.json()
}

#[tokio::test]
async fn summary_matches_entry_level_totals_and_groupings() {
    let server = TestServer::start().await;
    let (cookie, asha) = server.employee("Asha", "day_rate", 80_000).await;
    let (_, ravi) = server.employee("Ravi", "hourly", 15_000).await;
    let asha_id = asha["id"].as_str().unwrap().to_string();
    let ravi_id = ravi["id"].as_str().unwrap().to_string();

    entry(
        &server,
        &cookie,
        json!({
            "employee_id": asha_id,
            "work_date": "2026-11-01",
            "bonus_minor": 10_000,
            "advance_minor": 20_000,
            "deduction_minor": 5_000
        }),
    )
    .await;
    entry(
        &server,
        &cookie,
        json!({
            "employee_id": asha_id,
            "work_date": "2026-11-02",
            "day_fraction_bp": 5_000,
            "overtime_minutes": 120
        }),
    )
    .await;
    entry(
        &server,
        &cookie,
        json!({
            "employee_id": ravi_id,
            "work_date": "2026-11-01",
            "regular_minutes": 450,
            "overtime_minutes": 90
        }),
    )
    .await;

    let response = server
        .call(
            Method::GET,
            "/api/reports/summary?from=2026-11-01&to=2026-11-30",
            None,
            Some(&cookie),
        )
        .await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.text());
    let report = response.json();

    // Asha day 1: 80000 + 10000 - 20000 - 5000 = 65000
    // Asha day 2: 40000 + 30000 = 70000
    // Ravi day 1: 112500 + 33750 = 146250
    assert_eq!(report["totals"]["entries"], 3);
    assert_eq!(report["totals"]["base_minor"], 232_500);
    assert_eq!(report["totals"]["ot_minor"], 63_750);
    assert_eq!(report["totals"]["bonus_minor"], 10_000);
    assert_eq!(report["totals"]["advance_minor"], 20_000);
    assert_eq!(report["totals"]["deduction_minor"], 5_000);
    assert_eq!(report["totals"]["net_minor"], 281_250);

    let employees = report["employees"].as_array().unwrap();
    assert_eq!(employees.len(), 2);
    assert_eq!(employees[0]["name"], "Asha");
    assert_eq!(employees[0]["totals"]["entries"], 2);
    assert_eq!(employees[0]["totals"]["net_minor"], 135_000);
    assert_eq!(employees[1]["name"], "Ravi");
    assert_eq!(employees[1]["pay_basis"], "hourly");
    assert_eq!(employees[1]["totals"]["net_minor"], 146_250);

    let days = report["days"].as_array().unwrap();
    assert_eq!(days.len(), 2);
    assert_eq!(days[0]["work_date"], "2026-11-01");
    assert_eq!(days[0]["totals"]["entries"], 2);
    assert_eq!(days[1]["totals"]["net_minor"], 70_000);

    // Employee filter narrows the report without changing its math.
    let filtered = server
        .call(
            Method::GET,
            &format!(
                "/api/reports/summary?from=2026-11-01&to=2026-11-30&employee_id={ravi_id}"
            ),
            None,
            Some(&cookie),
        )
        .await
        .json();
    assert_eq!(filtered["totals"]["entries"], 1);
    assert_eq!(filtered["totals"]["net_minor"], 146_250);
    assert_eq!(filtered["employees"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn soft_deleted_entries_are_excluded_but_deleted_employee_history_remains() {
    let server = TestServer::start().await;
    let (cookie, employee) = server.employee("Meena", "fixed", 60_000).await;
    let employee_id = employee["id"].as_str().unwrap().to_string();

    let kept = entry(
        &server,
        &cookie,
        json!({ "employee_id": employee_id, "work_date": "2026-11-01" }),
    )
    .await;
    entry(
        &server,
        &cookie,
        json!({ "employee_id": employee_id, "work_date": "2026-11-02" }),
    )
    .await;

    // Soft-delete the second entry.
    let all = server
        .call(
            Method::GET,
            "/api/entries?from=2026-11-01&to=2026-11-30",
            None,
            Some(&cookie),
        )
        .await
        .json();
    let second_id = all[1]["id"].as_str().unwrap();
    server
        .call(
            Method::DELETE,
            &format!("/api/entries/{second_id}"),
            None,
            Some(&cookie),
        )
        .await;

    // Soft-delete the employee.
    server
        .call(
            Method::DELETE,
            &format!("/api/employees/{employee_id}"),
            None,
            Some(&cookie),
        )
        .await;

    let report = server
        .call(
            Method::GET,
            "/api/reports/summary?from=2026-11-01&to=2026-11-30",
            None,
            Some(&cookie),
        )
        .await
        .json();
    assert_eq!(report["totals"]["entries"], 1);
    assert_eq!(report["totals"]["net_minor"], 60_000);
    assert_eq!(report["employees"][0]["name"], "Meena");
    assert_eq!(
        report["employees"][0]["employee_id"],
        kept["employee_id"]
    );
}

#[tokio::test]
async fn csv_exports_are_downloadable_and_correct() {
    let server = TestServer::start().await;
    let (cookie, employee) = server.employee("Asha, Kumari", "day_rate", 80_000).await;
    let employee_id = employee["id"].as_str().unwrap().to_string();
    entry(
        &server,
        &cookie,
        json!({
            "employee_id": employee_id,
            "work_date": "2026-11-01",
            "advance_minor": 5000,
            "note": "paid, with \"quotes\""
        }),
    )
    .await;

    let response = server
        .call(
            Method::GET,
            "/api/reports/export.csv?from=2026-11-01&to=2026-11-30&layout=entries",
            None,
            Some(&cookie),
        )
        .await;
    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(
        response.headers.get("content-type").unwrap(),
        "text/csv; charset=utf-8"
    );
    let disposition = response
        .headers
        .get("content-disposition")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(disposition.contains("attachment"));
    assert!(disposition.contains("entries"));
    assert!(response.body.starts_with(&[0xEF, 0xBB, 0xBF]));

    let text = String::from_utf8(response.body[3..].to_vec()).unwrap();
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(text.as_bytes());
    let headers = reader.headers().unwrap().clone();
    assert_eq!(headers.get(0), Some("date"));
    let rows: Vec<csv::StringRecord> = reader.records().map(|r| r.unwrap()).collect();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].get(1), Some("Asha, Kumari"));
    assert_eq!(rows[0].get(13), Some("750.00"));
    assert_eq!(rows[0].get(14), Some("paid, with \"quotes\""));
    assert_eq!(rows[1].get(0), Some("TOTAL"));
}

#[tokio::test]
async fn printable_html_is_escaped_and_self_contained() {
    let server = TestServer::start().await;
    let (cookie, employee) = server
        .employee("<script>alert('x')</script>", "fixed", 50_000)
        .await;
    let employee_id = employee["id"].as_str().unwrap().to_string();
    entry(
        &server,
        &cookie,
        json!({ "employee_id": employee_id, "work_date": "2026-11-03", "note": "<b>bold</b>" }),
    )
    .await;

    let response = server
        .call(
            Method::GET,
            "/api/reports/print.html?from=2026-11-01&to=2026-11-30&layout=entries",
            None,
            Some(&cookie),
        )
        .await;
    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(
        response.headers.get("content-type").unwrap(),
        "text/html; charset=utf-8"
    );
    let page = response.text();
    assert!(page.contains("&lt;script&gt;alert(&#39;x&#39;)&lt;/script&gt;"));
    assert!(page.contains("&lt;b&gt;bold&lt;/b&gt;"));
    assert!(!page.contains("<script>alert"));
    assert!(page.contains("@media print"));
    assert!(page.contains("₹500.00"));
}

#[tokio::test]
async fn pdf_endpoints_produce_valid_documents_for_every_layout() {
    let server = TestServer::start().await;
    let (cookie, employee) = server.employee("Asha", "day_rate", 80_000).await;
    let employee_id = employee["id"].as_str().unwrap().to_string();
    entry(
        &server,
        &cookie,
        json!({
            "employee_id": employee_id,
            "work_date": "2026-11-01",
            "overtime_minutes": 120,
            "advance_minor": 10000
        }),
    )
    .await;

    for layout in ["employee", "daily", "entries"] {
        let response = server
            .call(
                Method::GET,
                &format!(
                    "/api/reports/report.pdf?from=2026-11-01&to=2026-11-30&layout={layout}"
                ),
                None,
                Some(&cookie),
            )
            .await;
        assert_eq!(response.status, StatusCode::OK, "layout {layout}: {}", response.text());
        assert_eq!(
            response.headers.get("content-type").unwrap(),
            "application/pdf"
        );
        assert!(response.body.starts_with(b"%PDF-"), "layout {layout}");
        assert!(response.body.len() > 2_000, "layout {layout}");
        let tail = String::from_utf8_lossy(&response.body[response.body.len().saturating_sub(64)..]);
        assert!(tail.contains("%%EOF"), "layout {layout} missing EOF");
    }

    // The empty period must still render.
    let empty = server
        .call(
            Method::GET,
            "/api/reports/report.pdf?from=2025-01-01&to=2025-01-31",
            None,
            Some(&cookie),
        )
        .await;
    assert_eq!(empty.status, StatusCode::OK);
    assert!(empty.body.starts_with(b"%PDF-"));
}

#[tokio::test]
async fn report_validation_and_auth_are_enforced() {
    let server = TestServer::start().await;
    let unauth = server
        .call(
            Method::GET,
            "/api/reports/summary?from=2026-11-01&to=2026-11-30",
            None,
            None,
        )
        .await;
    assert_eq!(unauth.status, StatusCode::UNAUTHORIZED);

    let cookie = server.login().await;
    let bad_layout = server
        .call(
            Method::GET,
            "/api/reports/export.csv?from=2026-11-01&to=2026-11-30&layout=weekly",
            None,
            Some(&cookie),
        )
        .await;
    assert_eq!(bad_layout.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        bad_layout.json()["error"]["fields"][0]["field"],
        "layout"
    );

    let bad_range = server
        .call(
            Method::GET,
            "/api/reports/summary?from=2026-11-30&to=2026-11-01",
            None,
            Some(&cookie),
        )
        .await;
    assert_eq!(bad_range.status, StatusCode::BAD_REQUEST);

    let too_long = server
        .call(
            Method::GET,
            "/api/reports/summary?from=2024-01-01&to=2026-12-31",
            None,
            Some(&cookie),
        )
        .await;
    assert_eq!(too_long.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn reports_default_to_the_current_ist_month() {
    let server = TestServer::start().await;
    let cookie = server.login().await;
    let response = server
        .call(Method::GET, "/api/reports/summary", None, Some(&cookie))
        .await;
    assert_eq!(response.status, StatusCode::OK);
    let report = response.json();
    let from = report["from"].as_str().unwrap();
    let to = report["to"].as_str().unwrap();
    assert_eq!(from.len(), 10);
    assert_eq!(to.len(), 10);
    assert!(from <= to);
}
