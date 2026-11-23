//! JSON/CSV/RDP import and JSON export.

use std::time::{SystemTime, UNIX_EPOCH};

use hr_core::{AppError, AppResult, new_id};
use serde::{Deserialize, Serialize};

use crate::rdp_file::RdpFile;
use crate::server::Server;

pub const EXPORT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportEnvelope {
    pub schema_version: u32,
    pub exported_at: i64,
    pub servers: Vec<Server>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub servers: Vec<Server>,
    pub warnings: Vec<String>,
}

/// Plaintext export; the UI must show an explicit warning (P2-60).
pub fn export_json(servers: &[Server]) -> AppResult<String> {
    let envelope = ExportEnvelope {
        schema_version: EXPORT_SCHEMA_VERSION,
        exported_at: now_seconds(),
        servers: servers.to_vec(),
    };
    Ok(serde_json::to_string_pretty(&envelope)?)
}

pub fn import_json(content: &str) -> ImportResult {
    let mut result = ImportResult::default();
    let content = content.trim_start_matches('\u{feff}').trim_start();
    let value: serde_json::Value = match serde_json::from_str(content) {
        Ok(value) => value,
        Err(error) => {
            result.warnings.push(format!("invalid JSON: {error}"));
            return result;
        }
    };
    let servers: Vec<Server> = if value.is_array() {
        let items = value.as_array().cloned().unwrap_or_default();
        match serde_json::from_value::<Vec<Server>>(serde_json::Value::Array(items.clone())) {
            Ok(servers) => servers,
            Err(error) => {
                // 1Remote/PRemoteM exports use PascalCase and no protocol tag.
                let legacy = legacy_servers(&items, &mut result.warnings);
                if legacy.is_empty() {
                    result
                        .warnings
                        .push(format!("invalid server array: {error}"));
                }
                legacy
            }
        }
    } else {
        match serde_json::from_value::<ExportEnvelope>(value) {
            Ok(envelope) => envelope.servers,
            Err(error) => {
                result
                    .warnings
                    .push(format!("invalid export envelope: {error}"));
                Vec::new()
            }
        }
    };
    for mut server in servers {
        server.base_mut().id = new_id();
        server.base_mut().session_id = None;
        server.normalize();
        let issues = server.validate();
        if issues.is_empty() {
            result.servers.push(server);
        } else {
            result.warnings.push(format!(
                "skipped '{}': {}",
                server.base().display_name,
                issues
                    .iter()
                    .map(|issue| format!("{}: {}", issue.field, issue.message))
                    .collect::<Vec<_>>()
                    .join("; ")
            ));
        }
    }
    result
}

