//! Turns a stored RDP server into an immutable session plan.
//!
//! The plan is backend-neutral: the embedded IronRDP engine consumes it, and the
//! `mstsc` fallback turns it into a generated `.rdp` file. Unsupported
//! redirections are reported as warnings instead of silently disappearing.

use hr_core::{AppError, AppResult};
use hr_domain::credential::SecretString;
use hr_domain::{
    AudioQualityMode, AudioRedirectionMode, DisplayPerformance, GatewayLogonMethod, GatewayMode,
    ProtocolControlSetting, RdpWindowResizeMode, Server,
};
use serde::{Deserialize, Serialize};

pub const MIN_WIDTH: u32 = 640;
pub const MAX_WIDTH: u32 = 7680;
pub const MIN_HEIGHT: u32 = 480;
pub const MAX_HEIGHT: u32 = 4320;
pub const MIN_SCALE: u16 = 100;
pub const MAX_SCALE: u16 = 300;
pub const DEFAULT_RECONNECT_ATTEMPTS: u32 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum EngineChoice {
    #[default]
    Embedded,
    ExternalMstsc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconnectPolicy {
    pub max_attempts: u32,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self {
            max_attempts: DEFAULT_RECONNECT_ATTEMPTS,
            initial_delay_ms: 1_000,
            max_delay_ms: 8_000,
        }
    }
}

