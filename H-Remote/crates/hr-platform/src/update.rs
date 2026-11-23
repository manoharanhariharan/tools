//! Update manifest parsing and version evaluation (P9-10).
//!
//! The network fetch lives in the shell (`src-tauri/src/updates.rs`) so the
//! comparison/pre-release/breaking-change rules stay pure and testable.

use std::cmp::Ordering;

use hr_core::{AppError, AppResult};
use serde::{Deserialize, Serialize};

/// Dotted numeric version with an optional pre-release suffix
/// (for example `26.11.22.1` or `0.2.0-beta.1`).
#[derive(Debug, Clone, Eq)]
pub struct Version {
    parts: Vec<u64>,
    prerelease: Option<String>,
}

impl Version {
    pub fn parse(value: &str) -> AppResult<Self> {
        let value = value.trim();
        if value.is_empty() {
            return Err(AppError::validation("the version is empty"));
        }
        let (numeric, prerelease) = match value.split_once('-') {
            Some((numeric, suffix)) => (
                numeric,
                Some(suffix.trim().to_string()).filter(|suffix| !suffix.is_empty()),
            ),
            None => (value, None),
        };
        let mut parts = Vec::new();
        for segment in numeric.split(['.', '_']) {
            let segment = segment.trim();
            if segment.is_empty() {
                return Err(AppError::validation(format!("invalid version '{value}'")));
            }
            let number = segment
                .parse::<u64>()
                .map_err(|_| AppError::validation(format!("invalid version '{value}'")))?;
            parts.push(number);
        }
        if parts.is_empty() {
            return Err(AppError::validation(format!("invalid version '{value}'")));
        }
        Ok(Self { parts, prerelease })
    }

    pub fn is_prerelease(&self) -> bool {
        self.prerelease.is_some()
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let numeric = self
            .parts
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join(".");
        match &self.prerelease {
            Some(suffix) => write!(formatter, "{numeric}-{suffix}"),
            None => formatter.write_str(&numeric),
        }
    }
}