/// Minimal mRemoteNG CSV importer (P2-62). Unknown protocols are reported.
pub fn import_mremoteng_csv(content: &str) -> ImportResult {
    let mut result = ImportResult::default();
    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(content.as_bytes());
    let headers = match reader.headers() {
        Ok(headers) => headers
            .iter()
            .enumerate()
            .map(|(index, header)| (header.to_lowercase(), index))
            .collect::<std::collections::HashMap<_, _>>(),
        Err(error) => {
            result.warnings.push(format!("invalid CSV header: {error}"));
            return result;
        }
    };
    if headers.is_empty() {
        result.warnings.push("CSV has no header row".to_string());
        return result;
    }
    let get = |record: &csv::StringRecord, key: &str| -> Option<String> {
        headers
            .get(key)
            .and_then(|index| record.get(*index))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    };
    for (index, record) in reader.records().enumerate() {
        let Ok(record) = record else {
            result
                .warnings
                .push(format!("row {} could not be parsed", index + 1));
            continue;
        };
        let name = get(&record, "name").unwrap_or_else(|| format!("Imported {}", index + 1));
        let protocol = get(&record, "protocol").unwrap_or_default().to_uppercase();
        let host = get(&record, "hostname")
            .or_else(|| get(&record, "host"))
            .unwrap_or_default();
        let port = get(&record, "port")
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(0);
        let username = get(&record, "username").unwrap_or_default();
        let password = get(&record, "password").unwrap_or_default();
        let domain = get(&record, "domain").unwrap_or_default();
        let mut server = match protocol.as_str() {
            "RDP" => Server::new_rdp(
                name.clone(),
                host.clone(),
                if port == 0 { 3389 } else { port },
            ),
            "VNC" => Server::new_vnc(
                name.clone(),
                host.clone(),
                if port == 0 { 5900 } else { port },
            ),
            "SSH" | "SSH2" => Server::new_ssh(name.clone(), host.clone(), username.clone()),
            "TELNET" => Server::new_telnet(
                name.clone(),
                host.clone(),
                if port == 0 { 23 } else { port },
            ),
            "SFTP" => Server::new_sftp(name.clone(), host.clone(), username.clone()),
            "FTP" => Server::new_ftp(name.clone(), host.clone(), username.clone()),
            _ => {
                result.warnings.push(format!(
                    "skipped row {}: unsupported protocol '{protocol}'",
                    index + 1
                ));
                continue;
            }
        };
        if let Some(credentials) = server.credentials_mut() {
            if credentials.username.is_empty() {
                credentials.username = username;
            }
            if !password.is_empty() {
                credentials.password = password.into();
            }
        }
        if let Server::Rdp(rdp) = &mut server {
            rdp.domain = domain;
        }
        if let Some(description) = get(&record, "description") {
            server.base_mut().note = description;
        }
        if let Some(tags) = get(&record, "tags") {
            server.base_mut().tags = tags.split([';', ',']).map(str::to_string).collect();
        }
        if let Some(panel) = get(&record, "panel") {
            server.base_mut().tree_nodes = panel
                .split(['\\', '/'])
                .filter(|segment| !segment.is_empty())
                .map(str::to_string)
                .collect();
        }
        server.normalize();
        let issues = server.validate();
        if issues.is_empty() {
            result.servers.push(server);
        } else {
            result.warnings.push(format!(
                "skipped row {}: {}",
                index + 1,
                issues
                    .iter()
                    .map(|issue| issue.message.clone())
                    .collect::<Vec<_>>()
                    .join("; ")
            ));
        }
    }
    result
}

/// Imports one `.rdp` file. Password must be resolved by the caller (e.g. a
/// TERMSRV credential) because the DPAPI blob is not portable.
pub fn import_rdp(content: &str, display_name: &str, password: Option<String>) -> ImportResult {
    let mut result = ImportResult::default();
    match RdpFile::parse(content) {
        Ok(file) => {
            let mut server = Server::Rdp(file.to_rdp_server(display_name, password));
            server.base_mut().id = new_id();
            server.normalize();
            let issues = server.validate();
            if issues.is_empty() {
                result.servers.push(server);
            } else {
                result.warnings.push(
                    issues
                        .iter()
                        .map(|issue| format!("{}: {}", issue.field, issue.message))
                        .collect::<Vec<_>>()
                        .join("; "),
                );
            }
        }
        Err(error) => result.warnings.push(error.to_string()),
    }
    result
}

pub fn export_rdp(server: &Server, password_blob: Option<&[u8]>) -> AppResult<String> {
    match server {
        Server::Rdp(rdp) => Ok(RdpFile::from_rdp_server(rdp, password_blob).to_string()),
        _ => Err(AppError::validation(
            "only RDP servers can be exported as .rdp",
        )),
    }
}

fn legacy_servers(items: &[serde_json::Value], warnings: &mut Vec<String>) -> Vec<Server> {
    let mut servers = Vec::new();
    for (index, item) in items.iter().enumerate() {
        match legacy_value_to_server(item) {
            Ok(server) => servers.push(server),
            Err(error) => warnings.push(format!("row {}: {error}", index + 1)),
        }
    }
    servers
}

