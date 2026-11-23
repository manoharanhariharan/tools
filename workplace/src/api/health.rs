use axum::Json;
use axum::extract::State;
use serde_json::{Value, json};

use crate::AppState;
use crate::domain::now_millis;
use crate::error::AppResult;

pub async fn health(State(state): State<AppState>) -> AppResult<Json<Value>> {
    // A real query (not just a pool handle) proves the database file is usable.
    let _: i64 = sqlx::query_scalar("SELECT 1").fetch_one(&state.pool).await?;
    Ok(Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "time": now_millis(),
        "uptime_seconds": (now_millis() - state.started_at) / 1000,
    })))
}
