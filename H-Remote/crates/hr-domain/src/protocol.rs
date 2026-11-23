//! Shared protocol base types and protocol identity.

use hr_core::new_id;
use serde::{Deserialize, Serialize};

use crate::credential::{AlternateCredential, SecretString};
use crate::tag::normalize_tags;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum GatewayLogonMethod {
    #[default]
    Password,
    SmartCard,
}

pub const LOCAL_SOURCE_ID: &str = "local";

pub fn default_source_id() -> String {
    LOCAL_SOURCE_ID.to_string()
}

/// Every built-in protocol plus the extension points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProtocolKind {
    Rdp,
    RemoteApp,
    Vnc,
    Ssh,
    Telnet,
    Serial,
    Sftp,
    Ftp,
    LocalApp,
    Custom,
    Dummy,
}

impl ProtocolKind {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Rdp => "RDP",
            Self::RemoteApp => "RemoteApp",
            Self::Vnc => "VNC",
            Self::Ssh => "SSH",
            Self::Telnet => "Telnet",
            Self::Serial => "Serial",
            Self::Sftp => "SFTP",
            Self::Ftp => "FTP",
            Self::LocalApp => "LocalApp",
            Self::Custom => "Custom",
            Self::Dummy => "Dummy",
        }
    }

    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Rdp => "RDP",
            Self::RemoteApp => "RemoteApp",
            Self::Vnc => "VNC",
            Self::Ssh => "SSH",
            Self::Telnet => "Telnet",
            Self::Serial => "Serial",
            Self::Sftp => "SFTP",
            Self::Ftp => "FTP",
            Self::LocalApp => "Local application",
            Self::Custom => "Custom protocol",
            Self::Dummy => "Placeholder",
        }
    }

    pub const fn class_version(self) -> u32 {
        match self {
            Self::Rdp => 4,
            Self::RemoteApp => 2,
            Self::Vnc => 3,
            Self::Ssh => 3,
            Self::Telnet => 2,
            Self::Serial => 2,
            Self::Sftp => 2,
            Self::Ftp => 2,
            Self::LocalApp => 3,
            Self::Custom => 1,
            Self::Dummy => 1,
        }
    }

    /// Built-in ids that custom protocols may not redefine.
    pub const fn builtin_ids() -> &'static [&'static str] {
        &[
            "RDP",
            "RemoteApp",
            "VNC",
            "SSH",
            "Telnet",
            "Serial",
            "SFTP",
            "FTP",
            "LocalApp",
            "Dummy",
        ]
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id.to_ascii_lowercase().as_str() {
            "rdp" => Some(Self::Rdp),
            "remoteapp" => Some(Self::RemoteApp),
            "vnc" => Some(Self::Vnc),
            "ssh" => Some(Self::Ssh),
            "telnet" => Some(Self::Telnet),
            "serial" => Some(Self::Serial),
            "sftp" => Some(Self::Sftp),
            "ftp" => Some(Self::Ftp),
            "localapp" => Some(Self::LocalApp),
            "custom" => Some(Self::Custom),
            "dummy" => Some(Self::Dummy),
            _ => None,
        }
    }

    pub fn is_builtin_name(name: &str) -> bool {
        Self::builtin_ids()
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(name.trim()))
    }
}

/// Fields shared by every protocol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ProtocolBase {
    pub id: String,
    pub class_version: u32,
    pub display_name: String,
    pub sub_title: String,
    pub icon_base64: Option<String>,
    /// `#RRGGBB` or `#AARRGGBB`.
    pub color_hex: String,
    pub tags: Vec<String>,
    /// Folder path. Legacy servers migrate their tags here when empty.
    pub tree_nodes: Vec<String>,
    pub note: String,
    pub command_before_connected: String,
    pub hide_command_before_connected_window: bool,
    pub command_after_disconnected: String,
    pub selected_runner_name: Option<String>,
    pub always_open_in_new_tab_window: bool,
    /// Runtime data-source id; persisted but always rewritten for secondary sources.
    #[serde(default = "default_source_id")]
    pub source_id: String,
    /// Runtime session identity; never persisted by the repository.
    pub session_id: Option<String>,
}

impl Default for ProtocolBase {
    fn default() -> Self {
        Self::new(ProtocolKind::Dummy)
    }
}

