mod common;

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use common::{SALES_TYPE, SERVICE_TYPE, TIPS_TYPE, TestServer, lines};

async fn entry(server: &TestServer, cookie: &str, body: Value) -> Value {
    let response = server
        .call(Method::POST, "/api/entries", Some(body), Some(cookie))
        .await;
    assert_eq!(response.status, StatusCode::CREATED, "{}", response.text());
    response.json()
}

#[tokio::test]
async fn sunday_ledger_carries_prior_balances_and_is_the_default_export() {
    let server = TestServer::start().await;
    let (cookie, asha) = server.employee("Asha").await;
    let (_, ravi) = server.employee("Ravi").await;
    let (_, rekha) = server.employee("Rekha").await;
    let asha_id = asha["id"].as_str().unwrap().to_string();
    let ravi_id = ravi["id"].as_str().unwrap().to_string();
    let rekha_id = rekha["id"].as_str().unwrap().to_string();

    // Carried-forward balances recorded before the report range.
    entry(
        &server,
        &cookie,
        json!({
            "employee_id": asha_id,
            "work_date": "2026-10-25",
            "lines": lines(&[(SALES_TYPE, "prior", 100_000)])
        }),
    )
    .await;
    entry(
        &server,
        &cookie,
        json!({
            "employee_id": rekha_id,
            "work_date": "2026-10-25",
            "lines": lines(&[(SALES_TYPE, "prior", 110_000)])
        }),
    )
    .await;

    // 2026-11-01 is a Sunday; 2026-11-02 is a Monday.
    entry(
        &server,
        &cookie,
        json!({
            "employee_id": asha_id,
            "work_date": "2026-11-01",
            "lines": lines(&[(SALES_TYPE, "", 40_000)]),
            "bonus_minor": 5_000,
            "advance_minor": 5_000
        }),
    )
    .await;
    entry(
        &server,
        &cookie,
        json!({
            "employee_id": asha_id,
            "work_date": "2026-11-02",
            "lines": lines(&[(SALES_TYPE, "", 10_000)])
        }),
    )
    .await;
    entry(
        &server,
        &cookie,
        json!({
            "employee_id": ravi_id,
            "work_date": "2026-11-01",
            "lines": lines(&[(SERVICE_TYPE, "", 60_000)])
        }),
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
    assert_eq!(report["previous_total_minor"], 210_000);
    assert_eq!(report["sunday_total_minor"], 100_000);
    assert_eq!(report["current_total_minor"], 310_000);

    let ledger = report["ledger"].as_array().unwrap();
    assert_eq!(ledger.len(), 3);
    assert_eq!(ledger[0]["name"], "Asha");
    assert_eq!(ledger[0]["previous_minor"], 100_000);
    assert_eq!(ledger[0]["sunday_minor"], 40_000);
    assert_eq!(ledger[0]["current_minor"], 140_000);
    assert_eq!(ledger[1]["name"], "Ravi");
    assert_eq!(ledger[1]["previous_minor"], 0);
    assert_eq!(ledger[1]["sunday_minor"], 60_000);
    assert_eq!(ledger[2]["name"], "Rekha");
    assert_eq!(ledger[2]["previous_minor"], 110_000);
    assert_eq!(ledger[2]["sunday_minor"], 0);
    assert_eq!(ledger[2]["current_minor"], 110_000);

    // Sunday exports are the default for downloads and the printable page.
    let csv = server
        .call(
            Method::GET,
            "/api/reports/export.csv?from=2026-11-01&to=2026-11-30",
            None,
            Some(&cookie),
        )
        .await;
    assert_eq!(csv.status, StatusCode::OK);
    let text = String::from_utf8(csv.body[3..].to_vec()).unwrap();
    assert!(text.starts_with("sl_no,name,previous_total,sunday_earning,current_total"));
    assert!(text.contains("1,Asha,1000.00,400.00,1400.00"));
    assert!(text.contains(",TOTAL,2100.00,1000.00,3100.00"));

    let html = server
        .call(
            Method::GET,
            "/api/reports/print.html?from=2026-11-01&to=2026-11-30",
            None,
            Some(&cookie),
        )
        .await;
    assert_eq!(html.status, StatusCode::OK);
    let page = html.text();
    assert!(page.contains("Sunday ledger"));
    assert!(page.contains("Previous total"));
    assert!(page.contains("₹1,400.00"));
}

#[tokio::test]
async fn summary_matches_line_totals_groupings_and_types() {
    let server = TestServer::start().await;
    let (cookie, asha) = server.employee("Asha").await;
    let (_, ravi) = server.employee("Ravi").await;
    let asha_id = asha["id"].as_str().unwrap().to_string();
    let ravi_id = ravi["id"].as_str().unwrap().to_string();

    entry(
        &server,
        &cookie,
        json!({
            "employee_id": asha_id,
            "work_date": "2026-11-01",
            "lines": lines(&[(SALES_TYPE, "counter", 50_000), (SERVICE_TYPE, "repair", 30_000)]),
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
            "lines": lines(&[(SALES_TYPE, "", 40_000)])
        }),
    )
    .await;
    entry(
        &server,
        &cookie,
        json!({
            "employee_id": ravi_id,
            "work_date": "2026-11-01",
            "lines": lines(&[(SERVICE_TYPE, "fitting", 60_000), (TIPS_TYPE, "", 5_000)])
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

    assert_eq!(report["totals"]["entries"], 3);
    assert_eq!(report["totals"]["business_minor"], 185_000);
    assert_eq!(report["totals"]["bonus_minor"], 10_000);
    assert_eq!(report["totals"]["gross_minor"], 195_000);
    assert_eq!(report["totals"]["advance_minor"], 20_000);
    assert_eq!(report["totals"]["deduction_minor"], 5_000);
    assert_eq!(report["totals"]["net_minor"], 170_000);

    let employees = report["employees"].as_array().unwrap();
    assert_eq!(employees.len(), 2);
    assert_eq!(employees[0]["name"], "Asha");
    assert_eq!(employees[0]["totals"]["entries"], 2);
    assert_eq!(employees[0]["totals"]["business_minor"], 120_000);
    assert_eq!(employees[0]["totals"]["net_minor"], 105_000);
    assert_eq!(employees[1]["name"], "Ravi");
    assert_eq!(employees[1]["totals"]["net_minor"], 65_000);
    assert!(employees[0].get("pay_basis").is_none());

    let days = report["days"].as_array().unwrap();
    assert_eq!(days.len(), 2);
    assert_eq!(days[0]["work_date"], "2026-11-01");
    assert_eq!(days[0]["totals"]["entries"], 2);
    assert_eq!(days[1]["totals"]["net_minor"], 40_000);

    let types = report["types"].as_array().unwrap();
    let sales = types.iter().find(|t| t["type_name"] == "Sales").unwrap();
    let service = types.iter().find(|t| t["type_name"] == "Service").unwrap();
    assert_eq!(sales["amount_minor"], 90_000);
    assert_eq!(service["amount_minor"], 90_000);

    let filtered = server
        .call(
            Method::GET,
            &format!("/api/reports/summary?from=2026-11-01&to=2026-11-30&employee_id={ravi_id}"),
            None,
            Some(&cookie),
        )
        .await
        .json();
    assert_eq!(filtered["totals"]["entries"], 1);
    assert_eq!(filtered["totals"]["business_minor"], 65_000);
    assert_eq!(filtered["employees"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn soft_deleted_sheets_are_excluded_but_deleted_employee_history_remains() {
    let server = TestServer::start().await;
    let (cookie, employee) = server.employee("Meena").await;
    let employee_id = employee["id"].as_str().unwrap().to_string();

    entry(
        &server,
        &cookie,
        json!({
            "employee_id": employee_id,
            "work_date": "2026-11-01",
            "lines": lines(&[(SALES_TYPE, "", 60_000)])
        }),
    )
    .await;
    entry(
        &server,
        &cookie,
        json!({
            "employee_id": employee_id,
            "work_date": "2026-11-02",
            "lines": lines(&[(SALES_TYPE, "", 30_000)])
        }),
    )
    .await;

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
    assert_eq!(report["totals"]["business_minor"], 60_000);
    assert_eq!(report["employees"][0]["name"], "Meena");
}

#[tokio::test]
async fn csv_exports_are_downloadable_and_correct() {
    let server = TestServer::start().await;
    let (cookie, employee) = server.employee("Asha, Kumari").await;
    let employee_id = employee["id"].as_str().unwrap().to_string();
    entry(
        &server,
        &cookie,
        json!({
            "employee_id": employee_id,
            "work_date": "2026-11-01",
            "lines": lines(&[(SALES_TYPE, "3 jobs", 80_000)]),
            "advance_minor": 5_000,
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
    assert_eq!(rows[0].get(3), Some("800.00"));
    assert_eq!(rows[0].get(8), Some("750.00"));
    assert_eq!(rows[0].get(9), Some("Sales: ₹800.00 (3 jobs)"));
    assert_eq!(rows[0].get(10), Some("paid, with \"quotes\""));
    assert_eq!(rows[1].get(0), Some("TOTAL"));
}

#[tokio::test]
async fn printable_html_is_escaped_and_self_contained() {
    let server = TestServer::start().await;
    let (cookie, employee) = server.employee("<script>alert('x')</script>").await;
    let employee_id = employee["id"].as_str().unwrap().to_string();
    entry(
        &server,
        &cookie,
        json!({
            "employee_id": employee_id,
            "work_date": "2026-11-03",
            "lines": lines(&[(SERVICE_TYPE, "<b>bold</b>", 50_000)]),
            "note": "<b>bold</b>"
        }),
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
    assert!(page.contains("Business by earning type"));
    assert!(page.contains("₹500.00"));
}

#[tokio::test]
async fn pdf_endpoints_produce_valid_documents_for_every_layout() {
    let server = TestServer::start().await;
    let (cookie, employee) = server.employee("Asha").await;
    let employee_id = employee["id"].as_str().unwrap().to_string();
    entry(
        &server,
        &cookie,
        json!({
            "employee_id": employee_id,
            "work_date": "2026-11-01",
            "lines": lines(&[(SALES_TYPE, "", 80_000), (TIPS_TYPE, "", 5_000)]),
            "advance_minor": 10_000
        }),
    )
    .await;

    for layout in ["sunday", "employee", "daily", "entries"] {
        let response = server
            .call(
                Method::GET,
                &format!("/api/reports/report.pdf?from=2026-11-01&to=2026-11-30&layout={layout}"),
                None,
                Some(&cookie),
            )
            .await;
        assert_eq!(
            response.status,
            StatusCode::OK,
            "layout {layout}: {}",
            response.text()
        );
        assert_eq!(
            response.headers.get("content-type").unwrap(),
            "application/pdf"
        );
        assert!(response.body.starts_with(b"%PDF-"), "layout {layout}");
        assert!(response.body.len() > 2_000, "layout {layout}");
        let tail =
            String::from_utf8_lossy(&response.body[response.body.len().saturating_sub(64)..]);
        assert!(tail.contains("%%EOF"), "layout {layout} missing EOF");
    }

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
    assert_eq!(bad_layout.json()["error"]["fields"][0]["field"], "layout");

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
    assert_eq!(report["from"].as_str().unwrap().len(), 10);
    assert_eq!(report["to"].as_str().unwrap().len(), 10);
    assert!(report["from"].as_str().unwrap() <= report["to"].as_str().unwrap());
}
