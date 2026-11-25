//! IronRDP embedded backend.
//!
//! The backend is a thin adapter: it maps a [`RdpPlan`] onto IronRDP's client
//! configuration, forwards resize/Ctrl+Alt+Del commands, converts RGBA frames
//! into the bounded frame transport and maps termination reasons onto the
//! session state machine.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use hr_core::{AppError, AppResult};
use hr_domain::AudioRedirectionMode;
use ironrdp_client::config::{ClipboardType, Config, ConfigBuilder, Destination, TransportKind};
use ironrdp_client::rdp::{RdpClient, RdpInputEvent, RdpOutputEvent};
use ironrdp_graphics::pointer::DecodedPointer;
use ironrdp_pdu::input::fast_path::{FastPathInputEvent, KeyboardFlags};
use ironrdp_pdu::rdp::capability_sets::MajorPlatformType;
use ironrdp_propertyset::PropertySet;

use crate::manager::{Backend, BackendContext, BackendEvent, BackendOutcome, SessionCommand};
use crate::plan::RdpPlan;
use crate::session_plan::SessionPlan;
use crate::state::{DisconnectReason, SessionBackend, SessionCursor};

pub const CLIENT_NAME: &str = "H-Remote";
pub const CLIENT_DIR: &str = "C:\\Windows\\System32";
/// Advertised Windows build. A realistic value lets the server negotiate modern
/// capability sets instead of treating the client as a very old mstsc build.
pub const CLIENT_BUILD: u32 = 22621;

#[derive(Debug, Default)]
pub struct IronRdpBackend;

impl Backend for IronRdpBackend {
    fn kind(&self) -> SessionBackend {
        SessionBackend::IronRdp
    }

    fn run<'a>(
        &'a self,
        context: BackendContext<'a>,
    ) -> Pin<Box<dyn Future<Output = BackendOutcome> + Send + 'a>> {
        Box::pin(async move { run(context).await })
    }
}

async fn run(context: BackendContext<'_>) -> BackendOutcome {
    let SessionPlan::Rdp(plan) = context.plan.clone() else {
        return BackendOutcome::Failed {
            reason: DisconnectReason::ConnectionFailed,
            message: "the RDP backend received a non-RDP plan".to_string(),
        };
    };
    let frames = Arc::clone(&context.frames);
    let events = context.events.clone();
    let cancel = context.cancel.clone();
    let (bridge_tx, bridge_rx) = tokio::sync::mpsc::unbounded_channel();
    let (done_tx, mut done_rx) = tokio::sync::oneshot::channel();
    // IronRDP's clipboard/rdpdr futures are deliberately `!Send` (native
    // backends hold OS handles), so the client runs on a dedicated thread with
    // its own current-thread runtime. The bridge carries input and shutdown.
    let thread = std::thread::Builder::new()
        .name("hremote-rdp".to_string())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = done_tx.send(BackendOutcome::Failed {
                        reason: DisconnectReason::ConnectionFailed,
                        message: format!("cannot start the RDP runtime: {error}"),
                    });
                    return;
                }
            };
            let outcome = runtime.block_on(run_client(plan, frames, events, bridge_rx, cancel));
            let _ = done_tx.send(outcome);
        });
    if let Err(error) = thread {
        return BackendOutcome::Failed {
            reason: DisconnectReason::ConnectionFailed,
            message: format!("cannot start the RDP thread: {error}"),
        };
    }

    loop {
        tokio::select! {
            _ = context.cancel.cancelled() => {
                let _ = bridge_tx.send(BridgeCommand::Shutdown);
                return BackendOutcome::Cancelled;
            }
            command = context.commands.recv() => {
                if let Some(command) = command {
                    let _ = bridge_tx.send(BridgeCommand::Input(command));
                }
            }
            outcome = &mut done_rx => {
                return outcome.unwrap_or_else(|_| {
                    tracing::error!("RDP worker ended without reporting a result");
                    BackendOutcome::Failed {
                        reason: DisconnectReason::ConnectionFailed,
                        message: "RDP worker ended without reporting a result".to_string(),
                    }
                });
            }
        }
    }
}