pub fn legacy_value_to_server(value: &serde_json::Value) -> Result<Server, String> {
    let protocol = string_field(value, &["Protocol", "protocol"])
        .ok_or_else(|| "missing 'Protocol' (not an H-Remote export?)".to_string())?
        .to_ascii_uppercase();
    let address = string_field(value, &["Address", "address"]).unwrap_or_default();
    let port = number_field(value, &["Port", "port"]).unwrap_or(0) as u16;
    let username = string_field(value, &["UserName", "Username", "username"]).unwrap_or_default();
    let display_name = string_field(value, &["DisplayName", "displayName", "Name"])
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| format!("{protocol} {address}"));
    let mut server = match protocol.as_str() {
        "RDP" => Server::new_rdp(display_name, address, if port == 0 { 3389 } else { port }),
        "VNC" => Server::new_vnc(display_name, address, if port == 0 { 5900 } else { port }),
        "SSH" => Server::new_ssh(display_name, address, username.clone()),
        "TELNET" => Server::new_telnet(display_name, address, if port == 0 { 23 } else { port }),
        "SERIAL" => Server::new_serial(
            display_name,
            string_field(value, &["SerialPort", "serialPort"]).unwrap_or_else(|| "COM1".into()),
        ),
        "SFTP" => Server::new_sftp(display_name, address, username.clone()),
        "FTP" => Server::new_ftp(display_name, address, username.clone()),
        "REMOTEAPP" => Server::new_remote_app(
            display_name,
            address,
            string_field(
                value,
                &["RemoteApplicationProgram", "remoteApplicationProgram"],
            )
            .unwrap_or_default(),
        ),
        "LOCALAPP" => Server::new_local_app(
            display_name,
            string_field(value, &["ExePath", "exePath"]).unwrap_or_default(),
        ),
        "CUSTOM" => Server::new_custom(
            display_name,
            string_field(value, &["CustomProtocolName", "Protocol"]).unwrap_or_default(),
        ),
        "DUMMY" => Server::new_dummy(display_name),
        other => return Err(format!("unsupported legacy protocol '{other}'")),
    };

    {
        let base = server.base_mut();
        if let Some(value) = string_field(value, &["Note", "note"]) {
            base.note = value;
        }
        base.tags = string_array(value, &["Tags", "tags"]);
        base.tree_nodes = string_array(value, &["TreeNodes", "treeNodes"]);
        base.icon_base64 = string_field(value, &["IconBase64", "iconBase64"]).map(|icon| {
            icon.split_once(',')
                .map(|(_, data)| data.to_string())
                .unwrap_or(icon)
        });
        if let Some(color) = string_field(value, &["ColorHex", "colorHex"]) {
            base.color_hex = color;
        }
        base.command_before_connected =
            string_field(value, &["CommandBeforeConnected", "commandBeforeConnected"])
                .unwrap_or_default();
        base.hide_command_before_connected_window = bool_field(
            value,
            &[
                "HideCommandBeforeConnectedWindow",
                "hideCommandBeforeConnectedWindow",
            ],
        )
        .unwrap_or(false);
        base.command_after_disconnected = string_field(
            value,
            &["CommandAfterDisconnected", "commandAfterDisconnected"],
        )
        .unwrap_or_default();
        base.selected_runner_name =
            string_field(value, &["SelectedRunnerName", "selectedRunnerName"])
                .filter(|name| !name.trim().is_empty());
        base.always_open_in_new_tab_window = bool_field(
            value,
            &["AlwaysOpenInNewTabWindow", "alwaysOpenInNewTabWindow"],
        )
        .unwrap_or(false);
    }

    if let Some(credentials) = server.credentials_mut() {
        credentials.username = username;
        if let Some(password) = string_field(value, &["Password", "password"])
            && !looks_encrypted(&password)
        {
            credentials.password = password.into();
        }
        credentials.private_key =
            string_field(value, &["PrivateKey", "privateKey"]).filter(|key| !key.is_empty());
        credentials.inherited_credential_name = string_field(
            value,
            &["InheritedCredentialName", "inheritedCredentialName"],
        )
        .filter(|name| !name.is_empty());
        credentials.ask_password_when_connect =
            bool_field(value, &["AskPasswordWhenConnect", "askPasswordWhenConnect"])
                .unwrap_or(false);
    }

    match &mut server {
        Server::Rdp(rdp) => {
            rdp.domain = string_field(value, &["Domain", "domain"]).unwrap_or_default();
            rdp.is_administrative_purposes = bool_field(
                value,
                &["IsAdministrativePurposes", "isAdministrativePurposes"],
            )
            .unwrap_or(false);
            rdp.load_balance_info =
                string_field(value, &["LoadBalanceInfo", "loadBalanceInfo"]).unwrap_or_default();
            rdp.rdp_width = number_field(value, &["RdpWidth", "rdpWidth"])
                .filter(|width| *width > 0)
                .unwrap_or(1920) as u32;
            rdp.rdp_height = number_field(value, &["RdpHeight", "rdpHeight"])
                .filter(|height| *height > 0)
                .unwrap_or(1080) as u32;
            rdp.is_conn_with_full_screen =
                bool_field(value, &["IsConnWithFullScreen", "isConnWithFullScreen"])
                    .unwrap_or(false);
            rdp.rdp_full_screen_flag =
                match number_field(value, &["RdpFullScreenFlag", "rdpFullScreenFlag"]) {
                    Some(1) => crate::server::RdpFullScreenFlag::EnableFullScreen,
                    Some(2) => crate::server::RdpFullScreenFlag::EnableFullAllScreens,
                    _ => crate::server::RdpFullScreenFlag::Disable,
                };
            rdp.rdp_window_resize_mode =
                match number_field(value, &["RdpWindowResizeMode", "rdpWindowResizeMode"]) {
                    Some(1) => crate::server::RdpWindowResizeMode::Stretch,
                    Some(2) => crate::server::RdpWindowResizeMode::Fixed,
                    Some(3) => crate::server::RdpWindowResizeMode::StretchFullScreen,
                    Some(4) => crate::server::RdpWindowResizeMode::FixedFullScreen,
                    _ => crate::server::RdpWindowResizeMode::AutoResize,
                };
            rdp.display_performance =
                match number_field(value, &["DisplayPerformance", "displayPerformance"]) {
                    Some(1) => crate::server::DisplayPerformance::Low,
                    Some(2) => crate::server::DisplayPerformance::Middle,
                    Some(3) => crate::server::DisplayPerformance::High,
                    _ => crate::server::DisplayPerformance::Auto,
                };
            rdp.audio_redirection_mode =
                match number_field(value, &["AudioRedirectionMode", "audioRedirectionMode"]) {
                    Some(1) => crate::server::AudioRedirectionMode::LeaveOnRemote,
                    Some(2) => crate::server::AudioRedirectionMode::Disabled,
                    _ => crate::server::AudioRedirectionMode::RedirectToLocal,
                };
            rdp.audio_quality_mode =
                match number_field(value, &["AudioQualityMode", "audioQualityMode"]) {
                    Some(1) => crate::server::AudioQualityMode::Medium,
                    Some(2) => crate::server::AudioQualityMode::High,
                    _ => crate::server::AudioQualityMode::Dynamic,
                };
            rdp.enable_clipboard =
                bool_field(value, &["EnableClipboard", "enableClipboard"]).unwrap_or(true);
            rdp.enable_disk_drives =
                bool_field(value, &["EnableDiskDrives", "enableDiskDrives"]).unwrap_or(false);
            rdp.enable_redirect_drives_plug_in = bool_field(
                value,
                &["EnableRedirectDrivesPlugIn", "enableRedirectDrivesPlugIn"],
            )
            .unwrap_or(false);
            rdp.enable_redirect_cameras =
                bool_field(value, &["EnableRedirectCameras", "enableRedirectCameras"])
                    .unwrap_or(false);
            rdp.enable_key_combinations =
                bool_field(value, &["EnableKeyCombinations", "enableKeyCombinations"])
                    .unwrap_or(true);
            rdp.enable_audio_capture =
                bool_field(value, &["EnableAudioCapture", "enableAudioCapture"]).unwrap_or(false);
            rdp.enable_ports = bool_field(value, &["EnablePorts", "enablePorts"]).unwrap_or(false);
            rdp.enable_printers =
                bool_field(value, &["EnablePrinters", "enablePrinters"]).unwrap_or(false);
            rdp.enable_smart_cards_and_win_hello = bool_field(
                value,
                &["EnableSmartCardsAndWinHello", "enableSmartCardsAndWinHello"],
            )
            .unwrap_or(false);
            rdp.mstsc_mode_enabled =
                bool_field(value, &["MstscModeEnabled", "mstscModeEnabled"]).unwrap_or(false);
            rdp.rdp_file_additional_settings = string_field(
                value,
                &["RdpFileAdditionalSettings", "rdpFileAdditionalSettings"],
            )
            .unwrap_or_default();
            rdp.gateway_mode = match number_field(value, &["GatewayMode", "gatewayMode"]) {
                Some(1) => crate::server::GatewayMode::UseTheseGatewayServerSettings,
                Some(2) => crate::server::GatewayMode::AutomaticallyDetectGatewayServerSettings,
                _ => crate::server::GatewayMode::DoNotUseGateway,
            };
            rdp.gateway_host_name =
                string_field(value, &["GatewayHostName", "gatewayHostName"]).unwrap_or_default();
            if rdp.gateway_host_name.trim().is_empty() {
                rdp.gateway_mode = crate::server::GatewayMode::DoNotUseGateway;
            }
            rdp.gateway_user_name =
                string_field(value, &["GatewayUserName", "gatewayUserName"]).unwrap_or_default();
            if let Some(password) = string_field(value, &["GatewayPassword", "gatewayPassword"]) {
                rdp.gateway_password = password.into();
            }
            rdp.gateway_logon_method =
                match number_field(value, &["GatewayLogonMethod", "gatewayLogonMethod"]) {
                    Some(1) => crate::protocol::GatewayLogonMethod::SmartCard,
                    _ => crate::protocol::GatewayLogonMethod::Password,
                };
            rdp.gateway_bypass_for_local_address = bool_field(
                value,
                &[
                    "GatewayBypassForLocalAddress",
                    "gatewayBypassForLocalAddress",
                ],
            )
            .unwrap_or(true);
        }
        Server::Vnc(vnc) => {
            vnc.vnc_window_resize_mode =
                match number_field(value, &["VncWindowResizeMode", "vncWindowResizeMode"]) {
                    Some(1) => crate::server::VncWindowResizeMode::Fixed,
                    _ => crate::server::VncWindowResizeMode::Stretch,
                };
        }
        Server::Ssh(ssh) => {
            ssh.ssh_version = string_field(value, &["SshVersion", "sshVersion"])
                .unwrap_or_else(|| "SSH-2".into());
            ssh.startup_auto_command =
                string_field(value, &["StartupAutoCommand", "startupAutoCommand"])
                    .unwrap_or_default();
            ssh.external_session_config_path = string_field(
                value,
                &["ExternalSessionConfigPath", "externalSessionConfigPath"],
            )
            .filter(|path| !path.is_empty());
            ssh.external_kitty_session_config_path = string_field(
                value,
                &[
                    "ExternalKittySessionConfigPath",
                    "externalKittySessionConfigPath",
                ],
            )
            .filter(|path| !path.is_empty());
            ssh.open_sftp_on_connected =
                bool_field(value, &["OpenSftpOnConnected", "openSftpOnConnected"]).unwrap_or(false);
        }
        Server::Telnet(telnet) => {
            telnet.startup_auto_command =
                string_field(value, &["StartupAutoCommand", "startupAutoCommand"])
                    .unwrap_or_default();
            telnet.external_session_config_path = string_field(
                value,
                &["ExternalSessionConfigPath", "externalSessionConfigPath"],
            )
            .filter(|path| !path.is_empty());
            telnet.external_kitty_session_config_path = string_field(
                value,
                &[
                    "ExternalKittySessionConfigPath",
                    "externalKittySessionConfigPath",
                ],
            )
            .filter(|path| !path.is_empty());
        }
        Server::Serial(serial) => {
            serial.bit_rate = number_field(value, &["BitRate", "bitRate"])
                .filter(|value| *value > 0)
                .unwrap_or(9600) as u32;
            serial.data_bits = number_field(value, &["DataBits", "dataBits"])
                .filter(|value| (5..=8).contains(value))
                .unwrap_or(8) as u8;
            serial.stop_bits = number_field(value, &["StopBits", "stopBits"])
                .filter(|value| (1..=2).contains(value))
                .unwrap_or(1) as u8;
            serial.parity = match number_field(value, &["Parity", "parity"]) {
                Some(1) => crate::server::SerialParity::Odd,
                Some(2) => crate::server::SerialParity::Even,
                Some(3) => crate::server::SerialParity::Mark,
                Some(4) => crate::server::SerialParity::Space,
                _ => crate::server::SerialParity::None,
            };
            serial.flow_control = match number_field(value, &["FlowControl", "flowControl"]) {
                Some(1) => crate::server::SerialFlowControl::XOnXOff,
                Some(2) => crate::server::SerialFlowControl::RequestToSend,
                Some(3) => crate::server::SerialFlowControl::RequestToSendXOnXOff,
                _ => crate::server::SerialFlowControl::None,
            };
            serial.startup_auto_command =
                string_field(value, &["StartupAutoCommand", "startupAutoCommand"])
                    .unwrap_or_default();
        }
        Server::Sftp(sftp) => {
            sftp.startup_path =
                string_field(value, &["StartupPath", "startupPath"]).unwrap_or_default();
        }
        Server::Ftp(ftp) => {
            ftp.startup_path =
                string_field(value, &["StartupPath", "startupPath"]).unwrap_or_default();
        }
        Server::RemoteApp(app) => {
            app.remote_application_name =
                string_field(value, &["RemoteApplicationName", "remoteApplicationName"])
                    .unwrap_or_default();
            if let Some(program) = string_field(
                value,
                &["RemoteApplicationProgram", "remoteApplicationProgram"],
            ) && !program.is_empty()
            {
                app.remote_application_program = program;
            }
            app.audio_redirection_mode =
                match number_field(value, &["AudioRedirectionMode", "audioRedirectionMode"]) {
                    Some(1) => crate::server::AudioRedirectionMode::LeaveOnRemote,
                    Some(2) => crate::server::AudioRedirectionMode::Disabled,
                    _ => crate::server::AudioRedirectionMode::RedirectToLocal,
                };
            app.rdp_file_additional_settings = string_field(
                value,
                &["RdpFileAdditionalSettings", "rdpFileAdditionalSettings"],
            )
            .unwrap_or_default();
        }
        Server::LocalApp(app) => {
            if let Some(exe) = string_field(value, &["ExePath", "exePath"])
                && !exe.is_empty()
            {
                app.exe_path = exe;
            }
            app.arguments = string_field(value, &["Arguments", "arguments"]).unwrap_or_default();
            app.run_with_hosting =
                bool_field(value, &["RunWithHosting", "runWithHosting"]).unwrap_or(false);
        }
        Server::Custom(custom) => {
            if let Some(name) = string_field(value, &["CustomProtocolName", "customProtocolName"])
                && !name.is_empty()
            {
                custom.custom_protocol_name = name;
            }
        }
        _ => {}
    }

    server.normalize();
    Ok(server)
}

