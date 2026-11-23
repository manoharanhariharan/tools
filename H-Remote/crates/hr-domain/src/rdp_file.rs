//! `.rdp` file parsing and serialisation.

use std::collections::BTreeMap;

use hr_core::{AppError, AppResult};
use serde::{Deserialize, Serialize};

use crate::server::{
    AudioQualityMode, AudioRedirectionMode, GatewayMode, RdpServer, RdpWindowResizeMode,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RdpFile {
    /// Ordered key -> (type, value). Unknown keys are preserved.
    pub settings: BTreeMap<String, (String, String)>,
}

impl RdpFile {
    pub fn parse(content: &str) -> AppResult<Self> {
        let mut settings = BTreeMap::new();
        for (index, line) in content.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut parts = line.splitn(3, ':');
            let key = parts.next().unwrap_or("").trim();
            let value_type = parts.next().unwrap_or("s").trim();
            let value = parts.next().unwrap_or("").trim();
            if key.is_empty() {
                return Err(AppError::validation(format!(
                    "invalid .rdp line {}",
                    index + 1
                )));
            }
            settings.insert(key.to_string(), (value_type.to_string(), value.to_string()));
        }
        Ok(Self { settings })
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.settings.get(key).map(|(_, value)| value.as_str())
    }

    pub fn set(&mut self, key: &str, value_type: &str, value: impl Into<String>) {
        self.settings
            .insert(key.to_string(), (value_type.to_string(), value.into()));
    }

    pub fn merge_freeform(&mut self, content: &str) {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Ok(parsed) = Self::parse(line)
                && let Some((key, value)) = parsed.settings.iter().next()
            {
                self.settings.insert(key.clone(), value.clone());
            }
        }
    }

    pub fn to_rdp_server(
        &self,
        display_name: impl Into<String>,
        password: Option<String>,
    ) -> RdpServer {
        let mut server = RdpServer::default();
        server.base.display_name = display_name.into();
        if let Some(address) = self.get("full address") {
            apply_full_address(&mut server, address);
        }
        if let Some(value) = self.get("username") {
            server.credentials.username = value.to_string();
        }
        if let Some(value) = self.get("domain") {
            server.domain = value.to_string();
        }
        if let Some(value) = self.get("administrative session").and_then(parse_bool) {
            server.is_administrative_purposes = value;
        }
        if let Some(value) = self
            .get("audiomode")
            .and_then(|value| value.parse::<u8>().ok())
        {
            server.audio_redirection_mode = match value {
                0 => AudioRedirectionMode::RedirectToLocal,
                1 => AudioRedirectionMode::LeaveOnRemote,
                _ => AudioRedirectionMode::Disabled,
            };
        }
        if let Some(value) = self.get("redirectclipboard").and_then(parse_bool) {
            server.enable_clipboard = value;
        }
        if let Some(value) = self.get("redirectdrives").and_then(parse_bool) {
            server.enable_disk_drives = value;
        }
        if let Some(value) = self.get("redirectprinters").and_then(parse_bool) {
            server.enable_printers = value;
        }
        if let Some(value) = self.get("redirectcomports").and_then(parse_bool) {
            server.enable_ports = value;
        }
        if let Some(value) = self.get("redirectsmartcards").and_then(parse_bool) {
            server.enable_smart_cards_and_win_hello = value;
        }
        if let Some(value) = self.get("usemultimon").and_then(parse_bool) {
            server.use_multimon = value;
        }
        if let Some(value) = self.get("smart sizing").and_then(parse_bool)
            && value
        {
            server.rdp_window_resize_mode = RdpWindowResizeMode::Stretch;
        }
        if let Some(value) = self
            .get("desktopwidth")
            .and_then(|value| value.parse().ok())
        {
            server.rdp_width = value;
        }
        if let Some(value) = self
            .get("desktopheight")
            .and_then(|value| value.parse().ok())
        {
            server.rdp_height = value;
        }
        if let Some(value) = self.get("screen mode id")
            && value == "2"
        {
            server.is_conn_with_full_screen = true;
        }
        if let Some(value) = self
            .get("gatewayusagemethod")
            .and_then(|value| value.parse::<u8>().ok())
        {
            server.gateway_mode = match value {
                1 => GatewayMode::UseTheseGatewayServerSettings,
                2 => GatewayMode::AutomaticallyDetectGatewayServerSettings,
                _ => GatewayMode::DoNotUseGateway,
            };
        }
        if let Some(value) = self.get("gatewayhostname") {
            server.gateway_host_name = value.to_string();
        }
        if let Some(value) = self.get("gatewayusername") {
            server.gateway_user_name = value.to_string();
        }
        if let Some(value) = self
            .get("audioqualitymode")
            .and_then(|value| value.parse::<u8>().ok())
        {
            server.audio_quality_mode = match value {
                1 => AudioQualityMode::Medium,
                2 => AudioQualityMode::High,
                _ => AudioQualityMode::Dynamic,
            };
        }
        if let Some(password) = password {
            server.credentials.password = password.into();
        }
        let mut wrapped = crate::server::Server::Rdp(server);
        wrapped.normalize();
        match wrapped {
            crate::server::Server::Rdp(server) => server,
            _ => unreachable!(),
        }
    }

    pub fn from_rdp_server(server: &RdpServer, password_blob: Option<&[u8]>) -> Self {
        let mut file = Self::default();
        file.set(
            "full address",
            "s",
            format!("{}:{}", server.address.address, server.address.port),
        );
        if !server.credentials.username.is_empty() {
            file.set("username", "s", server.credentials.username.clone());
        }
        if !server.domain.is_empty() {
            file.set("domain", "s", server.domain.clone());
        }
        if server.is_administrative_purposes {
            file.set("administrative session", "i", "1");
        }
        file.set(
            "audiomode",
            "i",
            match server.audio_redirection_mode {
                AudioRedirectionMode::RedirectToLocal => "0",
                AudioRedirectionMode::LeaveOnRemote => "1",
                AudioRedirectionMode::Disabled => "2",
            },
        );
        file.set(
            "audioqualitymode",
            "i",
            match server.audio_quality_mode {
                AudioQualityMode::Dynamic => "0",
                AudioQualityMode::Medium => "1",
                AudioQualityMode::High => "2",
            },
        );
        file.set("redirectclipboard", "i", bool_int(server.enable_clipboard));
        file.set("redirectdrives", "i", bool_int(server.enable_disk_drives));
        file.set("redirectprinters", "i", bool_int(server.enable_printers));
        file.set("redirectcomports", "i", bool_int(server.enable_ports));
        file.set(
            "redirectsmartcards",
            "i",
            bool_int(server.enable_smart_cards_and_win_hello),
        );
        file.set("usemultimon", "i", bool_int(server.use_multimon));
        file.set("authentication level", "i", "2");
        file.set("enablecredsspsupport", "i", "1");
        if server.is_conn_with_full_screen {
            file.set("screen mode id", "i", "2");
        } else {
            file.set("screen mode id", "i", "1");
            file.set("desktopwidth", "i", server.rdp_width.to_string());
            file.set("desktopheight", "i", server.rdp_height.to_string());
        }
        if matches!(
            server.rdp_window_resize_mode,
            RdpWindowResizeMode::Stretch | RdpWindowResizeMode::StretchFullScreen
        ) {
            file.set("smart sizing", "i", "1");
        }
        file.set(
            "gatewayusagemethod",
            "i",
            match server.gateway_mode {
                GatewayMode::DoNotUseGateway => "0",
                GatewayMode::UseTheseGatewayServerSettings => "1",
                GatewayMode::AutomaticallyDetectGatewayServerSettings => "2",
            },
        );
        if !server.gateway_host_name.is_empty() {
            file.set("gatewayhostname", "s", server.gateway_host_name.clone());
        }
        if !server.gateway_user_name.is_empty() {
            file.set("gatewayusername", "s", server.gateway_user_name.clone());
        }
        if let Some(blob) = password_blob {
            file.set("password 51", "b", hex_encode(blob));
        }
        file.merge_freeform(&server.rdp_file_additional_settings);
        file
    }
}

