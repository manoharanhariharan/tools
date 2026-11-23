use serde::{Deserialize, Serialize};

/// Category used to group protocols in pickers and the catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProtocolCategory {
    RemoteDesktop,
    Terminal,
    FileTransfer,
    Application,
    Local,
    Utility,
}

/// How a session is rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionKind {
    /// RDP/RemoteApp style desktop or published application.
    RemoteDesktop,
    /// RFB/VNC desktop.
    Vnc,
    /// SSH/Telnet/Serial terminal.
    Terminal,
    /// SFTP/FTP browser.
    FileTransfer,
    /// Local executable or embedded third-party runner.
    LocalProcess,
    /// Placeholder protocol used for read-only/empty sources.
    Placeholder,
}

/// Optional operating-system requirement for a protocol implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OsFamily {
    Windows,
    Linux,
    MacOs,
}

impl OsFamily {
    pub const fn current() -> Self {
        #[cfg(target_os = "windows")]
        {
            Self::Windows
        }
        #[cfg(target_os = "macos")]
        {
            Self::MacOs
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            Self::Linux
        }
        #[cfg(not(any(windows, unix)))]
        {
            Self::Linux
        }
    }

    pub const fn is_current(self) -> bool {
        matches!(
            (self, Self::current()),
            (Self::Windows, Self::Windows)
                | (Self::Linux, Self::Linux)
                | (Self::MacOs, Self::MacOs)
        )
    }
}

/// Feature capabilities advertised by a protocol implementation. The UI uses
/// these to enable/disable actions instead of hard-coding protocol names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolCapabilities {
    pub session_kind: SessionKind,
    pub supports_embedded_session: bool,
    pub supports_multiple_instances: bool,
    pub supports_file_transfer: bool,
    pub supports_private_key: bool,
    pub requires_os: Option<OsFamily>,
}

impl Default for ProtocolCapabilities {
    fn default() -> Self {
        Self {
            session_kind: SessionKind::RemoteDesktop,
            supports_embedded_session: false,
            supports_multiple_instances: true,
            supports_file_transfer: false,
            supports_private_key: false,
            requires_os: None,
        }
    }
}

impl ProtocolCapabilities {
    /// True when the protocol can run on the current OS.
    pub fn available(&self) -> bool {
        self.requires_os.is_none_or(OsFamily::is_current)
    }
}

/// Immutable description of a protocol implementation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolDescriptor {
    pub id: String,
    pub display_name: String,
    /// Template used for subtitles, e.g. `"{address}:{port}"`.
    pub subtitle_template: String,
    pub category: ProtocolCategory,
    pub list_order: i32,
    pub capabilities: ProtocolCapabilities,
    pub help_url: Option<String>,
}

impl ProtocolDescriptor {
    pub fn available(&self) -> bool {
        self.capabilities.available()
    }
}

/// Inventory entry. Protocols are registered at link time, which replaces the
/// reflection-based discovery used by the WPF implementation.
pub struct ProtocolRegistration {
    pub descriptor: fn() -> ProtocolDescriptor,
}

inventory::collect!(ProtocolRegistration);

/// Registers a protocol descriptor. The function must return a descriptor with
/// static strings; registration happens at link time.
#[macro_export]
macro_rules! register_protocol {
    ($descriptor_fn:path) => {
        inventory::submit! {
            $crate::registry::ProtocolRegistration { descriptor: $descriptor_fn }
        }
    };
}

/// All registered protocols, de-duplicated by id (first registration wins) and
/// sorted by `list_order` then display name.
pub fn all_protocols() -> Vec<ProtocolDescriptor> {
    let mut seen = std::collections::HashSet::new();
    let mut protocols: Vec<ProtocolDescriptor> = inventory::iter::<ProtocolRegistration>
        .into_iter()
        .map(|registration| (registration.descriptor)())
        .filter(|descriptor| seen.insert(descriptor.id.to_ascii_lowercase()))
        .collect();
    protocols.sort_by(|left, right| {
        left.list_order
            .cmp(&right.list_order)
            .then_with(|| left.display_name.cmp(&right.display_name))
            .then_with(|| left.id.cmp(&right.id))
    });
    protocols
}

/// Case-insensitive lookup.
pub fn get_protocol(id: &str) -> Option<ProtocolDescriptor> {
    all_protocols()
        .into_iter()
        .find(|descriptor| descriptor.id.eq_ignore_ascii_case(id))
}

/// Checks the registry for duplicate or malformed entries. Tests call this to
/// catch accidental double registration.
pub fn validate_registry() -> Result<(), Vec<String>> {
    let mut problems = Vec::new();
    let mut seen = std::collections::HashMap::new();
    for registration in inventory::iter::<ProtocolRegistration> {
        let descriptor = (registration.descriptor)();
        if descriptor.id.trim().is_empty() {
            problems.push("protocol with an empty id".to_string());
            continue;
        }
        if !descriptor
            .id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
        {
            problems.push(format!(
                "protocol id '{}' must be ASCII alphanumeric, '-' or '_'",
                descriptor.id
            ));
        }
        let key = descriptor.id.to_ascii_lowercase();
        *seen.entry(key).or_insert(0usize) += 1;
    }
    for (id, count) in seen {
        if count > 1 {
            problems.push(format!(
                "duplicate protocol id '{id}' registered {count} times"
            ));
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha() -> ProtocolDescriptor {
        ProtocolDescriptor {
            id: "test-alpha".to_string(),
            display_name: "Alpha".to_string(),
            subtitle_template: "{address}:{port}".to_string(),
            category: ProtocolCategory::RemoteDesktop,
            list_order: 20,
            capabilities: ProtocolCapabilities::default(),
            help_url: None,
        }
    }

    fn beta() -> ProtocolDescriptor {
        ProtocolDescriptor {
            id: "test-beta".to_string(),
            display_name: "Beta".to_string(),
            subtitle_template: "{address}".to_string(),
            category: ProtocolCategory::Terminal,
            list_order: 10,
            capabilities: ProtocolCapabilities {
                session_kind: SessionKind::Terminal,
                supports_multiple_instances: true,
                ..ProtocolCapabilities::default()
            },
            help_url: Some("https://example.invalid/beta".to_string()),
        }
    }

    crate::register_protocol!(alpha);
    crate::register_protocol!(beta);

    #[test]
    fn registry_is_sorted_and_lookup_is_case_insensitive() {
        let protocols = all_protocols();
        let alpha_index = protocols.iter().position(|p| p.id == "test-alpha").unwrap();
        let beta_index = protocols.iter().position(|p| p.id == "test-beta").unwrap();
        assert!(beta_index < alpha_index);
        assert_eq!(get_protocol("TEST-ALPHA").unwrap().display_name, "Alpha");
        assert!(get_protocol("does-not-exist").is_none());
    }

    #[test]
    fn registry_has_no_duplicates() {
        assert_eq!(validate_registry(), Ok(()));
    }

    #[test]
    fn unavailable_protocols_are_reported() {
        let capabilities = ProtocolCapabilities {
            requires_os: Some(match OsFamily::current() {
                OsFamily::Windows => OsFamily::Linux,
                _ => OsFamily::Windows,
            }),
            ..ProtocolCapabilities::default()
        };
        assert!(!capabilities.available());
    }
}
