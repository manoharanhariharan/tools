//! Kite feed bootstrap.
//!
//! Reads the same `CONFIGS_GLOBAL` keys the sibling `kite-client` uses
//! (`LATEST_KITE_ENC_KEY`, `LATEST_KITE_SESSION_KEY`, `KITE_USERID`) and reports
//! whether a Kite websocket *could* be opened. It never opens that socket and
//! never exposes the enctoken to the UI.

use crate::session;
use fv_feed::kite::KITE_WS_ROOT;
use serde::Serialize;
use std::env;

/// Kite can carry far more instruments than the Shoonya web socket. This is the
/// planned overflow capacity, not a live subscription count.
const KITE_SECONDARY_CAPACITY: usize = 3_000;

#[derive(Debug, Clone, Serialize)]
pub struct KiteBootstrapDto {
    /// `FV_KITE_FEED=1` was set.
    pub enabled: bool,
    /// Both an enctoken and a user id are available.
    pub configured: bool,
    pub user_id: Option<String>,
    pub ws_host: String,
    pub status: String,
    pub note: String,
    pub secondary_capacity: usize,
}

pub async fn bootstrap() -> KiteBootstrapDto {
    let enabled = env::var("FV_KITE_FEED")
        .map(|value| value.trim() == "1")
        .unwrap_or(false);

    let values = session::config_values(&[
        "LATEST_KITE_ENC_KEY",
        "LATEST_KITE_SESSION_KEY",
        "KITE_USERID",
    ])
    .await;

    let user_id = env::var("KITE_USERID")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            values
                .get("KITE_USERID")
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        });

    let has_enc = values
        .get("LATEST_KITE_ENC_KEY")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    let configured = user_id.is_some() && has_enc;

    let (status, note) = if !enabled {
        (
            "disabled",
            "Set FV_KITE_FEED=1 to reserve Kite as the overflow market-data provider.",
        )
    } else if !configured {
        (
            "session missing",
            "FV_KITE_FEED is set, but LATEST_KITE_ENC_KEY / KITE_USERID are not available.",
        )
    } else {
        (
            "ready (not connected)",
            "Kite websocket credentials are present. The provider is prepared but does not connect yet.",
        )
    };

    KiteBootstrapDto {
        enabled,
        configured,
        user_id,
        ws_host: KITE_WS_ROOT.to_string(),
        status: status.to_string(),
        note: note.to_string(),
        secondary_capacity: KITE_SECONDARY_CAPACITY,
    }
}