enum BridgeCommand {
    Input(SessionCommand),
    Shutdown,
}

async fn run_client(
    plan: RdpPlan,
    frames: Arc<crate::frames::FrameTransport>,
    events: tokio::sync::mpsc::UnboundedSender<BackendEvent>,
    mut bridge: tokio::sync::mpsc::UnboundedReceiver<BridgeCommand>,
    cancel: crate::manager::CancelToken,
) -> BackendOutcome {
    let config = match build_config(&plan) {
        Ok(config) => config,
        Err(error) => {
            return BackendOutcome::Failed {
                reason: DisconnectReason::ConnectionFailed,
                message: error.to_string(),
            };
        }
    };
    let (output_tx, mut output_rx) = tokio::sync::mpsc::channel(16);
    let client = RdpClient::new(config, output_tx);
    let input = client.input_sender();
    let mut task = Box::pin(client.run());
    let mut announced = false;
    // Tracks keyboard/mouse state so press/release and button masks stay
    // consistent across events (P3 input fix).
    let mut input_db = ironrdp_input::Database::new();

    loop {
        tokio::select! {
            // The client sends its terminal event before its task completes.
            // Prefer the event so a connection failure is never downgraded to
            // "Disconnected / NoInfo" by a simultaneously-ready task branch.
            biased;
            _ = cancel.cancelled() => {
                let _ = input.send(RdpInputEvent::Close);
                return BackendOutcome::Cancelled;
            }
            command = bridge.recv() => {
                match command {
                    Some(BridgeCommand::Input(SessionCommand::Resize { width, height, scale_percent })) => {
                        // MS-RDPEDISP caps the primary monitor at 8192x8192 and
                        // requires even dimensions; IronRDP adjusts the rest.
                        let width = width.clamp(320, 8192) as u16;
                        let height = height.clamp(240, 8192) as u16;
                        let _ = input.send(RdpInputEvent::Resize {
                            width,
                            height,
                            scale_factor: u32::from(scale_percent),
                            physical_size: None,
                        });
                    }
                    Some(BridgeCommand::Input(SessionCommand::RdpKey { code, key, down })) => {
                        if let Some(operation) = crate::rdp_input::key_operation(&code, &key, down) {
                            let events = input_db.apply([operation]);
                            if !events.is_empty() {
                                let _ = input.send(RdpInputEvent::FastPath(events));
                            }
                        }
                    }
                    Some(BridgeCommand::Input(SessionCommand::RdpPointerMove { x, y })) => {
                        let events = input_db.apply([crate::rdp_input::pointer_move(x, y)]);
                        if !events.is_empty() {
                            let _ = input.send(RdpInputEvent::FastPath(events));
                        }
                    }
                    Some(BridgeCommand::Input(SessionCommand::RdpPointerButton { x, y, button, down })) => {
                        let mut operations = vec![crate::rdp_input::pointer_move(x, y)];
                        if let Some(operation) = crate::rdp_input::button_operation(button, down) {
                            operations.push(operation);
                        }
                        let events = input_db.apply(operations);
                        if !events.is_empty() {
                            let _ = input.send(RdpInputEvent::FastPath(events));
                        }
                    }
                    Some(BridgeCommand::Input(SessionCommand::RdpWheel { x, y, vertical, units })) => {
                        let events = input_db.apply([
                            crate::rdp_input::pointer_move(x, y),
                            crate::rdp_input::wheel_operation(vertical, units),
                        ]);
                        if !events.is_empty() {
                            let _ = input.send(RdpInputEvent::FastPath(events));
                        }
                    }
                    Some(BridgeCommand::Input(SessionCommand::CtrlAltDel))
                    | Some(BridgeCommand::Input(SessionCommand::Special(
                        crate::manager::SpecialKey::CtrlAltDel,
                    ))) => {
                        for event in ctrl_alt_del_sequence() {
                            let _ = input.send(RdpInputEvent::FastPath(vec![event].into()));
                        }
                    }
                    Some(BridgeCommand::Input(_)) => {}
                    Some(BridgeCommand::Shutdown) | None => {
                        let _ = input.send(RdpInputEvent::Close);
                        return BackendOutcome::Cancelled;
                    }
                }
            }
            event = output_rx.recv() => {
                match event {
                    Some(RdpOutputEvent::Image { buffer, width, height }) => {
                        let (width, height) = (width.get() as u32, height.get() as u32);
                        match rgba_from_ironrdp(&buffer, width, height) {
                            Ok(rgba) => {
                                let seq = match frames.publish(width, height, &rgba) {
                                    Ok(seq) => seq,
                                    Err(error) => {
                                        return BackendOutcome::Failed {
                                            reason: DisconnectReason::ConnectionFailed,
                                            message: error.to_string(),
                                        };
                                    }
                                };
                                if !announced {
                                    announced = true;
                                    let _ = events.send(BackendEvent::Connected);
                                }
                                let _ = events.send(BackendEvent::Frame { seq, width, height });
                            }
                            Err(error) => {
                                tracing::warn!(%error, "ignoring malformed IronRDP frame");
                            }
                        }
                    }
                    Some(RdpOutputEvent::ConnectionFailure(error)) => {
                        let reason = map_connection_failure(&error);
                        tracing::warn!(
                            reason = ?reason,
                            message = %error,
                            "RDP connection failed"
                        );
                        return BackendOutcome::Failed {
                            reason,
                            message: error.to_string(),
                        };
                    }
                    Some(RdpOutputEvent::Terminated(result)) => {
                        return match result {
                            Ok(reason) => BackendOutcome::Disconnected {
                                reason: map_disconnect_reason(&reason),
                                message: Some(reason.description()),
                            },
                            Err(error) => {
                                tracing::warn!(%error, "RDP session terminated with an error");
                                BackendOutcome::Failed {
                                    reason: DisconnectReason::NetworkLoss,
                                    message: error.to_string(),
                                }
                            }
                        };
                    }
                    Some(RdpOutputEvent::PointerDefault) => {
                        let _ = events.send(BackendEvent::Cursor(SessionCursor::Default));
                    }
                    Some(RdpOutputEvent::PointerHidden) => {
                        let _ = events.send(BackendEvent::Cursor(SessionCursor::Hidden));
                    }
                    Some(RdpOutputEvent::PointerBitmap(pointer)) => {
                        if let Some(cursor) = cursor_from_pointer(&pointer) {
                            let _ = events.send(BackendEvent::Cursor(cursor));
                        }
                    }
                    Some(RdpOutputEvent::PointerPosition { .. }) => {
                        // The webview's OS cursor already tracks the pointer; the
                        // server-reported position is only needed by clients that
                        // move a system cursor themselves.
                    }
                    None => {
                        tracing::warn!("RDP output channel closed without a terminal event");
                        return BackendOutcome::Disconnected {
                            reason: DisconnectReason::NoInfo,
                            message: Some(
                                "RDP output channel closed without a terminal event".to_string(),
                            ),
                        };
                    }
                }
            }
            _ = &mut task => {
                // Drain any event that raced with task completion.
                while let Ok(event) = output_rx.try_recv() {
                    if let RdpOutputEvent::ConnectionFailure(error) = event {
                        let reason = map_connection_failure(&error);
                        tracing::warn!(
                            reason = ?reason,
                            message = %error,
                            "RDP connection failed"
                        );
                        return BackendOutcome::Failed {
                            reason,
                            message: error.to_string(),
                        };
                    }
                }
                tracing::warn!("RDP client task ended without a terminal event");
                return BackendOutcome::Disconnected {
                    reason: DisconnectReason::NoInfo,
                    message: Some("RDP client task ended without a terminal event".to_string()),
                };
            }
        }
    }
}