fn apply_full_address(server: &mut RdpServer, value: &str) {
    let (address, port) = match value.rsplit_once(':') {
        Some((address, port)) if port.parse::<u16>().is_ok() => {
            (address.to_string(), port.parse().unwrap_or(3389))
        }
        _ => (value.to_string(), 3389),
    };
    server.address.address = address;
    server.address.port = port;
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" => Some(true),
        "0" | "false" | "no" => Some(false),
        _ => None,
    }
}

fn bool_int(value: bool) -> &'static str {
    if value { "1" } else { "0" }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02X}")).collect()
}

impl std::fmt::Display for RdpFile {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut first = true;
        for (key, (value_type, value)) in &self.settings {
            if !first {
                write!(formatter, "\r\n")?;
            }
            first = false;
            write!(formatter, "{key}:{value_type}:{value}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_serialises_round_trip() {
        let content = "full address:s:host.example:3390\r\nusername:s:admin\r\ndomain:s:CORP\r\nredirectclipboard:i:0\r\ngatewayusagemethod:i:1\r\ngatewayhostname:s:gw\r\ncustom:i:42";
        let file = RdpFile::parse(content).unwrap();
        assert_eq!(file.get("username"), Some("admin"));
        assert_eq!(file.get("custom"), Some("42"));
        let reparsed = RdpFile::parse(&file.to_string()).unwrap();
        assert_eq!(reparsed, file);
    }

    #[test]
    fn converts_to_rdp_server() {
        let content = "full address:s:10.0.0.5:3390\r\nusername:s:root\r\ndomain:s:LAB\r\naudiomode:i:1\r\nredirectclipboard:i:0\r\nscreen mode id:i:2\r\ngatewayusagemethod:i:1\r\ngatewayhostname:s:gw.lab";
        let server = RdpFile::parse(content)
            .unwrap()
            .to_rdp_server("Imported", Some("pw".into()));
        assert_eq!(server.address.address, "10.0.0.5");
        assert_eq!(server.address.port, 3390);
        assert_eq!(server.credentials.username, "root");
        assert_eq!(server.domain, "LAB");
        assert_eq!(
            server.audio_redirection_mode,
            AudioRedirectionMode::LeaveOnRemote
        );
        assert!(!server.enable_clipboard);
        assert!(server.is_conn_with_full_screen);
        assert_eq!(
            server.gateway_mode,
            GatewayMode::UseTheseGatewayServerSettings
        );
        assert_eq!(server.credentials.password.expose(), "pw");
    }

    #[test]
    fn builds_rdp_file_with_optional_password_blob() {
        let mut server = RdpServer::default();
        server.address.address = "host".into();
        server.rdp_file_additional_settings = "custom:s:value".into();
        let file = RdpFile::from_rdp_server(&server, Some(&[0xDE, 0xAD]));
        assert_eq!(file.get("full address"), Some("host:3389"));
        assert_eq!(file.get("password 51"), Some("DEAD"));
        assert_eq!(file.get("custom"), Some("value"));
        assert_eq!(file.get("redirectclipboard"), Some("1"));
    }

    #[test]
    fn invalid_lines_are_rejected() {
        assert!(RdpFile::parse(":s:value").is_err());
        assert!(RdpFile::parse("# comment\n").unwrap().settings.is_empty());
    }
}
