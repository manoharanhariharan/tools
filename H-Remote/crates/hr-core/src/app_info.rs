use serde::{Deserialize, Serialize};

/// Serializable build/runtime information surfaced in the About page and
/// included in crash reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub target_os: String,
    pub target_arch: String,
    pub debug: bool,
}

impl AppInfo {
    pub fn current() -> Self {
        Self {
            name: "H-Remote".to_string(),
            // Release pipelines set HREMOTE_BUILD_VERSION from the artifact tag
            // so update checks compare release versions, not the crate version.
            version: option_env!("HREMOTE_BUILD_VERSION")
                .unwrap_or(env!("CARGO_PKG_VERSION"))
                .to_string(),
            target_os: std::env::consts::OS.to_string(),
            target_arch: std::env::consts::ARCH.to_string(),
            debug: cfg!(debug_assertions),
        }
    }

    pub fn summary(&self) -> String {
        format!(
            "{} {} ({}/{}, {})",
            self.name,
            self.version,
            self.target_os,
            self.target_arch,
            if self.debug { "debug" } else { "release" }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_info_is_complete() {
        let info = AppInfo::current();
        assert_eq!(info.name, "H-Remote");
        assert!(!info.version.is_empty());
        assert!(info.summary().contains("H-Remote"));
    }
}