/// Maps the plan to an IronRDP client configuration without leaking secrets to
/// logs (`Config`'s `Debug` redacts the properties it stores).
pub fn build_config(plan: &RdpPlan) -> AppResult<Config> {
    let properties = build_property_set(plan)?;
    let mut builder = ConfigBuilder::new()
        .with_destination(Destination::from_parts(plan.address.clone(), plan.port))
        .with_username(plan.username.clone())
        .with_password(plan.password.expose())
        .with_client_build(CLIENT_BUILD)
        .with_client_dir(CLIENT_DIR)
        .with_client_name(CLIENT_NAME)
        .with_platform(MajorPlatformType::WINDOWS)
        .with_desktop_width(
            plan.display
                .width
                .clamp(crate::plan::MIN_WIDTH, crate::plan::MAX_WIDTH) as u16,
        )
        .with_desktop_height(
            plan.display
                .height
                .clamp(crate::plan::MIN_HEIGHT, crate::plan::MAX_HEIGHT) as u16,
        )
        .with_desktop_scale_factor(u32::from(plan.display.scale_percent))
        .with_color_depth(plan.color_depth())
        .with_codecs(plan.codecs())
        .with_credssp(true)
        .with_tls(true)
        .with_clipboard(if plan.redirect.clipboard {
            ClipboardType::Enable
        } else {
            ClipboardType::Disable
        })
        .with_rdpdr(plan.redirect.disk_drives)
        .with_smartcard(plan.redirect.smart_cards);
    if !plan.domain.trim().is_empty() {
        builder = builder.with_domain(plan.domain.clone());
    }
    if properties.iter().next().is_some() {
        builder = builder
            .with_property_set(&properties)
            .map_err(|error| AppError::validation(format!("invalid RDP properties: {error}")))?;
    }
    // These toggles are plan-authoritative, so they are applied after imported
    // `.rdp` properties. Bulk compression is deliberately off: ironrdp-client
    // 0.1 rebuilds its FastPath processor without a decompressor after a
    // Deactivation-Reactivation (display resize), which drops every compressed
    // update and produces the "no decompressor is configured" warning. RDP
    // bitmap/RemoteFX compression is unaffected.
    //
    // Server-pointer updates are enabled so the UI can mirror the remote cursor
    // shape (I-beam, resize arrows, etc.), but software rendering stays off:
    // IronRDP then emits pointer bitmaps instead of compositing a lagging sprite
    // into the frame. The local OS cursor remains the single visible pointer.
    builder = builder
        .with_compression(false)
        .with_server_pointer(true)
        .with_sound(matches!(
            plan.redirect.audio,
            AudioRedirectionMode::RedirectToLocal
        ));
    if let Some(gateway) = &plan.gateway {
        builder = builder
            .with_transport(TransportKind::Gateway {
                endpoint: gateway.endpoint.clone(),
            })
            .with_gateway_username(gateway.username.clone())
            .with_gateway_password(gateway.password.expose());
    }
    builder
        .build()
        .map_err(|error| AppError::validation(format!("invalid RDP configuration: {error}")))
}