fn string_field(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(serde_json::Value::as_str))
        .map(str::to_string)
}

fn number_field(value: &serde_json::Value, keys: &[&str]) -> Option<i64> {
    keys.iter().find_map(|key| {
        let field = value.get(*key)?;
        field
            .as_i64()
            .or_else(|| field.as_str().and_then(|value| value.trim().parse().ok()))
    })
}

fn bool_field(value: &serde_json::Value, keys: &[&str]) -> Option<bool> {
    keys.iter().find_map(|key| {
        let field = value.get(*key)?;
        field.as_bool().or_else(|| {
            field.as_i64().map(|value| value != 0).or_else(|| {
                field.as_str().map(|value| {
                    matches!(
                        value.trim().to_ascii_lowercase().as_str(),
                        "1" | "true" | "yes"
                    )
                })
            })
        })
    })
}

fn string_array(value: &serde_json::Value, keys: &[&str]) -> Vec<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(serde_json::Value::as_array))
        .map(|items| {
            items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn looks_encrypted(value: &str) -> bool {
    value.starts_with("v1:") || value.starts_with("u3") || value.starts_with("AQAAANCM")
}

fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_export_and_import_assign_new_ids() {
        let server = Server::new_ssh("SSH", "host", "root");
        let original_id = server.base().id.clone();
        let json = export_json(&[server]).unwrap();
        let result = import_json(&json);
        assert!(result.warnings.is_empty());
        assert_eq!(result.servers.len(), 1);
        assert_ne!(result.servers[0].base().id, original_id);
    }

    #[test]
    fn json_import_rejects_invalid_payloads() {
        assert!(!import_json("{broken").warnings.is_empty());
        let result = import_json(r#"{"servers":[]}"#);
        assert!(result.servers.is_empty());
    }

    #[test]
    fn mremoteng_csv_maps_protocols_and_fields() {
        let csv = "Name,Protocol,Hostname,Port,Username,Password,Domain,Description,Tags,Panel\n\
                   Web,RDP,10.0.0.5,3390,admin,secret,CORP,Primary,prod;web,Prod\\EU\n\
                   Box,SSH2,10.0.0.6,22,root,pass,,,,\n\
                   Odd,XYZ,host,1,,,,,,";
        let result = import_mremoteng_csv(csv);
        assert_eq!(result.servers.len(), 2);
        assert_eq!(result.warnings.len(), 1);
        match &result.servers[0] {
            Server::Rdp(rdp) => {
                assert_eq!(rdp.address.port, 3390);
                assert_eq!(rdp.domain, "CORP");
                assert_eq!(rdp.credentials.username, "admin");
                assert_eq!(rdp.base.tags, vec!["prod", "web"]);
                assert_eq!(rdp.base.tree_nodes, vec!["Prod", "EU"]);
            }
            _ => panic!("expected RDP"),
        }
        match &result.servers[1] {
            Server::Ssh(ssh) => assert_eq!(ssh.credentials.username, "root"),
            _ => panic!("expected SSH"),
        }
    }

    #[test]
    fn rdp_import_and_export_round_trip() {
        let server = Server::new_rdp("Desktop", "10.0.0.9", 3390);
        let rdp = export_rdp(&server, None).unwrap();
        let result = import_rdp(&rdp, "Imported", Some("pw".into()));
        assert_eq!(result.servers.len(), 1);
        match &result.servers[0] {
            Server::Rdp(imported) => {
                assert_eq!(imported.address.address, "10.0.0.9");
                assert_eq!(imported.address.port, 3390);
                assert_eq!(imported.credentials.password.expose(), "pw");
            }
            _ => panic!("expected RDP"),
        }
        assert!(export_rdp(&Server::new_ssh("x", "h", "u"), None).is_err());
    }

    #[test]
    fn legacy_1remote_export_is_detected_and_mapped() {
        let json = r#"[
          {
            "Protocol": "RDP",
            "DisplayName": "C.NY_Test",
            "Address": "172.19.194.139",
            "Port": "3390",
            "UserName": "Administrator",
            "Password": "A3jhSt$uNR",
            "Domain": "CORP",
            "RdpWidth": 1024,
            "RdpHeight": 768,
            "RdpWindowResizeMode": 2,
            "AudioRedirectionMode": 1,
            "EnableClipboard": false,
            "GatewayMode": 1,
            "GatewayHostName": "gw.example",
            "GatewayUserName": "gwuser",
            "Tags": ["prod"],
            "TreeNodes": ["PS"],
            "Note": "legacy"
          },
          {
            "Protocol": "VNC",
            "DisplayName": "Lab",
            "Address": "10.0.0.9",
            "Port": 5901,
            "Password": "vncpass",
            "VncWindowResizeMode": 1
          },
          {
            "Protocol": "SSH",
            "DisplayName": "Box",
            "Address": "10.0.0.10",
            "Port": 22,
            "UserName": "root",
            "SshVersion": "SSH-2"
          }
        ]"#;
        let result = import_json(json);
        assert_eq!(result.warnings, Vec::<String>::new());
        assert_eq!(result.servers.len(), 3);
        match &result.servers[0] {
            Server::Rdp(rdp) => {
                assert_eq!(rdp.base.display_name, "C.NY_Test");
                assert_eq!(rdp.address.port, 3390);
                assert_eq!(rdp.credentials.password.expose(), "A3jhSt$uNR");
                assert_eq!(rdp.domain, "CORP");
                assert_eq!(rdp.rdp_width, 1024);
                assert_eq!(rdp.rdp_height, 768);
                assert_eq!(
                    rdp.rdp_window_resize_mode,
                    crate::server::RdpWindowResizeMode::Fixed
                );
                assert_eq!(
                    rdp.audio_redirection_mode,
                    crate::server::AudioRedirectionMode::LeaveOnRemote
                );
                assert!(!rdp.enable_clipboard);
                assert_eq!(rdp.gateway_host_name, "gw.example");
                assert_eq!(rdp.base.tags, vec!["prod"]);
                assert_eq!(rdp.base.tree_nodes, vec!["PS"]);
                assert_eq!(rdp.base.note, "legacy");
            }
            _ => panic!("expected RDP"),
        }
        assert!(matches!(&result.servers[1], Server::Vnc(vnc) if vnc.address.port == 5901));
        assert!(
            matches!(&result.servers[2], Server::Ssh(ssh) if ssh.credentials.username == "root")
        );
    }

    #[test]
    fn utf8_bom_is_ignored() {
        let json = "\u{feff}[{\"Protocol\":\"RDP\",\"DisplayName\":\"BOM\",\"Address\":\"host\"}]";
        let result = import_json(json);
        assert_eq!(result.servers.len(), 1);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn legacy_rows_without_protocol_are_reported() {
        let result = import_json(r#"[{"DisplayName":"x","Address":"y"}]"#);
        assert!(result.servers.is_empty());
        assert!(
            result
                .warnings
                .iter()
                .any(|warning| warning.contains("Protocol"))
        );
    }

    #[test]
    fn empty_csv_is_reported() {
        let result = import_mremoteng_csv("");
        assert!(result.servers.is_empty());
        assert!(!result.warnings.is_empty());
    }
}