impl ProtocolBase {
    pub fn new(kind: ProtocolKind) -> Self {
        Self {
            id: new_id(),
            class_version: kind.class_version(),
            display_name: String::new(),
            sub_title: String::new(),
            icon_base64: None,
            color_hex: String::new(),
            tags: Vec::new(),
            tree_nodes: Vec::new(),
            note: String::new(),
            command_before_connected: String::new(),
            hide_command_before_connected_window: false,
            command_after_disconnected: String::new(),
            selected_runner_name: None,
            always_open_in_new_tab_window: false,
            source_id: default_source_id(),
            session_id: None,
        }
    }

    pub fn with_display_name(kind: ProtocolKind, display_name: impl Into<String>) -> Self {
        let mut base = Self::new(kind);
        base.display_name = display_name.into();
        base
    }

    pub fn display_name_or(&self, fallback: &str) -> String {
        let trimmed = self.display_name.trim();
        if trimmed.is_empty() {
            fallback.to_string()
        } else {
            trimmed.to_string()
        }
    }

    /// Normalises tags/folder paths and trims free text. Called on save.
    pub fn normalize(&mut self) {
        self.display_name = self.display_name.trim().to_string();
        self.sub_title = self.sub_title.trim().to_string();
        self.color_hex = normalize_color(&self.color_hex);
        self.tags = normalize_tags(std::mem::take(&mut self.tags));
        self.tree_nodes = crate::folder::normalize_tree_nodes(std::mem::take(&mut self.tree_nodes));
        self.note = self.note.trim_end().to_string();
        self.command_before_connected = self.command_before_connected.trim().to_string();
        self.command_after_disconnected = self.command_after_disconnected.trim().to_string();
        self.selected_runner_name = self
            .selected_runner_name
            .as_deref()
            .map(str::trim)
            .filter(|runner| !runner.is_empty())
            .map(str::to_string);
    }

    pub fn duplicate(&self) -> Self {
        let mut base = self.clone();
        base.id = new_id();
        base.session_id = None;
        base.display_name = format!("{} (copy)", base.display_name_or("Server"));
        base
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct AddressPortBase {
    pub address: String,
    pub port: u16,
    pub alternate_credentials: Vec<AlternateCredential>,
    pub is_ping_before_connect: bool,
    pub is_auto_alternate_address_switching: bool,
}

impl AddressPortBase {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            ..Self::default()
        }
    }

    pub fn subtitle(&self, username: Option<&str>) -> String {
        let mut result = self.address.trim().to_string();
        if self.port != 0 {
            result.push(':');
            result.push_str(&self.port.to_string());
        }
        if let Some(username) = username.map(str::trim).filter(|value| !value.is_empty()) {
            result.push_str(" (");
            result.push_str(username);
            result.push(')');
        }
        result
    }