/// Additional `.rdp` lines and ActiveX-style properties that IronRDP accepts as
/// a generic property set. Known secret/identity keys are rejected so the plan
/// stays authoritative.
pub fn build_property_set(plan: &RdpPlan) -> AppResult<PropertySet> {
    let mut properties = PropertySet::new();
    let parsed = hr_domain::RdpFile::parse(&plan.additional_settings).map_err(|error| {
        AppError::validation(format!("invalid additional .rdp settings: {error}"))
    })?;
    for (key, (value_type, value)) in parsed.settings {
        insert_property(&mut properties, &key, &value_type, &value)?;
    }
    for setting in &plan.control_properties {
        let value_type = match setting.value_type {
            hr_domain::ControlSettingType::String => "s",
            hr_domain::ControlSettingType::Integer => "i",
            hr_domain::ControlSettingType::Boolean => "i",
        };
        let value = match setting.value_type {
            hr_domain::ControlSettingType::Boolean => {
                if setting.value.eq_ignore_ascii_case("true") {
                    "1".to_string()
                } else if setting.value.eq_ignore_ascii_case("false") {
                    "0".to_string()
                } else {
                    setting.value.clone()
                }
            }
            _ => setting.value.clone(),
        };
        insert_property(&mut properties, &setting.name, value_type, &value)?;
    }
    Ok(properties)
}

