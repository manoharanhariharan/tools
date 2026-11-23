//! Protocol-typed server models (Appendix A of the feature inventory).

use hr_core::{is_valid_id, new_id};
use serde::{Deserialize, Serialize};

use crate::credential::{Credential, EffectiveCredentials, SecretString};
use crate::folder::{migrate_tags_to_tree_nodes, validate_tree_path};
use crate::local_app::{AppArgument, MacroContext, expand_macros, render_arguments};
use crate::protocol::{
    AddressPortBase, GatewayLogonMethod, ProtocolBase, ProtocolKind, UserPasswordBase,
    is_valid_color,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationIssue {
    pub field: String,
    pub message: String,
}

impl ValidationIssue {
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum GatewayMode {
    #[default]
    DoNotUseGateway,
    UseTheseGatewayServerSettings,
    AutomaticallyDetectGatewayServerSettings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum RdpFullScreenFlag {
    #[default]
    Disable,
    EnableFullScreen,
    EnableFullAllScreens,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum RdpWindowResizeMode {
    #[default]
    AutoResize,
    Stretch,
    Fixed,
    StretchFullScreen,
    FixedFullScreen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum DisplayPerformance {
    #[default]
    Auto,
    Low,
    Middle,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum AudioRedirectionMode {
    #[default]
    RedirectToLocal,
    LeaveOnRemote,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum AudioQualityMode {
    #[default]
    Dynamic,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum VncWindowResizeMode {
    #[default]
    Stretch,
    Fixed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum SerialParity {
    #[default]
    None,
    Odd,
    Even,
    Mark,
    Space,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum SerialFlowControl {
    #[default]
    None,
    XOnXOff,
    RequestToSend,
    RequestToSendXOnXOff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ControlSettingType {
    #[default]
    String,
    Integer,
    Boolean,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct ProtocolControlSetting {
    pub name: String,
    pub value: String,
    pub value_type: ControlSettingType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RdpServer {
    #[serde(flatten)]
    pub base: ProtocolBase,
    #[serde(flatten)]
    pub address: AddressPortBase,
    #[serde(flatten)]
    pub credentials: UserPasswordBase,

    pub is_administrative_purposes: bool,
    pub domain: String,
    pub load_balance_info: String,
    pub gateway_mode: GatewayMode,
    pub gateway_bypass_for_local_address: bool,
    pub gateway_host_name: String,
    pub gateway_logon_method: GatewayLogonMethod,
    pub gateway_user_name: String,
    pub gateway_password: SecretString,

    pub rdp_full_screen_flag: RdpFullScreenFlag,
    pub is_conn_with_full_screen: bool,
    pub is_full_screen_with_connection_bar: bool,
    pub is_pin_the_connection_bar_by_default: bool,
    pub rdp_window_resize_mode: RdpWindowResizeMode,
    pub rdp_width: u32,
    pub rdp_height: u32,
    pub is_scale_factor_follow_system: bool,
    pub scale_factor_custom_value: u16,
    pub display_performance: DisplayPerformance,
    pub use_multimon: bool,

    pub enable_clipboard: bool,
    pub enable_disk_drives: bool,
    pub enable_redirect_drives_plug_in: bool,
    pub enable_redirect_cameras: bool,
    pub enable_key_combinations: bool,
    pub audio_redirection_mode: AudioRedirectionMode,
    pub audio_quality_mode: AudioQualityMode,
    pub enable_audio_capture: bool,
    pub enable_ports: bool,
    pub enable_printers: bool,
    pub enable_smart_cards_and_win_hello: bool,

    pub mstsc_mode_enabled: bool,
    pub rdp_file_additional_settings: String,
    pub rdp_control_additional_settings: Vec<ProtocolControlSetting>,
    pub last_full_screen_screen_index: Option<i32>,
}

impl Default for RdpServer {
    fn default() -> Self {
        Self {
            base: ProtocolBase::new(ProtocolKind::Rdp),
            address: AddressPortBase::new(3389),
            credentials: UserPasswordBase::default(),
            is_administrative_purposes: false,
            domain: String::new(),
            load_balance_info: String::new(),
            gateway_mode: GatewayMode::DoNotUseGateway,
            gateway_bypass_for_local_address: true,
            gateway_host_name: String::new(),
            gateway_logon_method: GatewayLogonMethod::Password,
            gateway_user_name: String::new(),
            gateway_password: SecretString::default(),
            rdp_full_screen_flag: RdpFullScreenFlag::Disable,
            is_conn_with_full_screen: false,
            is_full_screen_with_connection_bar: true,
            is_pin_the_connection_bar_by_default: false,
            rdp_window_resize_mode: RdpWindowResizeMode::AutoResize,
            rdp_width: 1920,
            rdp_height: 1080,
            is_scale_factor_follow_system: true,
            scale_factor_custom_value: 100,
            display_performance: DisplayPerformance::Auto,
            use_multimon: false,
            enable_clipboard: true,
            enable_disk_drives: true,
            enable_redirect_drives_plug_in: false,
            enable_redirect_cameras: false,
            enable_key_combinations: true,
            audio_redirection_mode: AudioRedirectionMode::RedirectToLocal,
            audio_quality_mode: AudioQualityMode::Dynamic,
            enable_audio_capture: false,
            enable_ports: false,
            enable_printers: true,
            enable_smart_cards_and_win_hello: false,
            mstsc_mode_enabled: false,
            rdp_file_additional_settings: String::new(),
            rdp_control_additional_settings: Vec::new(),
            last_full_screen_screen_index: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RemoteAppServer {
    #[serde(flatten)]
    pub base: ProtocolBase,
    #[serde(flatten)]
    pub address: AddressPortBase,
    #[serde(flatten)]
    pub credentials: UserPasswordBase,
    pub remote_application_name: String,
    pub remote_application_program: String,
    pub audio_redirection_mode: AudioRedirectionMode,
    pub audio_quality_mode: AudioQualityMode,
    pub rdp_file_additional_settings: String,
}

impl Default for RemoteAppServer {
    fn default() -> Self {
        Self {
            base: ProtocolBase::new(ProtocolKind::RemoteApp),
            address: AddressPortBase::new(3389),
            credentials: UserPasswordBase::default(),
            remote_application_name: String::new(),
            remote_application_program: String::new(),
            audio_redirection_mode: AudioRedirectionMode::RedirectToLocal,
            audio_quality_mode: AudioQualityMode::Dynamic,
            rdp_file_additional_settings: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct VncServer {
    #[serde(flatten)]
    pub base: ProtocolBase,
    #[serde(flatten)]
    pub address: AddressPortBase,
    #[serde(flatten)]
    pub credentials: UserPasswordBase,
    pub vnc_window_resize_mode: VncWindowResizeMode,
}

impl Default for VncServer {
    fn default() -> Self {
        Self {
            base: ProtocolBase::new(ProtocolKind::Vnc),
            address: AddressPortBase::new(5900),
            credentials: UserPasswordBase::default(),
            vnc_window_resize_mode: VncWindowResizeMode::Stretch,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SshServer {
    #[serde(flatten)]
    pub base: ProtocolBase,
    #[serde(flatten)]
    pub address: AddressPortBase,
    #[serde(flatten)]
    pub credentials: UserPasswordBase,
    pub ssh_version: String,
    pub startup_auto_command: String,
    pub external_session_config_path: Option<String>,
    pub external_kitty_session_config_path: Option<String>,
    pub open_sftp_on_connected: bool,
}

impl Default for SshServer {
    fn default() -> Self {
        Self {
            base: ProtocolBase::new(ProtocolKind::Ssh),
            address: AddressPortBase::new(22),
            credentials: UserPasswordBase::default(),
            ssh_version: "SSH-2".to_string(),
            startup_auto_command: String::new(),
            external_session_config_path: None,
            external_kitty_session_config_path: None,
            open_sftp_on_connected: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TelnetServer {
    #[serde(flatten)]
    pub base: ProtocolBase,
    #[serde(flatten)]
    pub address: AddressPortBase,
    #[serde(flatten)]
    pub credentials: UserPasswordBase,
    pub startup_auto_command: String,
    pub external_session_config_path: Option<String>,
    pub external_kitty_session_config_path: Option<String>,
}

impl Default for TelnetServer {
    fn default() -> Self {
        Self {
            base: ProtocolBase::new(ProtocolKind::Telnet),
            address: AddressPortBase::new(23),
            credentials: UserPasswordBase::default(),
            startup_auto_command: String::new(),
            external_session_config_path: None,
            external_kitty_session_config_path: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SerialServer {
    #[serde(flatten)]
    pub base: ProtocolBase,
    pub serial_port: String,
    pub bit_rate: u32,
    pub data_bits: u8,
    pub stop_bits: u8,
    pub parity: SerialParity,
    pub flow_control: SerialFlowControl,
    pub startup_auto_command: String,
    pub external_session_config_path: Option<String>,
    pub external_kitty_session_config_path: Option<String>,
}

impl Default for SerialServer {
    fn default() -> Self {
        Self {
            base: ProtocolBase::new(ProtocolKind::Serial),
            serial_port: "COM1".to_string(),
            bit_rate: 9600,
            data_bits: 8,
            stop_bits: 1,
            parity: SerialParity::None,
            flow_control: SerialFlowControl::None,
            startup_auto_command: String::new(),
            external_session_config_path: None,
            external_kitty_session_config_path: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SftpServer {
    #[serde(flatten)]
    pub base: ProtocolBase,
    #[serde(flatten)]
    pub address: AddressPortBase,
    #[serde(flatten)]
    pub credentials: UserPasswordBase,
    pub startup_path: String,
}

impl Default for SftpServer {
    fn default() -> Self {
        Self {
            base: ProtocolBase::new(ProtocolKind::Sftp),
            address: AddressPortBase::new(22),
            credentials: UserPasswordBase::default(),
            startup_path: "/".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FtpServer {
    #[serde(flatten)]
    pub base: ProtocolBase,
    #[serde(flatten)]
    pub address: AddressPortBase,
    #[serde(flatten)]
    pub credentials: UserPasswordBase,
    pub startup_path: String,
}

impl Default for FtpServer {
    fn default() -> Self {
        Self {
            base: ProtocolBase::new(ProtocolKind::Ftp),
            address: AddressPortBase::new(21),
            credentials: UserPasswordBase::default(),
            startup_path: "/".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LocalAppServer {
    #[serde(flatten)]
    pub base: ProtocolBase,
    pub exe_path: String,
    pub app_protocol_display_name: String,
    pub argument_list: Vec<AppArgument>,
    pub run_with_hosting: bool,
    /// Legacy untyped argument string from older profiles.
    pub arguments: String,
}

impl Default for LocalAppServer {
    fn default() -> Self {
        Self {
            base: ProtocolBase::new(ProtocolKind::LocalApp),
            exe_path: String::new(),
            app_protocol_display_name: String::new(),
            argument_list: Vec::new(),
            run_with_hosting: true,
            arguments: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CustomServer {
    #[serde(flatten)]
    pub base: ProtocolBase,
    #[serde(flatten)]
    pub address: AddressPortBase,
    #[serde(flatten)]
    pub credentials: UserPasswordBase,
    pub custom_protocol_name: String,
    /// Free-form fields validated by the custom protocol runner (Phase 10).
    pub extra_fields: serde_json::Value,
}

impl Default for CustomServer {
    fn default() -> Self {
        Self {
            base: ProtocolBase::new(ProtocolKind::Custom),
            address: AddressPortBase::default(),
            credentials: UserPasswordBase::default(),
            custom_protocol_name: String::new(),
            extra_fields: serde_json::Value::Object(Default::default()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DummyServer {
    #[serde(flatten)]
    pub base: ProtocolBase,
}

impl Default for DummyServer {
    fn default() -> Self {
        Self {
            base: ProtocolBase::new(ProtocolKind::Dummy),
        }
    }
}

/// The stored server model. The JSON tag is the protocol id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "protocol", rename_all = "camelCase")]
pub enum Server {
    Rdp(RdpServer),
    RemoteApp(RemoteAppServer),
    Vnc(VncServer),
    Ssh(SshServer),
    Telnet(TelnetServer),
    Serial(SerialServer),
    Sftp(SftpServer),
    Ftp(FtpServer),
    LocalApp(LocalAppServer),
    Custom(CustomServer),
    Dummy(DummyServer),
}

impl Server {
    pub fn new_rdp(display_name: impl Into<String>, address: impl Into<String>, port: u16) -> Self {
        let mut server = Self::Rdp(RdpServer::default());
        server.initialize(display_name, ProtocolKind::Rdp);
        if let Some(address_port) = server.address_mut() {
            address_port.address = address.into();
            address_port.port = port;
        }
        server.normalize();
        server
    }

    pub fn new_remote_app(
        display_name: impl Into<String>,
        address: impl Into<String>,
        program: impl Into<String>,
    ) -> Self {
        let mut server = Self::RemoteApp(RemoteAppServer::default());
        server.initialize(display_name, ProtocolKind::RemoteApp);
        if let Self::RemoteApp(inner) = &mut server {
            inner.address.address = address.into();
            inner.remote_application_program = program.into();
        }
        server.normalize();
        server
    }

    pub fn new_vnc(display_name: impl Into<String>, address: impl Into<String>, port: u16) -> Self {
        let mut server = Self::Vnc(VncServer::default());
        server.initialize(display_name, ProtocolKind::Vnc);
        if let Some(address_port) = server.address_mut() {
            address_port.address = address.into();
            address_port.port = port;
        }
        server.normalize();
        server
    }

    pub fn new_ssh(
        display_name: impl Into<String>,
        address: impl Into<String>,
        username: impl Into<String>,
    ) -> Self {
        let mut server = Self::Ssh(SshServer::default());
        server.initialize(display_name, ProtocolKind::Ssh);
        if let Self::Ssh(inner) = &mut server {
            inner.address.address = address.into();
            inner.credentials.username = username.into();
        }
        server.normalize();
        server
    }

    pub fn new_telnet(
        display_name: impl Into<String>,
        address: impl Into<String>,
        port: u16,
    ) -> Self {
        let mut server = Self::Telnet(TelnetServer::default());
        server.initialize(display_name, ProtocolKind::Telnet);
        if let Self::Telnet(inner) = &mut server {
            inner.address.address = address.into();
            inner.address.port = port;
        }
        server.normalize();
        server
    }

    pub fn new_serial(display_name: impl Into<String>, port: impl Into<String>) -> Self {
        let mut server = Self::Serial(SerialServer::default());
        server.initialize(display_name, ProtocolKind::Serial);
        if let Self::Serial(inner) = &mut server {
            inner.serial_port = port.into();
        }
        server.normalize();
        server
    }

    pub fn new_sftp(
        display_name: impl Into<String>,
        address: impl Into<String>,
        username: impl Into<String>,
    ) -> Self {
        let mut server = Self::Sftp(SftpServer::default());
        server.initialize(display_name, ProtocolKind::Sftp);
        if let Self::Sftp(inner) = &mut server {
            inner.address.address = address.into();
            inner.credentials.username = username.into();
        }
        server.normalize();
        server
    }

    pub fn new_ftp(
        display_name: impl Into<String>,
        address: impl Into<String>,
        username: impl Into<String>,
    ) -> Self {
        let mut server = Self::Ftp(FtpServer::default());
        server.initialize(display_name, ProtocolKind::Ftp);
        if let Self::Ftp(inner) = &mut server {
            inner.address.address = address.into();
            inner.credentials.username = username.into();
        }
        server.normalize();
        server
    }

    pub fn new_local_app(display_name: impl Into<String>, exe_path: impl Into<String>) -> Self {
        let mut server = Self::LocalApp(LocalAppServer::default());
        server.initialize(display_name, ProtocolKind::LocalApp);
        if let Self::LocalApp(inner) = &mut server {
            inner.exe_path = exe_path.into();
        }
        server.normalize();
        server
    }

    pub fn new_custom(display_name: impl Into<String>, protocol_name: impl Into<String>) -> Self {
        let mut server = Self::Custom(CustomServer::default());
        server.initialize(display_name, ProtocolKind::Custom);
        if let Self::Custom(inner) = &mut server {
            inner.custom_protocol_name = protocol_name.into();
        }
        server.normalize();
        server
    }

    pub fn new_dummy(display_name: impl Into<String>) -> Self {
        let mut server = Self::Dummy(DummyServer::default());
        server.initialize(display_name, ProtocolKind::Dummy);
        server.normalize();
        server
    }

    fn initialize(&mut self, display_name: impl Into<String>, kind: ProtocolKind) {
        *self.base_mut() = ProtocolBase::with_display_name(kind, display_name);
    }

    pub fn protocol(&self) -> ProtocolKind {
        match self {
            Self::Rdp(_) => ProtocolKind::Rdp,
            Self::RemoteApp(_) => ProtocolKind::RemoteApp,
            Self::Vnc(_) => ProtocolKind::Vnc,
            Self::Ssh(_) => ProtocolKind::Ssh,
            Self::Telnet(_) => ProtocolKind::Telnet,
            Self::Serial(_) => ProtocolKind::Serial,
            Self::Sftp(_) => ProtocolKind::Sftp,
            Self::Ftp(_) => ProtocolKind::Ftp,
            Self::LocalApp(_) => ProtocolKind::LocalApp,
            Self::Custom(_) => ProtocolKind::Custom,
            Self::Dummy(_) => ProtocolKind::Dummy,
        }
    }

    pub fn base(&self) -> &ProtocolBase {
        match self {
            Self::Rdp(inner) => &inner.base,
            Self::RemoteApp(inner) => &inner.base,
            Self::Vnc(inner) => &inner.base,
            Self::Ssh(inner) => &inner.base,
            Self::Telnet(inner) => &inner.base,
            Self::Serial(inner) => &inner.base,
            Self::Sftp(inner) => &inner.base,
            Self::Ftp(inner) => &inner.base,
            Self::LocalApp(inner) => &inner.base,
            Self::Custom(inner) => &inner.base,
            Self::Dummy(inner) => &inner.base,
        }
    }

    pub fn base_mut(&mut self) -> &mut ProtocolBase {
        match self {
            Self::Rdp(inner) => &mut inner.base,
            Self::RemoteApp(inner) => &mut inner.base,
            Self::Vnc(inner) => &mut inner.base,
            Self::Ssh(inner) => &mut inner.base,
            Self::Telnet(inner) => &mut inner.base,
            Self::Serial(inner) => &mut inner.base,
            Self::Sftp(inner) => &mut inner.base,
            Self::Ftp(inner) => &mut inner.base,
            Self::LocalApp(inner) => &mut inner.base,
            Self::Custom(inner) => &mut inner.base,
            Self::Dummy(inner) => &mut inner.base,
        }
    }

    pub fn address(&self) -> Option<&AddressPortBase> {
        match self {
            Self::Rdp(inner) => Some(&inner.address),
            Self::RemoteApp(inner) => Some(&inner.address),
            Self::Vnc(inner) => Some(&inner.address),
            Self::Ssh(inner) => Some(&inner.address),
            Self::Telnet(inner) => Some(&inner.address),
            Self::Sftp(inner) => Some(&inner.address),
            Self::Ftp(inner) => Some(&inner.address),
            Self::Custom(inner) => Some(&inner.address),
            Self::Serial(_) | Self::LocalApp(_) | Self::Dummy(_) => None,
        }
    }

    pub fn address_mut(&mut self) -> Option<&mut AddressPortBase> {
        match self {
            Self::Rdp(inner) => Some(&mut inner.address),
            Self::RemoteApp(inner) => Some(&mut inner.address),
            Self::Vnc(inner) => Some(&mut inner.address),
            Self::Ssh(inner) => Some(&mut inner.address),
            Self::Telnet(inner) => Some(&mut inner.address),
            Self::Sftp(inner) => Some(&mut inner.address),
            Self::Ftp(inner) => Some(&mut inner.address),
            Self::Custom(inner) => Some(&mut inner.address),
            Self::Serial(_) | Self::LocalApp(_) | Self::Dummy(_) => None,
        }
    }

    pub fn credentials(&self) -> Option<&UserPasswordBase> {
        match self {
            Self::Rdp(inner) => Some(&inner.credentials),
            Self::RemoteApp(inner) => Some(&inner.credentials),
            Self::Vnc(inner) => Some(&inner.credentials),
            Self::Ssh(inner) => Some(&inner.credentials),
            Self::Telnet(inner) => Some(&inner.credentials),
            Self::Sftp(inner) => Some(&inner.credentials),
            Self::Ftp(inner) => Some(&inner.credentials),
            Self::Custom(inner) => Some(&inner.credentials),
            Self::Serial(_) | Self::LocalApp(_) | Self::Dummy(_) => None,
        }
    }

    pub fn credentials_mut(&mut self) -> Option<&mut UserPasswordBase> {
        match self {
            Self::Rdp(inner) => Some(&mut inner.credentials),
            Self::RemoteApp(inner) => Some(&mut inner.credentials),
            Self::Vnc(inner) => Some(&mut inner.credentials),
            Self::Ssh(inner) => Some(&mut inner.credentials),
            Self::Telnet(inner) => Some(&mut inner.credentials),
            Self::Sftp(inner) => Some(&mut inner.credentials),
            Self::Ftp(inner) => Some(&mut inner.credentials),
            Self::Custom(inner) => Some(&mut inner.credentials),
            Self::Serial(_) | Self::LocalApp(_) | Self::Dummy(_) => None,
        }
    }

    pub fn effective_credentials(
        &self,
        credentials: &[Credential],
    ) -> Option<EffectiveCredentials> {
        let base = self.credentials()?;
        let inherited = base.inherited_credential_name.as_deref().and_then(|name| {
            credentials
                .iter()
                .find(|credential| credential.name.eq_ignore_ascii_case(name))
        });
        Some(base.resolve_effective(inherited))
    }

    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.base_mut().tags = tags.into_iter().map(Into::into).collect();
        self.normalize();
        self
    }

    pub fn with_folder(mut self, folder: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.base_mut().tree_nodes = folder.into_iter().map(Into::into).collect();
        self.normalize();
        self
    }

    pub fn duplicate(&self) -> Self {
        let mut duplicate = self.clone();
        *duplicate.base_mut() = self.base().duplicate();
        duplicate
    }

    /// Normalises shared and protocol-specific fields and derives the subtitle.
    pub fn normalize(&mut self) {
        self.base_mut().normalize();
        if let Some(address) = self.address_mut() {
            address.normalize();
        }
        if let Some(credentials) = self.credentials_mut() {
            credentials.normalize();
        }
        match self {
            Self::Rdp(inner) => {
                inner.gateway_host_name = inner.gateway_host_name.trim().to_string();
                inner.gateway_user_name = inner.gateway_user_name.trim().to_string();
                inner.domain = inner.domain.trim().to_string();
                inner.rdp_file_additional_settings = inner
                    .rdp_file_additional_settings
                    .trim_matches(['\r', '\n'])
                    .to_string();
                for setting in &mut inner.rdp_control_additional_settings {
                    setting.name = setting.name.trim().to_string();
                    setting.value = setting.value.trim().to_string();
                }
                inner
                    .rdp_control_additional_settings
                    .retain(|setting| !setting.name.is_empty());
                if inner.is_scale_factor_follow_system {
                    inner.scale_factor_custom_value = 100;
                } else {
                    inner.scale_factor_custom_value =
                        inner.scale_factor_custom_value.clamp(100, 300);
                }
            }
            Self::RemoteApp(inner) => {
                inner.remote_application_name = inner.remote_application_name.trim().to_string();
                inner.remote_application_program =
                    inner.remote_application_program.trim().to_string();
                inner.rdp_file_additional_settings = inner
                    .rdp_file_additional_settings
                    .trim_matches(['\r', '\n'])
                    .to_string();
            }
            Self::Ssh(inner) => {
                inner.ssh_version = inner.ssh_version.trim().to_string();
                inner.startup_auto_command = inner.startup_auto_command.trim().to_string();
            }
            Self::Telnet(inner) => {
                inner.startup_auto_command = inner.startup_auto_command.trim().to_string();
            }
            Self::Serial(inner) => {
                inner.serial_port = inner.serial_port.trim().to_uppercase();
                inner.startup_auto_command = inner.startup_auto_command.trim().to_string();
            }
            Self::Sftp(inner) => {
                inner.startup_path = normalize_remote_path(&inner.startup_path);
            }
            Self::Ftp(inner) => {
                inner.startup_path = normalize_remote_path(&inner.startup_path);
            }
            Self::LocalApp(inner) => {
                inner.exe_path = inner.exe_path.trim().to_string();
                inner.app_protocol_display_name =
                    inner.app_protocol_display_name.trim().to_string();
                for argument in &mut inner.argument_list {
                    argument.normalize();
                }
            }
            Self::Custom(inner) => {
                inner.custom_protocol_name = inner.custom_protocol_name.trim().to_string();
            }
            Self::Vnc(_) | Self::Dummy(_) => {}
        }

        let fallback = self.default_subtitle();
        {
            let base = self.base_mut();
            migrate_tags_to_tree_nodes(&base.tags, &mut base.tree_nodes);
            if base.display_name.trim().is_empty() {
                base.display_name = fallback.clone();
            }
            base.sub_title = fallback;
        }
    }

    pub fn default_subtitle(&self) -> String {
        match self {
            Self::Serial(inner) => format!("{}({})", inner.serial_port, inner.bit_rate),
            Self::LocalApp(inner) => inner.exe_path.clone(),
            Self::Dummy(_) => String::new(),
            Self::Rdp(_)
            | Self::RemoteApp(_)
            | Self::Vnc(_)
            | Self::Ssh(_)
            | Self::Telnet(_)
            | Self::Sftp(_)
            | Self::Ftp(_)
            | Self::Custom(_) => {
                let address = self.address().expect("address protocol");
                address.subtitle(
                    self.credentials()
                        .map(|credentials| credentials.username.as_str()),
                )
            }
        }
    }

    pub fn connection_id(&self) -> String {
        match self {
            Self::Rdp(inner) => format!(
                "rdp|{}|{}|{}",
                inner.address.address.to_lowercase(),
                inner.address.port,
                inner.credentials.username.to_lowercase()
            ),
            Self::RemoteApp(inner) => format!(
                "remoteapp|{}|{}|{}",
                inner.address.address.to_lowercase(),
                inner.address.port,
                inner.credentials.username.to_lowercase()
            ),
            Self::Vnc(inner) => format!(
                "vnc|{}|{}",
                inner.address.address.to_lowercase(),
                inner.address.port
            ),
            Self::Ssh(inner) => format!(
                "ssh|{}|{}|{}",
                inner.address.address.to_lowercase(),
                inner.address.port,
                inner.credentials.username.to_lowercase()
            ),
            Self::Telnet(inner) => format!(
                "telnet|{}|{}",
                inner.address.address.to_lowercase(),
                inner.address.port
            ),
            Self::Serial(inner) => format!(
                "serial|{}|{}",
                inner.serial_port.to_lowercase(),
                inner.bit_rate
            ),
            Self::Sftp(inner) => format!(
                "sftp|{}|{}|{}",
                inner.address.address.to_lowercase(),
                inner.address.port,
                inner.credentials.username.to_lowercase()
            ),
            Self::Ftp(inner) => format!(
                "ftp|{}|{}|{}",
                inner.address.address.to_lowercase(),
                inner.address.port,
                inner.credentials.username.to_lowercase()
            ),
            Self::LocalApp(inner) => format!("localapp|{}", inner.exe_path.to_lowercase()),
            Self::Custom(inner) => format!(
                "custom|{}|{}|{}|{}",
                inner.custom_protocol_name.to_lowercase(),
                inner.address.address.to_lowercase(),
                inner.address.port,
                inner.credentials.username.to_lowercase()
            ),
            Self::Dummy(inner) => format!("dummy|{}", inner.base.id),
        }
    }

    pub fn render_local_app_arguments(&self, context: &MacroContext) -> Option<Vec<String>> {
        match self {
            Self::LocalApp(inner) => Some(render_arguments(&inner.argument_list, context)),
            _ => None,
        }
    }

    pub fn expand_local_app_path(&self, context: &MacroContext) -> Option<String> {
        match self {
            Self::LocalApp(inner) => Some(expand_macros(&inner.exe_path, context)),
            _ => None,
        }
    }

    pub fn validate(&self) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();
        let base = self.base();
        if !is_valid_id(&base.id) {
            issues.push(ValidationIssue::new("id", "server id must be a ULID"));
        }
        if base.display_name.trim().is_empty() {
            issues.push(ValidationIssue::new(
                "displayName",
                "display name is required",
            ));
        }
        if !base.color_hex.is_empty() && !is_valid_color(&base.color_hex) {
            issues.push(ValidationIssue::new(
                "colorHex",
                "colour must be #RRGGBB or #AARRGGBB",
            ));
        }
        for issue in validate_tree_path(&base.tree_nodes) {
            issues.push(ValidationIssue::new("treeNodes", issue));
        }
        if let Some(icon) = base
            .icon_base64
            .as_deref()
            .filter(|icon| !icon.trim().is_empty())
        {
            if let Err(error) = crate::icon::IconData::decode(icon) {
                issues.push(ValidationIssue::new("iconBase64", error.to_string()));
            }
        }
        if let Some(address) = self.address() {
            if address.address.trim().is_empty() {
                issues.push(ValidationIssue::new("address", "address is required"));
            }
            if address.port == 0 {
                issues.push(ValidationIssue::new(
                    "port",
                    "port must be greater than zero",
                ));
            }
            for alternate in &address.alternate_credentials {
                for issue in alternate.validate() {
                    issues.push(ValidationIssue::new("alternateCredentials", issue));
                }
            }
        }
        if let Some(credentials) = self.credentials() {
            if let Some(issue) = credentials.credential_exclusivity_issue() {
                issues.push(ValidationIssue::new("password", issue));
            }
            if !credentials.is_private_key_all_ascii() {
                issues.push(ValidationIssue::new(
                    "privateKey",
                    "private key path must be ASCII",
                ));
            }
        }

        match self {
            Self::Rdp(inner) => {
                if !inner.is_scale_factor_follow_system
                    && !(100..=300).contains(&inner.scale_factor_custom_value)
                {
                    issues.push(ValidationIssue::new(
                        "scaleFactorCustomValue",
                        "custom scale must be between 100 and 300",
                    ));
                }
                if inner.rdp_width == 0 || inner.rdp_height == 0 {
                    issues.push(ValidationIssue::new(
                        "rdpWidth",
                        "fixed resolution must be greater than zero",
                    ));
                }
                if inner.gateway_mode == GatewayMode::UseTheseGatewayServerSettings
                    && inner.gateway_host_name.trim().is_empty()
                {
                    issues.push(ValidationIssue::new(
                        "gatewayHostName",
                        "gateway host is required when an explicit gateway is selected",
                    ));
                }
                if inner.gateway_mode != GatewayMode::DoNotUseGateway
                    && inner.gateway_logon_method == GatewayLogonMethod::Password
                    && inner.gateway_user_name.trim().is_empty()
                {
                    issues.push(ValidationIssue::new(
                        "gatewayUserName",
                        "gateway username is required for password logon",
                    ));
                }
                for setting in &inner.rdp_control_additional_settings {
                    if setting.name.trim().is_empty() {
                        issues.push(ValidationIssue::new(
                            "rdpControlAdditionalSettings",
                            "additional control setting name is required",
                        ));
                    }
                }
            }
            Self::RemoteApp(inner) => {
                if inner.remote_application_program.trim().is_empty() {
                    issues.push(ValidationIssue::new(
                        "remoteApplicationProgram",
                        "published application program is required",
                    ));
                }
            }
            Self::Ssh(inner) => {
                if inner.ssh_version.trim().is_empty() {
                    issues.push(ValidationIssue::new(
                        "sshVersion",
                        "SSH version is required",
                    ));
                }
            }
            Self::Serial(inner) => {
                if inner.serial_port.trim().is_empty() {
                    issues.push(ValidationIssue::new(
                        "serialPort",
                        "serial port is required",
                    ));
                }
                if inner.bit_rate == 0 {
                    issues.push(ValidationIssue::new(
                        "bitRate",
                        "bit rate must be greater than zero",
                    ));
                }
                if !(5..=8).contains(&inner.data_bits) {
                    issues.push(ValidationIssue::new("dataBits", "data bits must be 5-8"));
                }
                if !matches!(inner.stop_bits, 1 | 2) {
                    issues.push(ValidationIssue::new("stopBits", "stop bits must be 1 or 2"));
                }
            }
            Self::LocalApp(inner) => {
                if inner.exe_path.trim().is_empty() {
                    issues.push(ValidationIssue::new(
                        "exePath",
                        "executable path is required",
                    ));
                }
                for argument in &inner.argument_list {
                    for issue in argument.validate() {
                        issues.push(ValidationIssue::new("argumentList", issue));
                    }
                }
            }
            Self::Custom(inner) => {
                if inner.custom_protocol_name.trim().is_empty() {
                    issues.push(ValidationIssue::new(
                        "customProtocolName",
                        "custom protocol name is required",
                    ));
                } else if ProtocolKind::is_builtin_name(&inner.custom_protocol_name) {
                    issues.push(ValidationIssue::new(
                        "customProtocolName",
                        "built-in protocol names cannot be redefined",
                    ));
                }
            }
            Self::Vnc(_) | Self::Telnet(_) | Self::Sftp(_) | Self::Ftp(_) | Self::Dummy(_) => {}
        }

        issues
    }

    pub fn is_valid(&self) -> bool {
        self.validate().is_empty()
    }
}

fn normalize_remote_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        "/".to_string()
    } else if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    }
}

/// Creates one fully-populated server per protocol. Used by round-trip tests and
/// the developer seed command.
pub fn sample_servers() -> Vec<Server> {
    let mut rdp = match Server::new_rdp("RDP production", "rdp.example.com", 3389) {
        Server::Rdp(inner) => inner,
        _ => unreachable!(),
    };
    rdp.base.tags = vec!["prod".into(), "windows".into()];
    rdp.base.tree_nodes = vec!["Prod".into(), "EU".into()];
    rdp.credentials.username = "Administrator".into();
    rdp.domain = "CORP".into();
    rdp.gateway_mode = GatewayMode::UseTheseGatewayServerSettings;
    rdp.gateway_host_name = "gw.example.com".into();
    rdp.gateway_user_name = "gw-user".into();
    rdp.gateway_password = "gw-secret".into();
    rdp.enable_clipboard = true;
    rdp.enable_disk_drives = true;
    rdp.rdp_control_additional_settings = vec![ProtocolControlSetting {
        name: "AuthenticationLevel".into(),
        value: "2".into(),
        value_type: ControlSettingType::Integer,
    }];

    let mut remote_app =
        match Server::new_remote_app("Published app", "ra.example.com", "||Notepad") {
            Server::RemoteApp(inner) => inner,
            _ => unreachable!(),
        };
    remote_app.remote_application_name = "Notepad".into();
    remote_app.base.tags = vec!["apps".into()];

    let mut vnc = match Server::new_vnc("Lab VNC", "vnc.example.com", 5901) {
        Server::Vnc(inner) => inner,
        _ => unreachable!(),
    };
    vnc.credentials.password = "vnc-secret".into();
    vnc.base.color_hex = "#FF112233".into();

    let mut ssh = match Server::new_ssh("SSH bastion", "ssh.example.com", "root") {
        Server::Ssh(inner) => inner,
        _ => unreachable!(),
    };
    ssh.credentials.password = "ssh-secret".into();
    ssh.startup_auto_command = "uptime".into();
    ssh.open_sftp_on_connected = true;
    ssh.base.tree_nodes = vec!["Prod".into()];

    let mut telnet = match Server::new_telnet("Old router", "telnet.example.com", 23) {
        Server::Telnet(inner) => inner,
        _ => unreachable!(),
    };
    telnet.credentials.username = "admin".into();

    let mut serial = match Server::new_serial("Console", "COM3") {
        Server::Serial(inner) => inner,
        _ => unreachable!(),
    };
    serial.bit_rate = 115200;
    serial.parity = SerialParity::Even;

    let mut sftp = match Server::new_sftp("SFTP files", "files.example.com", "deploy") {
        Server::Sftp(inner) => inner,
        _ => unreachable!(),
    };
    sftp.credentials.password = "sftp-secret".into();
    sftp.startup_path = "/srv/data".into();

    let mut ftp = match Server::new_ftp("FTP drop", "ftp.example.com", "anonymous") {
        Server::Ftp(inner) => inner,
        _ => unreachable!(),
    };
    ftp.credentials.password = "ftp-secret".into();

    let mut local = match Server::new_local_app("Chrome", "C:\\Program Files\\Chrome\\chrome.exe") {
        Server::LocalApp(inner) => inner,
        _ => unreachable!(),
    };
    local.argument_list = crate::local_app::templates()
        .into_iter()
        .find(|template| template.name == "Chrome / Edge")
        .map(|template| template.arguments)
        .unwrap_or_default();
    local.base.tags = vec!["local".into()];

    let mut custom = match Server::new_custom("Custom runner", "MyProtocol") {
        Server::Custom(inner) => inner,
        _ => unreachable!(),
    };
    custom.address.address = "custom.example.com".into();
    custom.address.port = 9000;

    let mut dummy = match Server::new_dummy("Placeholder") {
        Server::Dummy(inner) => inner,
        _ => unreachable!(),
    };
    dummy.base.id = new_id();

    vec![
        Server::Rdp(rdp),
        Server::RemoteApp(remote_app),
        Server::Vnc(vnc),
        Server::Ssh(ssh),
        Server::Telnet(telnet),
        Server::Serial(serial),
        Server::Sftp(sftp),
        Server::Ftp(ftp),
        Server::LocalApp(local),
        Server::Custom(custom),
        Server::Dummy(dummy),
    ]
    .into_iter()
    .map(|mut server| {
        server.normalize();
        server
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_protocol_round_trips_through_json() {
        for server in sample_servers() {
            let json = serde_json::to_string(&server).expect("serialise server");
            let decoded: Server = serde_json::from_str(&json).expect("deserialise server");
            assert_eq!(decoded, server, "{} round trip", server.protocol().id());
        }
    }

    #[test]
    fn protocol_tag_is_stable() {
        let server = Server::new_ssh("a", "host", "root");
        let value = serde_json::to_value(&server).unwrap();
        assert_eq!(value["protocol"], "ssh");
        assert_eq!(value["address"], "host");
        assert_eq!(value["port"], 22);
        assert_eq!(value["username"], "root");
    }

    #[test]
    fn sample_servers_are_valid_after_normalisation_and_have_ids() {
        for server in sample_servers() {
            assert!(server.is_valid(), "{:?}", server.validate());
            assert!(is_valid_id(&server.base().id));
        }
    }

    #[test]
    fn subtitles_and_connection_ids_are_derived() {
        let ssh = Server::new_ssh("SSH", "Host.Example", "Root");
        assert_eq!(ssh.default_subtitle(), "Host.Example:22 (Root)");
        assert_eq!(ssh.connection_id(), "ssh|host.example|22|root");

        let serial = Server::new_serial("Console", "com3");
        assert_eq!(serial.default_subtitle(), "COM3(9600)");
        assert_eq!(serial.connection_id(), "serial|com3|9600");
    }

    #[test]
    fn legacy_tags_migrate_into_folder_paths() {
        let server = Server::new_ssh("SSH", "host", "root").with_tags(["prod", "linux"]);
        assert_eq!(server.base().tree_nodes, vec!["linux", "prod"]);
    }

    #[test]
    fn duplicate_changes_id_and_name() {
        let server = Server::new_rdp("Prod", "host", 3389);
        let duplicate = server.duplicate();
        assert_ne!(duplicate.base().id, server.base().id);
        assert_eq!(duplicate.base().display_name, "Prod (copy)");
        assert_eq!(duplicate.protocol(), server.protocol());
    }

    #[test]
    fn validation_rejects_bad_values() {
        let mut ssh = match Server::new_ssh("SSH", "host", "root") {
            Server::Ssh(inner) => inner,
            _ => unreachable!(),
        };
        ssh.credentials.password = "a".into();
        ssh.credentials.private_key = Some("C:\\key".into());
        ssh.address.port = 0;
        let server = Server::Ssh(ssh);
        let issues = server.validate();
        assert!(issues.iter().any(|issue| issue.field == "port"));
        assert!(issues.iter().any(|issue| issue.field == "password"));

        let mut custom = match Server::new_custom("x", "rdp") {
            Server::Custom(inner) => inner,
            _ => unreachable!(),
        };
        custom.custom_protocol_name = "rdp".into();
        assert!(
            Server::Custom(custom)
                .validate()
                .iter()
                .any(|issue| issue.message.contains("built-in"))
        );
    }

    #[test]
    fn rdp_gateway_and_scale_validation() {
        let mut rdp = match Server::new_rdp("RDP", "host", 3389) {
            Server::Rdp(inner) => inner,
            _ => unreachable!(),
        };
        rdp.gateway_mode = GatewayMode::UseTheseGatewayServerSettings;
        rdp.is_scale_factor_follow_system = false;
        rdp.scale_factor_custom_value = 50;
        let issues = Server::Rdp(rdp).validate();
        assert!(issues.iter().any(|issue| issue.field == "gatewayHostName"));
        assert!(
            issues
                .iter()
                .any(|issue| issue.field == "scaleFactorCustomValue")
        );
    }

    #[test]
    fn effective_credentials_find_inherited_vault_entries() {
        let mut vault = Credential::new("vault", "vault-user");
        vault.password = "vault-pass".into();
        let mut server = Server::new_ssh("SSH", "host", "");
        server.credentials_mut().unwrap().inherited_credential_name = Some("vault".into());
        let effective = server.effective_credentials(&[vault]).unwrap();
        assert_eq!(effective.username, "vault-user");
        assert_eq!(effective.password.expose(), "vault-pass");
    }
}
