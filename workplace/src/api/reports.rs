use axum::Json;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::api::{AppQuery, resolve_range};
use crate::auth::Auth;
use crate::error::{AppError, AppResult, FieldError};
use crate::reports::summary::{
    self, DayTotals, EmployeeTotals, LedgerRow, MoneyTotals, ReportData, ReportLayout, TypeTotals,
};
use crate::reports::{csv, html, pdf};

#[derive(Debug, Deserialize)]
pub struct ReportQuery {
    pub from: Option<String>,
    pub to: Option<String>,
    pub employee_id: Option<String>,
    pub layout: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SummaryResponse {
    pub from: String,
    pub to: String,
    pub generated_at: i64,
    pub totals: MoneyTotals,
    pub employees: Vec<EmployeeTotals>,
    pub days: Vec<DayTotals>,
    pub types: Vec<TypeTotals>,
    pub ledger: Vec<LedgerRow>,
    pub previous_total_minor: i64,
    pub sunday_total_minor: i64,
    pub current_total_minor: i64,
}

impl From<ReportData> for SummaryResponse {
    fn from(data: ReportData) -> Self {
        Self {
            from: data.from,
            to: data.to,
            generated_at: data.generated_at,
            totals: data.totals,
            employees: data.employees,
            days: data.days,
            types: data.types,
            ledger: data.ledger,
            previous_total_minor: data.previous_total_minor,
            sunday_total_minor: data.sunday_total_minor,
            current_total_minor: data.current_total_minor,
        }
    }
}

struct ReportRequest {
    from: String,
    to: String,
    employee_id: Option<String>,
}

fn parse_request(query: &ReportQuery) -> AppResult<ReportRequest> {
    let (from, to) = resolve_range(query.from.as_deref(), query.to.as_deref())?;
    let employee_id = query
        .employee_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if let Some(id) = &employee_id
        && id.chars().count() > 64
    {
        return Err(AppError::Validation(vec![FieldError::new(
            "employee_id",
            "is too long",
        )]));
    }
    Ok(ReportRequest {
        from,
        to,
        employee_id,
    })
}

fn layout_or_default(value: Option<&str>, default: ReportLayout) -> AppResult<ReportLayout> {
    match value {
        None => Ok(default),
        Some(value) => ReportLayout::parse(value).ok_or_else(|| {
            AppError::Validation(vec![FieldError::new(
                "layout",
                "must be one of sunday, employee, daily, entries",
            )])
        }),
    }
}

/// Employees can only ever report on themselves; admins keep their filter.
fn scoped_request(auth: &crate::auth::Identity, mut request: ReportRequest) -> ReportRequest {
    if !auth.is_admin() {
        request.employee_id = auth.employee_id.clone();
    }
    request
}

pub async fn summary(
    State(state): State<AppState>,
    Auth(auth): Auth,
    AppQuery(query): AppQuery<ReportQuery>,
) -> AppResult<Json<SummaryResponse>> {
    let request = scoped_request(&auth, parse_request(&query)?);
    let data = summary::load(
        &state.pool,
        &request.from,
        &request.to,
        request.employee_id.as_deref(),
    )
    .await?;
    Ok(Json(data.into()))
}

pub async fn export_csv(
    State(state): State<AppState>,
    Auth(auth): Auth,
    AppQuery(query): AppQuery<ReportQuery>,
) -> AppResult<Response> {
    let request = scoped_request(&auth, parse_request(&query)?);
    let layout = layout_or_default(query.layout.as_deref(), ReportLayout::Sunday)?;
    let data = summary::load(
        &state.pool,
        &request.from,
        &request.to,
        request.employee_id.as_deref(),
    )
    .await?;
    let bytes = csv::render(&data, layout)?;
    Ok(download(
        bytes,
        "text/csv; charset=utf-8",
        &format!(
            "daybook-{}-{}--{}.csv",
            layout.as_str(),
            request.from,
            request.to
        ),
        true,
    ))
}

pub async fn print_html(
    State(state): State<AppState>,
    Auth(auth): Auth,
    AppQuery(query): AppQuery<ReportQuery>,
) -> AppResult<Response> {
    let request = scoped_request(&auth, parse_request(&query)?);
    let layout = layout_or_default(query.layout.as_deref(), ReportLayout::Sunday)?;
    let data = summary::load(
        &state.pool,
        &request.from,
        &request.to,
        request.employee_id.as_deref(),
    )
    .await?;
    let page = html::render(&data, layout);
    Ok(download(
        page.into_bytes(),
        "text/html; charset=utf-8",
        &format!(
            "daybook-{}-{}--{}.html",
            layout.as_str(),
            request.from,
            request.to
        ),
        false,
    ))
}

pub async fn report_pdf(
    State(state): State<AppState>,
    Auth(auth): Auth,
    AppQuery(query): AppQuery<ReportQuery>,
) -> AppResult<Response> {
    let request = scoped_request(&auth, parse_request(&query)?);
    let layout = layout_or_default(query.layout.as_deref(), ReportLayout::Sunday)?;
    let data = summary::load(
        &state.pool,
        &request.from,
        &request.to,
        request.employee_id.as_deref(),
    )
    .await?;

    let font = state.pdf_font.clone();
    let bytes = tokio::task::spawn_blocking(move || pdf::render(&data, layout, font.as_deref()))
        .await
        .map_err(|err| AppError::internal(anyhow::anyhow!("PDF task failed: {err}")))??;

    Ok(download(
        bytes,
        "application/pdf",
        &format!(
            "daybook-{}-{}--{}.pdf",
            layout.as_str(),
            request.from,
            request.to
        ),
        true,
    ))
}

fn download(bytes: Vec<u8>, content_type: &str, filename: &str, attachment: bool) -> Response {
    let disposition = if attachment {
        format!("attachment; filename=\"{filename}\"")
    } else {
        format!("inline; filename=\"{filename}\"")
    };
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type.to_string()),
            (header::CONTENT_DISPOSITION, disposition),
            (header::CACHE_CONTROL, "no-store".to_string()),
        ],
        bytes,
    )
        .into_response()
}