fn insert_property(
    properties: &mut PropertySet,
    key: &str,
    value_type: &str,
    value: &str,
) -> AppResult<()> {
    let key = key.trim();
    if key.is_empty() {
        return Err(AppError::validation(
            "an additional RDP property has no name",
        ));
    }
    if is_protected_property(key) {
        return Err(AppError::validation(format!(
            "the RDP property '{key}' is managed by H-Remote"
        )));
    }
    match value_type {
        "i" | "b" | "d" => {
            let parsed = value.parse::<i64>().map_err(|_| {
                AppError::validation(format!("the RDP property '{key}' needs an integer value"))
            })?;
            properties.insert(key.to_string(), parsed);
        }
        _ => {
            properties.insert(key.to_string(), value.to_string());
        }
    }
    Ok(())
}

fn is_protected_property(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "password 51" | "password" | "full address" | "username" | "domain"
    )
}

fn cursor_from_pointer(pointer: &DecodedPointer) -> Option<SessionCursor> {
    if pointer.width == 0 || pointer.height == 0 {
        return Some(SessionCursor::Hidden);
    }
    let image = image::RgbaImage::from_raw(
        u32::from(pointer.width),
        u32::from(pointer.height),
        pointer.bitmap_data.clone(),
    )?;
    let mut png = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new_with_quality(
        std::io::Cursor::new(&mut png),
        image::codecs::png::CompressionType::Fast,
        image::codecs::png::FilterType::Sub,
    );
    if let Err(error) = image.write_with_encoder(encoder) {
        tracing::warn!(%error, "cannot encode pointer bitmap");
        return None;
    }
    Some(SessionCursor::Image {
        data_url: format!("data:image/png;base64,{}", STANDARD.encode(&png)),
        hotspot_x: pointer.hotspot_x.min(pointer.width.saturating_sub(1)),
        hotspot_y: pointer.hotspot_y.min(pointer.height.saturating_sub(1)),
    })
}

fn rgba_from_ironrdp(buffer: &[u32], width: u32, height: u32) -> AppResult<Vec<u8>> {
    let expected = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| AppError::validation("frame dimensions overflow"))?;
    if buffer.len() != expected {
        return Err(AppError::validation(format!(
            "IronRDP delivered {} pixels, expected {expected}",
            buffer.len()
        )));
    }
    let mut rgba = Vec::with_capacity(expected * 4);
    for pixel in buffer {
        rgba.push((pixel >> 16) as u8);
        rgba.push((pixel >> 8) as u8);
        rgba.push(*pixel as u8);
        rgba.push(255);
    }
    Ok(rgba)
}

fn map_disconnect_reason(reason: &ironrdp_session::GracefulDisconnectReason) -> DisconnectReason {
    match reason {
        ironrdp_session::GracefulDisconnectReason::UserInitiated => DisconnectReason::RemoteUser,
        ironrdp_session::GracefulDisconnectReason::ServerInitiated => {
            DisconnectReason::RemoteServer
        }
        ironrdp_session::GracefulDisconnectReason::Other(_) => DisconnectReason::NoInfo,
    }
}