impl PartialEq for Version {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        let length = self.parts.len().max(other.parts.len());
        for index in 0..length {
            let left = self.parts.get(index).copied().unwrap_or(0);
            let right = other.parts.get(index).copied().unwrap_or(0);
            match left.cmp(&right) {
                Ordering::Equal => {}
                ordering => return ordering,
            }
        }
        // A stable release is newer than any pre-release of the same number.
        match (&self.prerelease, &other.prerelease) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Greater,
            (Some(_), None) => Ordering::Less,
            (Some(left), Some(right)) => left.cmp(right),
        }
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateManifest {
    pub version: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub published_at: Option<i64>,
    #[serde(default)]
    pub prerelease: bool,
    /// Oldest version that can upgrade directly; older installs see a
    /// breaking-change warning (P9-10).
    #[serde(default)]
    pub minimum_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDecision {
    pub available: bool,
    pub prerelease: bool,
    /// The installed version is older than `minimumVersion`.
    pub breaking: bool,
    pub notes: String,
    pub url: Option<String>,
    pub published_at: Option<i64>,
    pub version: Option<String>,
}

impl UpdateDecision {
    pub fn none() -> Self {
        Self {
            available: false,
            prerelease: false,
            breaking: false,
            notes: String::new(),
            url: None,
            published_at: None,
            version: None,
        }
    }
}

/// Decides whether a manifest should be offered to the running build.
pub fn evaluate(
    current: &str,
    manifest: &UpdateManifest,
    include_prerelease: bool,
) -> AppResult<UpdateDecision> {
    let current = Version::parse(current)?;
    let latest = Version::parse(&manifest.version)?;
    let is_prerelease = manifest.prerelease || latest.is_prerelease();
    if is_prerelease && !include_prerelease {
        return Ok(UpdateDecision::none());
    }
    if latest <= current {
        return Ok(UpdateDecision::none());
    }
    let breaking = match &manifest.minimum_version {
        Some(minimum) if !minimum.trim().is_empty() => current < Version::parse(minimum)?,
        _ => false,
    };
    Ok(UpdateDecision {
        available: true,
        prerelease: is_prerelease,
        breaking,
        notes: manifest.notes.clone(),
        url: manifest.url.clone(),
        published_at: manifest.published_at,
        version: Some(latest.to_string()),
    })
}

/// Parses a JSON manifest and evaluates it in one step.
pub fn evaluate_json(
    current: &str,
    json: &str,
    include_prerelease: bool,
) -> AppResult<UpdateDecision> {
    let manifest: UpdateManifest = serde_json::from_str(json)
        .map_err(|error| AppError::validation(format!("invalid update manifest: {error}")))?;
    evaluate(current, &manifest, include_prerelease)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_numerically_with_padding() {
        assert!(Version::parse("26.11.22.2").unwrap() > Version::parse("26.11.22.1").unwrap());
        assert!(Version::parse("1.10").unwrap() > Version::parse("1.9.9").unwrap());
        assert_eq!(
            Version::parse("1.2").unwrap(),
            Version::parse("1.2.0").unwrap()
        );
        assert_eq!(Version::parse("1.2").unwrap().to_string(), "1.2");
    }

    #[test]
    fn prereleases_sort_before_stable_releases() {
        let beta = Version::parse("0.2.0-beta.1").unwrap();
        let stable = Version::parse("0.2.0").unwrap();
        assert!(beta < stable);
        assert!(beta.is_prerelease());
        assert!(!stable.is_prerelease());
    }

    #[test]
    fn invalid_versions_are_rejected() {
        assert!(Version::parse("").is_err());
        assert!(Version::parse("v1.2").is_err());
        assert!(Version::parse("1..2").is_err());
    }

    #[test]
    fn newer_stable_manifest_is_offered() {
        let manifest = UpdateManifest {
            version: "0.2.0".into(),
            notes: "Faster sessions".into(),
            url: Some("https://example.invalid/h-remote-0.2.0.exe".into()),
            published_at: Some(100),
            prerelease: false,
            minimum_version: None,
        };
        let decision = evaluate("0.1.0", &manifest, false).unwrap();
        assert!(decision.available);
        assert!(!decision.breaking);
        assert_eq!(decision.version.as_deref(), Some("0.2.0"));
    }

    #[test]
    fn prereleases_require_the_opt_in_flag() {
        let manifest = UpdateManifest {
            version: "0.2.0-beta.1".into(),
            notes: String::new(),
            url: None,
            published_at: None,
            prerelease: true,
            minimum_version: None,
        };
        assert!(!evaluate("0.1.0", &manifest, false).unwrap().available);
        let decision = evaluate("0.1.0", &manifest, true).unwrap();
        assert!(decision.available);
        assert!(decision.prerelease);
    }

    #[test]
    fn older_or_equal_manifests_are_ignored() {
        let mut manifest = UpdateManifest {
            version: "0.1.0".into(),
            notes: String::new(),
            url: None,
            published_at: None,
            prerelease: false,
            minimum_version: None,
        };
        assert!(!evaluate("0.1.0", &manifest, true).unwrap().available);
        manifest.version = "0.0.9".into();
        assert!(!evaluate("0.1.0", &manifest, true).unwrap().available);
    }

    #[test]
    fn a_minimum_version_older_than_the_build_marks_a_breaking_upgrade() {
        let manifest = UpdateManifest {
            version: "1.0.0".into(),
            notes: String::new(),
            url: None,
            published_at: None,
            prerelease: false,
            minimum_version: Some("0.5.0".into()),
        };
        let decision = evaluate("0.4.0", &manifest, false).unwrap();
        assert!(decision.available);
        assert!(decision.breaking);
    }

    #[test]
    fn json_manifests_round_trip() {
        let json = r#"{
            "version": "0.3.0",
            "notes": "New",
            "url": "https://example.invalid/app.exe",
            "publishedAt": 42,
            "prerelease": false,
            "minimumVersion": "0.1.0"
        }"#;
        let decision = evaluate_json("0.1.0", json, false).unwrap();
        assert!(decision.available);
        assert_eq!(decision.published_at, Some(42));
        assert!(!decision.breaking);
    }

    #[test]
    fn malformed_manifests_report_a_validation_error() {
        assert!(evaluate_json("0.1.0", "{ not json", false).is_err());
        assert!(evaluate_json("0.1.0", r#"{"version":"nope"}"#, false).is_err());
    }
}