impl ReconnectPolicy {
    /// Exponential backoff capped at `max_delay_ms`.
    pub fn delay(&self, attempt: u32) -> std::time::Duration {
        let exponent = attempt.saturating_sub(1).min(16);
        let delay = self
            .initial_delay_ms
            .saturating_mul(1u64 << exponent)
            .min(self.max_delay_ms);
        std::time::Duration::from_millis(delay)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayPlan {
    pub width: u32,
    pub height: u32,
    pub scale_percent: u16,
    pub full_screen: bool,
    pub multimon: bool,
    pub resize_mode: RdpWindowResizeMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedirectPlan {
    pub clipboard: bool,
    pub disk_drives: bool,
    pub smart_cards: bool,
    pub audio: AudioRedirectionMode,
    pub audio_quality: AudioQualityMode,
    pub audio_capture: bool,
    pub printers: bool,
    pub ports: bool,
    pub cameras: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayPlan {
    pub endpoint: String,
    pub username: String,
    pub password: SecretString,
    pub logon_method: GatewayLogonMethod,
    pub bypass_for_local: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RdpPlan {
    pub server_id: String,
    pub server_name: String,
    pub address: String,
    pub port: u16,
    pub username: String,
    pub password: SecretString,
    pub domain: String,
    pub admin_session: bool,
    pub load_balance_info: String,
    pub display: DisplayPlan,
    pub redirect: RedirectPlan,
    pub gateway: Option<GatewayPlan>,
    pub performance: DisplayPerformance,
    pub additional_settings: String,
    pub control_properties: Vec<ProtocolControlSetting>,
    pub engine: EngineChoice,
    pub reconnect: ReconnectPolicy,
    /// Human-readable notes about settings the embedded engine cannot honour.
    pub warnings: Vec<String>,
}

impl RdpPlan {
    /// Builds a plan from an RDP server. `viewport` is the current session area
    /// used for auto-resize/full-screen modes.
    pub fn from_server(
        server: &Server,
        viewport: (u32, u32),
        device_scale: u16,
        reconnect: ReconnectPolicy,
    ) -> AppResult<Self> {
        let Server::Rdp(rdp) = server else {
            return Err(AppError::validation(
                "only RDP servers can start an RDP session",
            ));
        };
        if rdp.address.address.trim().is_empty() {
            return Err(AppError::validation("the server address is required"));
        }
        if rdp.address.port == 0 {
            return Err(AppError::validation("the server port is required"));
        }
        let display = DisplayPlan {
            width: clamp_dimension(rdp.rdp_width, MIN_WIDTH, MAX_WIDTH, 1920),
            height: clamp_dimension(rdp.rdp_height, MIN_HEIGHT, MAX_HEIGHT, 1080),
            scale_percent: resolve_scale(
                rdp.is_scale_factor_follow_system,
                device_scale,
                rdp.scale_factor_custom_value,
            ),
            full_screen: rdp.is_conn_with_full_screen
                || !matches!(
                    rdp.rdp_full_screen_flag,
                    hr_domain::RdpFullScreenFlag::Disable
                ),
            multimon: rdp.use_multimon,
            resize_mode: rdp.rdp_window_resize_mode,
        };
        let (width, height) = match display.resize_mode {
            RdpWindowResizeMode::AutoResize | RdpWindowResizeMode::Stretch => {
                let (viewport_width, viewport_height) = viewport;
                (
                    clamp_dimension(viewport_width, MIN_WIDTH, MAX_WIDTH, display.width),
                    clamp_dimension(viewport_height, MIN_HEIGHT, MAX_HEIGHT, display.height),
                )
            }
            RdpWindowResizeMode::Fixed
            | RdpWindowResizeMode::StretchFullScreen
            | RdpWindowResizeMode::FixedFullScreen => (display.width, display.height),
        };
        let display = DisplayPlan {
            width,
            height,
            ..display
        };
        let redirect = RedirectPlan {
            clipboard: rdp.enable_clipboard,
            disk_drives: rdp.enable_disk_drives,
            smart_cards: rdp.enable_smart_cards_and_win_hello,
            audio: rdp.audio_redirection_mode,
            audio_quality: rdp.audio_quality_mode,
            audio_capture: rdp.enable_audio_capture,
            printers: rdp.enable_printers,
            ports: rdp.enable_ports,
            cameras: rdp.enable_redirect_cameras,
        };
        let gateway = match rdp.gateway_mode {
            GatewayMode::DoNotUseGateway => None,
            GatewayMode::UseTheseGatewayServerSettings
            | GatewayMode::AutomaticallyDetectGatewayServerSettings => {
                let endpoint = rdp.gateway_host_name.trim();
                if endpoint.is_empty() {
                    return Err(AppError::validation(
                        "a gateway host is required when the gateway is enabled",
                    ));
                }
                Some(GatewayPlan {
                    endpoint: if endpoint.contains(':') {
                        endpoint.to_string()
                    } else {
                        format!("{endpoint}:443")
                    },
                    username: rdp.gateway_user_name.clone(),
                    password: rdp.gateway_password.clone(),
                    logon_method: rdp.gateway_logon_method,
                    bypass_for_local: rdp.gateway_bypass_for_local_address,
                })
            }
        };
        let mut warnings = Vec::new();
        if redirect.printers {
            warnings.push("Printer redirection requires the external mstsc engine.".to_string());
        }
        if redirect.ports {
            warnings.push("COM port redirection requires the external mstsc engine.".to_string());
        }
        if redirect.cameras {
            warnings.push("Camera redirection requires the external mstsc engine.".to_string());
        }
        if redirect.audio_capture {
            warnings.push(
                "Microphone redirection is not available in the embedded engine.".to_string(),
            );
        }
        if display.multimon {
            warnings.push(
                "Multi-monitor full screen is presented as a single canvas in this phase."
                    .to_string(),
            );
        }
        if rdp.is_administrative_purposes {
            warnings.push(
                "The /admin console flag is only sent by the external mstsc engine.".to_string(),
            );
        }
        if rdp.address.is_auto_alternate_address_switching {
            warnings.push(
                "Automatic alternate-address switching is not wired into the engine yet."
                    .to_string(),
            );
        }
        let engine = if rdp.mstsc_mode_enabled {
            EngineChoice::ExternalMstsc
        } else {
            EngineChoice::Embedded
        };
        Ok(Self {
            server_id: rdp.base.id.clone(),
            server_name: rdp.base.display_name.clone(),
            address: rdp.address.address.clone(),
            port: rdp.address.port,
            username: rdp.credentials.username.clone(),
            password: rdp.credentials.password.clone(),
            domain: rdp.domain.clone(),
            admin_session: rdp.is_administrative_purposes,
            load_balance_info: rdp.load_balance_info.clone(),
            display,
            redirect,
            gateway,
            performance: rdp.display_performance,
            additional_settings: rdp.rdp_file_additional_settings.clone(),
            control_properties: rdp.rdp_control_additional_settings.clone(),
            engine,
            reconnect,
            warnings,
        })
    }

    pub fn destination(&self) -> String {
        if self.address.contains(':') && !self.address.starts_with('[') {
            format!("[{}]:{}", self.address, self.port)
        } else {
            format!("{}:{}", self.address, self.port)
        }
    }

    /// Bits-per-pixel for the requested display performance preset.
    pub fn color_depth(&self) -> u32 {
        match self.performance {
            DisplayPerformance::Low => 16,
            DisplayPerformance::Auto | DisplayPerformance::Middle | DisplayPerformance::High => 32,
        }
    }

    pub fn codecs(&self) -> Vec<String> {
        match self.performance {
            // `ironrdp-pdu` parses codec names case-sensitively; the display
            // label must not be forwarded verbatim or `build` fails with
            // "Unknown codecs: RemoteFX".
            DisplayPerformance::High => vec!["remotefx:on".to_string()],
            _ => Vec::new(),
        }
    }

    /// `.rdp` settings for the external fallback. Unknown keys from the server
    /// editor are preserved by `RdpFile`.
    pub fn rdp_file_settings(&self) -> Vec<(String, String, String)> {
        let mut settings = vec![
            ("full address".into(), "s".into(), self.destination()),
            ("username".into(), "s".into(), self.username.clone()),
            ("domain".into(), "s".into(), self.domain.clone()),
            (
                "screen mode id".into(),
                "i".into(),
                if self.display.full_screen { "2" } else { "1" }.to_string(),
            ),
            (
                "desktopwidth".into(),
                "i".into(),
                self.display.width.to_string(),
            ),
            (
                "desktopheight".into(),
                "i".into(),
                self.display.height.to_string(),
            ),
            (
                "redirectclipboard".into(),
                "i".into(),
                bool_int(self.redirect.clipboard),
            ),
            (
                "redirectdrives".into(),
                "i".into(),
                bool_int(self.redirect.disk_drives),
            ),
            (
                "redirectprinters".into(),
                "i".into(),
                bool_int(self.redirect.printers),
            ),
            (
                "redirectcomports".into(),
                "i".into(),
                bool_int(self.redirect.ports),
            ),
            (
                "redirectsmartcards".into(),
                "i".into(),
                bool_int(self.redirect.smart_cards),
            ),
            (
                "usemultimon".into(),
                "i".into(),
                bool_int(self.display.multimon),
            ),
            (
                "audiomode".into(),
                "i".into(),
                match self.redirect.audio {
                    AudioRedirectionMode::RedirectToLocal => "0",
                    AudioRedirectionMode::LeaveOnRemote => "1",
                    AudioRedirectionMode::Disabled => "2",
                }
                .to_string(),
            ),
        ];
        if self.admin_session {
            settings.push(("administrative session".into(), "i".into(), "1".into()));
        }
        if let Some(gateway) = &self.gateway {
            settings.push(("gatewayusagemethod".into(), "i".into(), "1".to_string()));
            settings.push((
                "gatewayhostname".into(),
                "s".into(),
                gateway.endpoint.clone(),
            ));
            settings.push((
                "gatewayusername".into(),
                "s".into(),
                gateway.username.clone(),
            ));
        }
        settings
    }
}

fn clamp_dimension(value: u32, min: u32, max: u32, fallback: u32) -> u32 {
    let value = if value == 0 { fallback } else { value };
    value.clamp(min, max)
}

pub fn resolve_scale(follow_system: bool, device_scale: u16, custom: u16) -> u16 {
    let value = if follow_system {
        if device_scale == 0 { 100 } else { device_scale }
    } else {
        custom
    };
    value.clamp(MIN_SCALE, MAX_SCALE)
}

fn bool_int(value: bool) -> String {
    if value { "1" } else { "0" }.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use hr_domain::Server;

    fn rdp_server() -> Server {
        Server::new_rdp("Desktop", "10.0.0.5", 3390)
    }

    #[test]
    fn fixed_mode_keeps_stored_dimensions() {
        let mut server = rdp_server();
        let Server::Rdp(rdp) = &mut server else {
            unreachable!()
        };
        rdp.rdp_window_resize_mode = RdpWindowResizeMode::Fixed;
        rdp.rdp_width = 1280;
        rdp.rdp_height = 720;
        let plan = RdpPlan::from_server(&server, (1920, 1080), 150, Default::default()).unwrap();
        assert_eq!(plan.display.width, 1280);
        assert_eq!(plan.display.height, 720);
        assert_eq!(plan.display.scale_percent, 150);
        assert_eq!(plan.destination(), "10.0.0.5:3390");
    }

    #[test]
    fn auto_resize_uses_the_viewport_and_clamps() {
        let server = rdp_server();
        let plan = RdpPlan::from_server(&server, (200, 100), 100, Default::default()).unwrap();
        assert_eq!(plan.display.width, MIN_WIDTH);
        assert_eq!(plan.display.height, MIN_HEIGHT);
        let plan =
            RdpPlan::from_server(&server, (20_000, 20_000), 100, Default::default()).unwrap();
        assert_eq!(plan.display.width, MAX_WIDTH);
        assert_eq!(plan.display.height, MAX_HEIGHT);
    }

    #[test]
    fn scale_prefers_custom_value_and_clamps() {
        assert_eq!(resolve_scale(true, 125, 50), 125);
        assert_eq!(resolve_scale(false, 125, 50), 100);
        assert_eq!(resolve_scale(false, 125, 275), 275);
        assert_eq!(resolve_scale(false, 125, 900), MAX_SCALE);
        assert_eq!(resolve_scale(true, 0, 0), 100);
    }

    #[test]
    fn performance_maps_to_depth_and_codecs() {
        let mut server = rdp_server();
        {
            let Server::Rdp(rdp) = &mut server else {
                unreachable!()
            };
            rdp.display_performance = DisplayPerformance::Low;
        }
        let plan = RdpPlan::from_server(&server, (1280, 720), 100, Default::default()).unwrap();
        assert_eq!(plan.color_depth(), 16);
        assert!(plan.codecs().is_empty());

        {
            let Server::Rdp(rdp) = &mut server else {
                unreachable!()
            };
            rdp.display_performance = DisplayPerformance::High;
        }
        let plan = RdpPlan::from_server(&server, (1280, 720), 100, Default::default()).unwrap();
        assert_eq!(plan.color_depth(), 32);
        assert_eq!(plan.codecs(), vec!["remotefx:on".to_string()]);
    }

    #[test]
    fn unsupported_redirections_are_reported_not_silently_dropped() {
        let mut server = rdp_server();
        let Server::Rdp(rdp) = &mut server else {
            unreachable!()
        };
        rdp.enable_printers = true;
        rdp.enable_ports = true;
        rdp.enable_redirect_cameras = true;
        rdp.enable_audio_capture = true;
        rdp.is_administrative_purposes = true;
        let plan = RdpPlan::from_server(&server, (1280, 720), 100, Default::default()).unwrap();
        assert_eq!(plan.warnings.len(), 5);
        assert!(
            plan.warnings
                .iter()
                .any(|warning| warning.contains("Printer"))
        );
        assert!(
            plan.warnings
                .iter()
                .any(|warning| warning.contains("admin"))
        );
        let settings = plan.rdp_file_settings();
        assert!(
            settings
                .iter()
                .any(|(key, _, value)| key == "administrative session" && value == "1")
        );
    }

    #[test]
    fn gateway_requires_a_host_and_normalises_the_port() {
        let mut server = rdp_server();
        {
            let Server::Rdp(rdp) = &mut server else {
                unreachable!()
            };
            rdp.gateway_mode = GatewayMode::UseTheseGatewayServerSettings;
        }
        assert!(RdpPlan::from_server(&server, (1280, 720), 100, Default::default()).is_err());
        {
            let Server::Rdp(rdp) = &mut server else {
                unreachable!()
            };
            rdp.gateway_host_name = "gw.example".into();
            rdp.gateway_user_name = "gwuser".into();
            rdp.gateway_password = "gwpass".into();
        }
        let plan = RdpPlan::from_server(&server, (1280, 720), 100, Default::default()).unwrap();
        let gateway = plan.gateway.as_ref().unwrap();
        assert_eq!(gateway.endpoint, "gw.example:443");
        assert!(
            plan.rdp_file_settings()
                .iter()
                .any(|(key, _, value)| key == "gatewayhostname" && value == "gw.example:443")
        );
    }

    #[test]
    fn mstsc_mode_selects_the_external_engine() {
        let mut server = rdp_server();
        let Server::Rdp(rdp) = &mut server else {
            unreachable!()
        };
        rdp.mstsc_mode_enabled = true;
        let plan = RdpPlan::from_server(&server, (1280, 720), 100, Default::default()).unwrap();
        assert_eq!(plan.engine, EngineChoice::ExternalMstsc);
    }

    #[test]
    fn non_rdp_servers_are_rejected() {
        let server = Server::new_ssh("Box", "host", "root");
        assert!(RdpPlan::from_server(&server, (1280, 720), 100, Default::default()).is_err());
    }

    #[test]
    fn reconnect_backoff_grows_and_is_capped() {
        let policy = ReconnectPolicy {
            max_attempts: 5,
            initial_delay_ms: 500,
            max_delay_ms: 3_000,
        };
        assert_eq!(policy.delay(1).as_millis(), 500);
        assert_eq!(policy.delay(2).as_millis(), 1_000);
        assert_eq!(policy.delay(3).as_millis(), 2_000);
        assert_eq!(policy.delay(4).as_millis(), 3_000);
        assert_eq!(policy.delay(9).as_millis(), 3_000);
    }
}