fn map_connection_failure(error: &ironrdp_connector::ConnectorError) -> DisconnectReason {
    let text = format!("{error:?}").to_ascii_lowercase();
    if text.contains("credssp") || text.contains("auth") || text.contains("logon") {
        DisconnectReason::AuthenticationFailed
    } else if text.contains("timeout") || text.contains("timed out") {
        DisconnectReason::Timeout
    } else {
        DisconnectReason::ConnectionFailed
    }
}

/// Ctrl (0x1D), Alt (0x38), Del (extended 0x53) press/release sequence.
fn ctrl_alt_del_sequence() -> Vec<FastPathInputEvent> {
    vec![
        FastPathInputEvent::KeyboardEvent(KeyboardFlags::empty(), 0x1D),
        FastPathInputEvent::KeyboardEvent(KeyboardFlags::empty(), 0x38),
        FastPathInputEvent::KeyboardEvent(KeyboardFlags::EXTENDED, 0x53),
        FastPathInputEvent::KeyboardEvent(KeyboardFlags::RELEASE | KeyboardFlags::EXTENDED, 0x53),
        FastPathInputEvent::KeyboardEvent(KeyboardFlags::RELEASE, 0x38),
        FastPathInputEvent::KeyboardEvent(KeyboardFlags::RELEASE, 0x1D),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use hr_domain::{
        AudioRedirectionMode, ControlSettingType, DisplayPerformance, ProtocolControlSetting,
        Server,
    };

    fn plan() -> RdpPlan {
        let mut server = Server::new_rdp("Desktop", "10.0.0.5", 3390);
        let Server::Rdp(rdp) = &mut server else {
            unreachable!()
        };
        rdp.credentials.username = "admin".into();
        rdp.credentials.password = "secret".into();
        rdp.domain = "CORP".into();
        rdp.rdp_window_resize_mode = hr_domain::RdpWindowResizeMode::Fixed;
        RdpPlan::from_server(&server, (1280, 720), 100, Default::default()).unwrap()
    }

    #[test]
    fn converts_ironrdp_pixels_to_rgba() {
        let buffer = vec![0x00_11_22_33, 0x00_AA_BB_CC];
        let rgba = rgba_from_ironrdp(&buffer, 2, 1).unwrap();
        assert_eq!(rgba, vec![0x11, 0x22, 0x33, 255, 0xAA, 0xBB, 0xCC, 255]);
        assert!(rgba_from_ironrdp(&buffer, 3, 1).is_err());
    }

    #[test]
    fn ctrl_alt_del_has_press_and_release_pairs() {
        let sequence = ctrl_alt_del_sequence();
        assert_eq!(sequence.len(), 6);
        assert!(matches!(
            sequence[2],
            FastPathInputEvent::KeyboardEvent(flags, 0x53) if flags.contains(KeyboardFlags::EXTENDED)
        ));
    }

    #[test]
    fn additional_settings_are_parsed_and_protected_keys_rejected() {
        let mut plan = plan();
        plan.additional_settings = "custom:s:value\r\naudioqualitymode:i:2".into();
        let properties = build_property_set(&plan).unwrap();
        assert!(
            properties
                .iter()
                .any(|(key, value)| key.as_ref() == "custom" && value.as_str() == Some("value"))
        );
        assert!(properties
            .iter()
            .any(|(key, value)| key.as_ref() == "audioqualitymode" && value.as_int() == Some(2)));
        plan.additional_settings = "password 51:b:DEAD".into();
        assert!(build_property_set(&plan).is_err());
    }

    #[test]
    fn control_properties_map_typed_values() {
        let mut plan = plan();
        plan.control_properties = vec![
            ProtocolControlSetting {
                name: "disablewallpaper".into(),
                value: "true".into(),
                value_type: ControlSettingType::Boolean,
            },
            ProtocolControlSetting {
                name: "compression".into(),
                value: "1".into(),
                value_type: ControlSettingType::Integer,
            },
        ];
        let properties = build_property_set(&plan).unwrap();
        assert!(properties.iter().count() >= 2);
    }

    #[test]
    fn configuration_builds_for_direct_and_gateway_plans() {
        let mut plan = plan();
        let config = build_config(&plan).unwrap();
        assert_eq!(config.destination().name(), "10.0.0.5");
        assert_eq!(config.destination().port(), 3390);
        // Bulk compression stays off until ironrdp-client preserves its
        // FastPath decompressor across Deactivation-Reactivation.
        assert!(config.connector().compression_type.is_none());
        assert!(config.connector().enable_audio_playback);
        assert!(config.channels().sound);
        // Pointer shape events are consumed by the UI as a CSS cursor; software
        // rendering stays off so no sprite is baked into the frame.
        assert!(config.connector().enable_server_pointer);
        assert!(!config.connector().pointer_software_rendering);
        assert_eq!(config.connector().client_build, CLIENT_BUILD);

        plan.gateway = Some(crate::plan::GatewayPlan {
            endpoint: "gw.example:443".into(),
            username: "gwuser".into(),
            password: "gwpass".into(),
            logon_method: hr_domain::GatewayLogonMethod::Password,
            bypass_for_local: true,
        });
        assert!(build_config(&plan).is_ok());
    }

    #[test]
    fn remote_audio_is_only_advertised_when_redirected_locally() {
        let mut plan = plan();
        assert!(plan.redirect.audio == AudioRedirectionMode::RedirectToLocal);
        assert!(build_config(&plan).unwrap().channels().sound);

        plan.redirect.audio = AudioRedirectionMode::Disabled;
        let config = build_config(&plan).unwrap();
        assert!(!config.channels().sound);

        plan.redirect.audio = AudioRedirectionMode::LeaveOnRemote;
        let config = build_config(&plan).unwrap();
        assert!(!config.channels().sound);
    }

    #[test]
    fn external_audio_properties_cannot_override_the_plan() {
        let mut plan = plan();
        plan.additional_settings = "audiomode:i:2".into();
        assert!(plan.redirect.audio == AudioRedirectionMode::RedirectToLocal);
        assert!(build_config(&plan).unwrap().channels().sound);
    }

    #[test]
    fn high_performance_and_wide_desktops_build() {
        let mut plan = plan();
        plan.performance = DisplayPerformance::High;
        plan.display.width = 5760;
        plan.display.height = 2160;
        let config = build_config(&plan).unwrap();
        assert_eq!(config.connector().desktop_size.width, 5760);
        assert_eq!(config.connector().desktop_size.height, 2160);
    }

    #[test]
    fn pointer_bitmaps_become_css_cursor_images() {
        let pointer = DecodedPointer {
            width: 2,
            height: 1,
            hotspot_x: 1,
            hotspot_y: 0,
            bitmap_data: vec![255, 0, 0, 255, 0, 0, 0, 0],
        };
        match cursor_from_pointer(&pointer).unwrap() {
            SessionCursor::Image {
                data_url,
                hotspot_x,
                hotspot_y,
            } => {
                assert!(data_url.starts_with("data:image/png;base64,"));
                assert_eq!((hotspot_x, hotspot_y), (1, 0));
            }
            other => panic!("unexpected cursor: {other:?}"),
        }
        let invisible = DecodedPointer {
            width: 0,
            height: 0,
            hotspot_x: 0,
            hotspot_y: 0,
            bitmap_data: Vec::new(),
        };
        assert_eq!(cursor_from_pointer(&invisible), Some(SessionCursor::Hidden));
    }

    #[test]
    fn termination_reasons_map_to_disconnect_reasons() {
        assert_eq!(
            map_disconnect_reason(&ironrdp_session::GracefulDisconnectReason::UserInitiated),
            DisconnectReason::RemoteUser
        );
        assert_eq!(
            map_disconnect_reason(&ironrdp_session::GracefulDisconnectReason::ServerInitiated),
            DisconnectReason::RemoteServer
        );
    }
}
