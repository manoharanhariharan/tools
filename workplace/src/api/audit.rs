use axum::Json;
use axum::extract::State;
use serde::Deserialize;

use crate::AppState;
use crate::api::AppQuery;
use crate::auth::Auth;
use crate::error::AppResult;
use crate::repo::audit::{self, AuditRow};

#[derive(Debug, Deserialize)]
pub struct AuditQuery {
    pub limit: Option<i64>,
    pub before: Option<i64>,
}

pub async fn list(
    State(state): State<AppState>,
    Auth(auth): Auth,
    AppQuery(query): AppQuery<AuditQuery>,
) -> AppResult<Json<Vec<AuditRow>>> {
    auth.require_admin()?;
    let rows = audit::list(
        &state.pool,
        query.limit.unwrap_or(audit::DEFAULT_LIMIT),
        query.before,
    )
    .await?;
    Ok(Json(rows))
}
