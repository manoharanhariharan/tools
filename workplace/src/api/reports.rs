use axum::Json;
use axum::extract::State;use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::api::{AppQuery, resolve_range};
use crate::error::{AppError, AppResult, FieldError};
use crate::reports::summary::{
    self, DayTotals, EmployeeTotals, MoneyTotals, ReportData, ReportLayout,
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
    if let Some(id) = &employee_id {
        if id.chars().count() > 64 {
            return Err(AppError::Validation(vec![FieldError::new(
                "employee_id",
                "is too long",
            )]));
        }
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
                "must be one of employee, daily, entries",
            )])
        }),
    }
}

pub async fn summary(
    State(state): State<AppState>,
    AppQuery(query): AppQuery<ReportQuery>,
) -> AppResult<Json<SummaryResponse>> {
    let request = parse_request(&query)?;
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
    AppQuery(query): AppQuery<ReportQuery>,
) -> AppResult<Response> {
    let request = parse_request(&query)?;
    let layout = layout_or_default(query.layout.as_deref(), ReportLayout::Employee)?;
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
    AppQuery(query): AppQuery<ReportQuery>,
) -> AppResult<Response> {
    let request = parse_request(&query)?;
    let layout = layout_or_default(query.layout.as_deref(), ReportLayout::Employee)?;
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
    AppQuery(query): AppQuery<ReportQuery>,
) -> AppResult<Response> {
    let request = parse_request(&query)?;
    let layout = layout_or_default(query.layout.as_deref(), ReportLayout::Employee)?;
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
