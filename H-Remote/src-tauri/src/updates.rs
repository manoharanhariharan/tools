//! Update checks (P9-10).
//!
//! The manifest is a small JSON document published by the release pipeline:
//!
//! ```json
//! {
//!   "version": "26.12.1.1",
//!   "notes": "Fixes and polish",
//!   "url": "https://example.invalid/h-remote-26.12.1.1.exe",
//!   "publishedAt": 1766000000,
//!   "prerelease": false,
//!   "minimumVersion": "26.10.1.1"
//! }
//! ```

use std::time::Duration;

use hr_platform::UpdateDecision;
#[cfg(test)]
use hr_platform::evaluate_update;
use serde::Serialize;

pub const MAX_MANIFEST_BYTES: usize = 256 * 1024;
pub const CHECK_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    /// Automatic checks are enabled (`!do_not_check_for_updates`).
    pub enabled: bool,
    pub feed_configured: bool,
    pub current: String,
    pub checked_at: Option<i64>,
    pub available: bool,
    pub version: Option<String>,
    pub prerelease: bool,
    pub breaking: bool,
    pub notes: String,
    pub url: Option<String>,
    pub error: Option<String>,
}

impl UpdateStatus {
    pub fn idle(current: &str, enabled: bool, feed_configured: bool) -> Self {
        Self {
            enabled,
            feed_configured,
            current: current.to_string(),
            checked_at: None,
            available: false,
            version: None,
            prerelease: false,
            breaking: false,
            notes: String::new(),
            url: None,
            error: None,
        }
    }

    fn from_decision(
        current: &str,
        decision: UpdateDecision,
        checked_at: i64,
        enabled: bool,
    ) -> Self {
        Self {
            enabled,
            feed_configured: true,
            current: current.to_string(),
            checked_at: Some(checked_at),
            available: decision.available,
            version: decision.version,
            prerelease: decision.prerelease,
            breaking: decision.breaking,
            notes: decision.notes,
            url: decision.url,
            error: None,
        }
    }
}

pub fn failure(
    current: &str,
    enabled: bool,
    feed_configured: bool,
    checked_at: i64,
    error: impl Into<String>,
) -> UpdateStatus {
    UpdateStatus {
        enabled,
        feed_configured,
        current: current.to_string(),
        checked_at: Some(checked_at),
        available: false,
        version: None,
        prerelease: false,
        breaking: false,
        notes: String::new(),
        url: None,
        error: Some(error.into()),
    }
}

/// Fetches and evaluates the manifest. Network or manifest errors are reported
/// inside the status so the settings page can show them without a hard failure.
pub async fn check(
    feed_url: &str,
    current: &str,
    include_prerelease: bool,
    enabled: bool,
    checked_at: i64,
) -> UpdateStatus {
    let client = match reqwest::Client::builder()
        .timeout(CHECK_TIMEOUT)
        .user_agent(concat!("H-Remote/", env!("CARGO_PKG_VERSION")))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return failure(
                current,
                enabled,
                true,
                checked_at,
                format!("cannot create the HTTP client: {error}"),
            );
        }
    };
    let response = match client.get(feed_url).send().await {
        Ok(response) => response,
        Err(error) => {
            return failure(
                current,
                enabled,
                true,
                checked_at,
                format!("cannot reach the update feed: {error}"),
            );
        }
    };
    let response = match response.error_for_status() {
        Ok(response) => response,
        Err(error) => {
            return failure(
                current,
                enabled,
                true,
                checked_at,
                format!("the update feed returned an error: {error}"),
            );
        }
    };
    let body = match response.text().await {
        Ok(body) => body,
        Err(error) => {
            return failure(
                current,
                enabled,
                true,
                checked_at,
                format!("cannot read the update feed: {error}"),
            );
        }
    };
    if body.len() > MAX_MANIFEST_BYTES {
        return failure(
            current,
            enabled,
            true,
            checked_at,
            "the update manifest is too large",
        );
    }
    match hr_platform::evaluate_update_json(current, &body, include_prerelease) {
        Ok(decision) => UpdateStatus::from_decision(current, decision, checked_at, enabled),
        Err(error) => failure(current, enabled, true, checked_at, error.to_string()),
    }
}

/// Convenience for tests and callers that already have a manifest value.
#[cfg(test)]
pub fn evaluate_manifest(
    current: &str,
    manifest: &hr_platform::UpdateManifest,
    include_prerelease: bool,
    enabled: bool,
    checked_at: i64,
) -> UpdateStatus {
    match evaluate_update(current, manifest, include_prerelease) {
        Ok(decision) => UpdateStatus::from_decision(current, decision, checked_at, enabled),
        Err(error) => failure(current, enabled, true, checked_at, error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_status_reports_configuration_state() {
        let status = UpdateStatus::idle("0.1.0", true, false);
        assert!(status.enabled);
        assert!(!status.feed_configured);
        assert!(!status.available);
        assert_eq!(status.current, "0.1.0");
    }

    #[test]
    fn manifests_are_evaluated_into_status() {
        let manifest = hr_platform::UpdateManifest {
            version: "0.2.0".into(),
            notes: "Faster".into(),
            url: Some("https://example.invalid/app.exe".into()),
            published_at: Some(5),
            prerelease: false,
            minimum_version: Some("0.1.5".into()),
        };
        let status = evaluate_manifest("0.1.0", &manifest, false, true, 10);
        assert!(status.available);
        assert!(status.breaking);
        assert_eq!(status.version.as_deref(), Some("0.2.0"));
        assert_eq!(status.checked_at, Some(10));
    }

    #[test]
    fn failures_keep_the_current_version_and_clear_availability() {
        let status = failure("0.1.0", true, true, 11, "network down");
        assert!(!status.available);
        assert_eq!(status.current, "0.1.0");
        assert_eq!(status.error.as_deref(), Some("network down"));
    }

    #[tokio::test]
    async fn http_client_construction_uses_the_installed_crypto_provider() {
        // Regression: release builds with both rustls providers panicked while
        // building the reqwest client before a provider was installed.
        crate::tls::install_crypto_provider();
        let status = check("http://127.0.0.1:1/latest.json", "0.1.0", false, true, 0).await;
        assert!(status.error.is_some());
        assert!(!status.available);
    }
}
