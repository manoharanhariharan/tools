//! Embedded VNC (RFB) backend using `vnc-rs`.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use hr_core::{AppError, AppResult};
use tokio::net::TcpStream;
use vnc::client::connector::VncState;
use vnc::{
    ClientKeyEvent, ClientMouseEvent, PixelFormat, Rect, VncConnector, VncEncoding, VncEvent,
    X11Event,
};

use crate::manager::{
    Backend, BackendContext, BackendEvent, BackendOutcome, CancelToken, SessionCommand,
};
use crate::session_plan::{SessionPlan, VncPlan};
use crate::state::{DisconnectReason, SessionBackend};

#[derive(Debug, Default)]
pub struct VncBackend;

impl Backend for VncBackend {
    fn kind(&self) -> SessionBackend {
        SessionBackend::Vnc
    }

    fn run<'a>(
        &'a self,
        context: BackendContext<'a>,
    ) -> Pin<Box<dyn Future<Output = BackendOutcome> + Send + 'a>> {
        Box::pin(async move { run(context).await })
    }
}

async fn run(context: BackendContext<'_>) -> BackendOutcome {
    let SessionPlan::Vnc(plan) = context.plan else {
        return BackendOutcome::Failed {
            reason: DisconnectReason::ConnectionFailed,
            message: "the VNC backend received a non-VNC plan".to_string(),
        };
    };
    let plan = plan.clone();
    let frames = Arc::clone(&context.frames);
    let events = context.events.clone();
    let cancel = context.cancel.clone();
    let commands = context.commands;

    let client = match connect(&plan, &cancel).await {
        Ok(client) => client,
        Err(error) => {
            return BackendOutcome::Failed {
                reason: if cancel.is_cancelled() {
                    DisconnectReason::LocalCancel
                } else {
                    DisconnectReason::ConnectionFailed
                },
                message: error.to_string(),
            };
        }
    };
    let _ = events.send(BackendEvent::Connected);
    let mut framebuffer = Framebuffer::default();

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                let _ = client.close().await;
                return BackendOutcome::Cancelled;
            }
            command = commands.recv() => {
                match command {
                    Some(SessionCommand::Resize { width, height, .. }) => {
                        let _ = client
                            .resize_desktop(width as u16, height as u16)
                            .await;
                    }
                    Some(SessionCommand::Special(key)) => {
                        for event in special_events(key) {
                            let _ = client.input(event).await;
                        }
                    }
                    Some(SessionCommand::VncKey { keycode, down }) => {
                        let _ = client
                            .input(X11Event::KeyEvent(ClientKeyEvent { keycode, down }))
                            .await;
                    }
                    Some(SessionCommand::VncPointer { x, y, buttons }) => {
                        let _ = client
                            .input(X11Event::PointerEvent(ClientMouseEvent {
                                position_x: x,
                                position_y: y,
                                bottons: buttons,
                            }))
                            .await;
                    }
                    Some(SessionCommand::VncRefresh) => {
                        let _ = client.input(X11Event::Refresh).await;
                    }
                    Some(SessionCommand::VncText(text)) => {
                        let _ = client.input(X11Event::CopyText(text)).await;
                    }
                    Some(SessionCommand::CtrlAltDel) => {
                        for event in special_events(crate::manager::SpecialKey::CtrlAltDel) {
                            let _ = client.input(event).await;
                        }
                    }
                    Some(_) => {}
                    None => {}
                }
            }
            event = client.recv_event() => {
                match event {
                    Ok(event) => {
                        match apply_event(event, &mut framebuffer) {
                            Ok(Some(seq)) => {
                                if let Err(error) = frames.publish(
                                    framebuffer.width,
                                    framebuffer.height,
                                    &framebuffer.pixels,
                                ) {
                                    return BackendOutcome::Failed {
                                        reason: DisconnectReason::ConnectionFailed,
                                        message: error.to_string(),
                                    };
                                }
                                let _ = events.send(BackendEvent::Frame {
                                    seq,
                                    width: framebuffer.width,
                                    height: framebuffer.height,
                                });
                            }
                            Ok(None) => {}
                            Err(error) => {
                                return BackendOutcome::Failed {
                                    reason: DisconnectReason::ConnectionFailed,
                                    message: error.to_string(),
                                };
                            }
                        }
                    }
                    Err(error) => {
                        let message = error.to_string();
                        return BackendOutcome::Failed {
                            reason: DisconnectReason::NetworkLoss,
                            message,
                        };
                    }
                }
            }
        }
    }
}