    pub fn normalize(&mut self) {
        self.address = self.address.trim().to_string();
        for alternate in &mut self.alternate_credentials {
            alternate.normalize();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct UserPasswordBase {
    pub username: String,
    pub password: SecretString,
    pub private_key: Option<String>,
    pub ask_password_when_connect: bool,
    pub inherited_credential_name: Option<String>,
}

impl UserPasswordBase {
    pub fn with_username(username: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            ..Self::default()
        }
    }

    pub fn has_private_key(&self) -> bool {
        self.private_key
            .as_deref()
            .is_some_and(|path| !path.trim().is_empty())
    }

    pub fn is_private_key_all_ascii(&self) -> bool {
        self.private_key
            .as_deref()
            .is_none_or(|path| path.is_ascii())
    }

    /// Both a password and private key must never be configured at once.
    pub fn credential_exclusivity_issue(&self) -> Option<String> {
        if !self.password.is_empty() && self.has_private_key() {
            Some("a server cannot use both a password and a private key".to_string())
        } else {
            None
        }
    }

    pub fn normalize(&mut self) {
        self.username = self.username.trim().to_string();
        self.private_key = self
            .private_key
            .as_deref()
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(str::to_string);
        self.inherited_credential_name = self
            .inherited_credential_name
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_string);
        if self.ask_password_when_connect {
            self.password.clear();
        }
    }

    /// Resolves the vault credential when configured, filling only missing
    /// values so inline overrides keep working (P7-06/P7-15).
    pub fn resolve_effective(
        &self,
        inherited: Option<&crate::credential::Credential>,
    ) -> crate::credential::EffectiveCredentials {
        let Some(inherited) = inherited else {
            return crate::credential::EffectiveCredentials {
                username: self.username.clone(),
                password: self.password.clone(),
                private_key: self.private_key.clone(),
                source: crate::credential::CredentialSource::Inline,
            };
        };
        crate::credential::EffectiveCredentials {
            username: if self.username.trim().is_empty() {
                inherited.username.clone()
            } else {
                self.username.clone()
            },
            password: if self.password.is_empty() {
                inherited.password.clone()
            } else {
                self.password.clone()
            },
            private_key: if self.has_private_key() {
                self.private_key.clone()
            } else {
                inherited.private_key_path.clone()
            },
            source: crate::credential::CredentialSource::Inherited,
        }
    }
}

pub fn is_valid_color(value: &str) -> bool {
    let Some(hex) = value.trim().strip_prefix('#') else {
        return false;
    };
    matches!(hex.len(), 6 | 8) && hex.chars().all(|character| character.is_ascii_hexdigit())
}

pub fn normalize_color(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if is_valid_color(trimmed) {
        let hex = trimmed.trim_start_matches('#').to_ascii_uppercase();
        format!("#{hex}")
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn servers_default_to_the_local_source_and_round_trip_their_source_id() {
        let mut server = crate::Server::new_ssh("Box", "host", "root");
        assert_eq!(server.base().source_id, LOCAL_SOURCE_ID);
        server.base_mut().source_id = "team-db".to_string();
        let json = serde_json::to_string(&server).unwrap();
        let restored: crate::Server = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.base().source_id, "team-db");
        // Older payloads without the field keep working.
        let mut value: serde_json::Value = serde_json::from_str(&json).unwrap();
        value.as_object_mut().unwrap().remove("sourceId");
        let legacy: crate::Server = serde_json::from_value(value).unwrap();
        assert_eq!(legacy.base().source_id, LOCAL_SOURCE_ID);
    }

    #[test]
    fn protocol_ids_are_stable_and_case_insensitive() {
        for kind in [
            ProtocolKind::Rdp,
            ProtocolKind::RemoteApp,
            ProtocolKind::Vnc,
            ProtocolKind::Ssh,
            ProtocolKind::Telnet,
            ProtocolKind::Serial,
            ProtocolKind::Sftp,
            ProtocolKind::Ftp,
            ProtocolKind::LocalApp,
            ProtocolKind::Custom,
            ProtocolKind::Dummy,
        ] {
            assert_eq!(ProtocolKind::from_id(kind.id()), Some(kind));
            assert_eq!(ProtocolKind::from_id(&kind.id().to_lowercase()), Some(kind));
        }
        assert_eq!(ProtocolKind::from_id("nope"), None);
        assert!(ProtocolKind::is_builtin_name(" rdp "));
        assert!(!ProtocolKind::is_builtin_name("Custom"));
        assert!(!ProtocolKind::is_builtin_name("MyProtocol"));
    }

    #[test]
    fn base_duplicate_gets_new_identity() {
        let mut base = ProtocolBase::with_display_name(ProtocolKind::Rdp, "Prod");
        base.session_id = Some("runtime".into());
        let duplicate = base.duplicate();
        assert_ne!(duplicate.id, base.id);
        assert_eq!(duplicate.display_name, "Prod (copy)");
        assert!(duplicate.session_id.is_none());
    }

    #[test]
    fn subtitle_includes_port_and_user() {
        let mut address = AddressPortBase::new(3389);
        address.address = " host.example ".into();
        address.normalize();
        assert_eq!(address.subtitle(Some("admin")), "host.example:3389 (admin)");
        assert_eq!(address.subtitle(Some("  ")), "host.example:3389");
    }

    #[test]
    fn password_and_private_key_are_mutually_exclusive() {
        let mut credentials = UserPasswordBase::with_username("root");
        credentials.password = "secret".into();
        credentials.private_key = Some("C:\\key".into());
        assert!(credentials.credential_exclusivity_issue().is_some());
        credentials.password.clear();
        assert!(credentials.credential_exclusivity_issue().is_none());
    }

    #[test]
    fn ask_password_clears_persisted_password() {
        let mut credentials = UserPasswordBase {
            password: "secret".into(),
            ask_password_when_connect: true,
            ..UserPasswordBase::default()
        };
        credentials.normalize();
        assert!(credentials.password.is_empty());
    }

    #[test]
    fn colors_are_normalised_or_rejected() {
        assert!(is_valid_color("#AABBCCDD"));
        assert!(is_valid_color("#aabbcc"));
        assert_eq!(normalize_color("#aabbcc"), "#AABBCC");
        assert_eq!(normalize_color("aabbcc"), "");
        assert_eq!(normalize_color("#XYZXYZ"), "");
    }
}