/// Drives the RFB handshake with the configured password and pixel format.
async fn connect(plan: &VncPlan, cancel: &CancelToken) -> AppResult<vnc::VncClient> {
    let stream = tokio::select! {
        _ = cancel.cancelled() => {
            return Err(AppError::unavailable("VNC connection cancelled"));
        }
        result = tokio::time::timeout(
            std::time::Duration::from_secs(20),
            TcpStream::connect((plan.address.as_str(), plan.port)),
        ) => {
            result
                .map_err(|_| AppError::platform("the VNC connection timed out"))?
                .map_err(|error| AppError::platform(format!("cannot reach the VNC server: {error}")))?
        }
    };
    let password = plan.password.expose().to_string();
    let pixel_format = PixelFormat::rgba();
    let mut state = VncConnector::new(stream)
        .set_auth_method(async move { Ok(password) })
        .set_pixel_format(pixel_format)
        .allow_shared(plan.shared)
        .add_encoding(VncEncoding::Zrle)
        .add_encoding(VncEncoding::Trle)
        .add_encoding(VncEncoding::CopyRect)
        .add_encoding(VncEncoding::Raw)
        .add_encoding(VncEncoding::DesktopSizePseudo)
        .build()
        .map_err(|error| AppError::platform(format!("VNC configuration failed: {error}")))?;
    for _ in 0..8 {
        state = state
            .try_start()
            .await
            .map_err(|error| AppError::platform(format!("VNC handshake failed: {error}")))?;
        match state {
            VncState::Connected(client) => return Ok(client),
            VncState::Authenticate(connector) => state = VncState::Authenticate(connector),
            VncState::Handshake(connector) => state = VncState::Handshake(connector),
        }
    }
    Err(AppError::platform("the VNC handshake did not converge"))
}

/// Press/release pairs for X11 keycodes used by common VNC servers.
fn special_events(key: crate::manager::SpecialKey) -> Vec<X11Event> {
    const CTRL: u32 = 37;
    const ALT: u32 = 64;
    const ESC: u32 = 9;
    const DEL: u32 = 107;
    const F4: u32 = 70;
    let keys: &[u32] = match key {
        crate::manager::SpecialKey::CtrlAltDel => &[CTRL, ALT, DEL],
        crate::manager::SpecialKey::CtrlEsc => &[CTRL, ESC],
        crate::manager::SpecialKey::AltF4 => &[ALT, F4],
    };
    let mut events = Vec::with_capacity(keys.len() * 2);
    for keycode in keys {
        events.push(X11Event::KeyEvent(ClientKeyEvent {
            keycode: *keycode,
            down: true,
        }));
    }
    for keycode in keys.iter().rev() {
        events.push(X11Event::KeyEvent(ClientKeyEvent {
            keycode: *keycode,
            down: false,
        }));
    }
    events
}

#[derive(Debug, Default)]
struct Framebuffer {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl Framebuffer {
    fn resize(&mut self, width: u32, height: u32) -> AppResult<bool> {
        if width == 0 || height == 0 || width > 4096 || height > 4096 {
            return Err(AppError::validation("invalid VNC desktop size"));
        }
        if self.width == width && self.height == height {
            return Ok(false);
        }
        self.width = width;
        self.height = height;
        self.pixels = vec![0; (width * height * 4) as usize];
        Ok(true)
    }

    fn blit(&mut self, rect: Rect, data: &[u8]) -> AppResult<()> {
        if rect.x as u32 + rect.width as u32 > self.width
            || rect.y as u32 + rect.height as u32 > self.height
        {
            return Err(AppError::validation("VNC update exceeds the framebuffer"));
        }
        let row = rect.width as usize * 4;
        if data.len() < row * rect.height as usize {
            return Err(AppError::validation("VNC update is truncated"));
        }
        let stride = self.width as usize * 4;
        for line in 0..rect.height as usize {
            let source = &data[line * row..line * row + row];
            let target = (rect.y as usize + line) * stride + rect.x as usize * 4;
            self.pixels[target..target + row].copy_from_slice(source);
        }
        Ok(())
    }

    fn copy(&mut self, destination: Rect, source: Rect) {
        let stride = self.width as usize * 4;
        let width = destination.width.min(source.width) as usize;
        let height = destination.height.min(source.height) as usize;
        let mut rows = Vec::with_capacity(height);
        for line in 0..height {
            let start = (source.y as usize + line) * stride + source.x as usize * 4;
            rows.push(self.pixels[start..start + width * 4].to_vec());
        }
        for (line, row) in rows.into_iter().enumerate() {
            let target = (destination.y as usize + line) * stride + destination.x as usize * 4;
            self.pixels[target..target + width * 4].copy_from_slice(&row);
        }
    }
}

/// Applies one RFB event. Returns `Some` when the framebuffer was published.
fn apply_event(event: VncEvent, framebuffer: &mut Framebuffer) -> AppResult<Option<u64>> {
    match event {
        VncEvent::SetResolution(screen) => {
            let changed = framebuffer.resize(u32::from(screen.width), u32::from(screen.height))?;
            Ok(changed.then_some(1))
        }
        VncEvent::DesktopUpdate(update) => {
            let Some(layout) = update.layout else {
                return Ok(None);
            };
            let changed = framebuffer.resize(u32::from(layout.width), u32::from(layout.height))?;
            Ok(changed.then_some(1))
        }
        VncEvent::SetPixelFormat(_) => Ok(None),
        VncEvent::RawImage(rect, data) => {
            if framebuffer.width == 0 || framebuffer.height == 0 {
                return Ok(None);
            }
            framebuffer.blit(rect, &data)?;
            Ok(Some(1))
        }
        VncEvent::Copy(destination, source) => {
            framebuffer.copy(destination, source);
            Ok(Some(1))
        }
        VncEvent::JpegImage(..) | VncEvent::SetCursor(..) | VncEvent::Bell => Ok(None),
        VncEvent::Text(_) => Ok(None),
        VncEvent::Error(message) => Err(AppError::platform(message)),
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framebuffer_blits_and_copies_pixels() {
        let mut framebuffer = Framebuffer::default();
        framebuffer.resize(2, 2).unwrap();
        let red = [255, 0, 0, 255];
        framebuffer
            .blit(
                Rect {
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                },
                &red,
            )
            .unwrap();
        assert_eq!(&framebuffer.pixels[..4], &red);
        framebuffer.copy(
            Rect {
                x: 1,
                y: 1,
                width: 1,
                height: 1,
            },
            Rect {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            },
        );
        assert_eq!(&framebuffer.pixels[12..16], &red);
    }

    #[test]
    fn framebuffer_rejects_out_of_bounds_updates() {
        let mut framebuffer = Framebuffer::default();
        framebuffer.resize(2, 2).unwrap();
        assert!(
            framebuffer
                .blit(
                    Rect {
                        x: 1,
                        y: 1,
                        width: 4,
                        height: 4
                    },
                    &[0; 64]
                )
                .is_err()
        );
        assert!(framebuffer.resize(0, 2).is_err());
        assert!(framebuffer.resize(9000, 9000).is_err());
    }

    #[test]
    fn resolution_changes_resize_the_canvas() {
        let mut framebuffer = Framebuffer::default();
        let changed = apply_event(
            VncEvent::SetResolution(vnc::Screen {
                width: 3,
                height: 2,
            }),
            &mut framebuffer,
        )
        .unwrap();
        assert!(changed.is_some());
        assert_eq!((framebuffer.width, framebuffer.height), (3, 2));
        assert_eq!(framebuffer.pixels.len(), 3 * 2 * 4);
    }
}
