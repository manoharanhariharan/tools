//! Integration coverage for the public session surface: plan → manager →
//! shared frames → events → teardown.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use hr_domain::Server;
use hr_session::{
    Backend, BackendContext, BackendEvent, BackendOutcome, BackendRegistry, RdpPlan,
    ReconnectPolicy, SessionBackend, SessionEvent, SessionManager, SessionState,
};

struct FrameBackend;

impl Backend for FrameBackend {
    fn kind(&self) -> SessionBackend {
        SessionBackend::Synthetic
    }

    fn run<'a>(
        &'a self,
        context: BackendContext<'a>,
    ) -> Pin<Box<dyn Future<Output = BackendOutcome> + Send + 'a>> {
        Box::pin(async move {
            context
                .frames
                .publish(
                    2,
                    2,
                    &[
                        10, 20, 30, 255, 40, 50, 60, 255, 70, 80, 90, 255, 1, 2, 3, 255,
                    ],
                )
                .expect("publish frame");
            let _ = context.events.send(BackendEvent::Connected);
            let _ = context.events.send(BackendEvent::Frame {
                seq: 1,
                width: 2,
                height: 2,
            });
            context.cancel.cancelled().await;
            BackendOutcome::Cancelled
        })
    }
}

struct IdleBackend;

impl Backend for IdleBackend {
    fn kind(&self) -> SessionBackend {
        SessionBackend::Mstsc
    }

    fn run<'a>(
        &'a self,
        context: BackendContext<'a>,
    ) -> Pin<Box<dyn Future<Output = BackendOutcome> + Send + 'a>> {
        Box::pin(async move {
            context.cancel.cancelled().await;
            BackendOutcome::Cancelled
        })
    }
}

fn plan() -> RdpPlan {
    let mut server = Server::new_rdp("Integration", "127.0.0.1", 3390);
    let Server::Rdp(rdp) = &mut server else {
        unreachable!()
    };
    rdp.rdp_window_resize_mode = hr_domain::RdpWindowResizeMode::Fixed;
    rdp.rdp_width = 800;
    rdp.rdp_height = 600;
    RdpPlan::from_server(&server, (800, 600), 100, ReconnectPolicy::default()).unwrap()
}

async fn next_state(
    receiver: &mut tokio::sync::broadcast::Receiver<SessionEvent>,
    expected: SessionState,
) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        assert!(!left.is_zero(), "timed out waiting for {expected:?}");
        if let Ok(Ok(SessionEvent::State { session })) =
            tokio::time::timeout(left, receiver.recv()).await
            && session.state == expected
        {
            return;
        }
    }
}

#[tokio::test]
async fn lifecycle_publishes_frames_and_cancels_cleanly() {
    let manager = SessionManager::new(BackendRegistry::new(
        Arc::new(FrameBackend),
        Arc::new(IdleBackend),
    ));
    let mut events = manager.subscribe();
    let snapshot = manager.start(plan()).unwrap();
    assert_eq!(snapshot.backend, SessionBackend::IronRdp);
    assert_eq!(snapshot.width, 800);

    next_state(&mut events, SessionState::Connected).await;
    let frame = manager.frame(&snapshot.id).unwrap().expect("frame");
    assert_eq!((frame.width, frame.height), (2, 2));
    assert_eq!(&frame.rgba[..4], &[10, 20, 30, 255]);

    if cfg!(windows) {
        assert!(
            manager
                .frame_ring(&snapshot.id)
                .unwrap()
                .expect("ring")
                .starts_with("Local\\HRemoteFrame-")
        );
    }

    manager.cancel(&snapshot.id).unwrap();
    next_state(&mut events, SessionState::Disconnected).await;
    let terminal = manager.snapshot(&snapshot.id).unwrap();
    assert_eq!(
        terminal.reason,
        Some(hr_session::DisconnectReason::LocalCancel)
    );
    assert!(manager.remove(&snapshot.id).unwrap());
    assert!(manager.snapshot(&snapshot.id).is_err());
}

#[tokio::test]
async fn cancel_before_the_backend_connects_is_still_terminal() {
    let manager = SessionManager::new(BackendRegistry::new(
        Arc::new(IdleBackend),
        Arc::new(IdleBackend),
    ));
    let mut events = manager.subscribe();
    let snapshot = manager.start(plan()).unwrap();
    next_state(&mut events, SessionState::Connecting).await;
    manager.cancel(&snapshot.id).unwrap();
    next_state(&mut events, SessionState::Disconnected).await;
    assert!(manager.snapshot(&snapshot.id).unwrap().ended_at.is_some());
}

#[tokio::test]
async fn cancelling_an_unknown_session_reports_not_found() {
    let manager = SessionManager::new(BackendRegistry::new(
        Arc::new(IdleBackend),
        Arc::new(IdleBackend),
    ));
    assert!(manager.cancel("missing").is_err());
    assert!(manager.snapshot("missing").is_err());
}

/// Manual smoke test against a real RDP host. Run with:
/// `HREMOTE_RDP_HOST=host:port HREMOTE_RDP_USER=user HREMOTE_RDP_PASSWORD=...`
/// `cargo test -p hr-session --test session_flow -- --ignored`.
#[tokio::test]
#[ignore = "requires a reachable RDP host and credentials"]
async fn connects_to_a_live_rdp_host() {
    let destination = std::env::var("HREMOTE_RDP_HOST").expect("HREMOTE_RDP_HOST must be set");
    let username = std::env::var("HREMOTE_RDP_USER").unwrap_or_else(|_| "Administrator".into());
    let password = std::env::var("HREMOTE_RDP_PASSWORD").expect("HREMOTE_RDP_PASSWORD");
    let (host, port) = match destination.rsplit_once(':') {
        Some((host, port)) if port.parse::<u16>().is_ok() => {
            (host.to_string(), port.parse().unwrap())
        }
        _ => (destination, 3389),
    };
    let mut server = Server::new_rdp("Live", host, port);
    let Server::Rdp(rdp) = &mut server else {
        unreachable!()
    };
    rdp.credentials.username = username;
    rdp.credentials.password = password.into();
    rdp.rdp_window_resize_mode = hr_domain::RdpWindowResizeMode::Fixed;
    rdp.rdp_width = 1024;
    rdp.rdp_height = 768;

    let plan = RdpPlan::from_server(&server, (1024, 768), 100, ReconnectPolicy::default()).unwrap();
    let manager = SessionManager::new(BackendRegistry::new(
        Arc::new(hr_session::IronRdpBackend),
        Arc::new(hr_session::MstscBackend),
    ));
    let mut events = manager.subscribe();
    let snapshot = manager.start(plan).unwrap();

    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        assert!(!left.is_zero(), "live RDP session did not connect");
        match tokio::time::timeout(left, events.recv()).await {
            Ok(Ok(SessionEvent::State { session })) if session.state == SessionState::Connected => {
                break;
            }
            Ok(Ok(SessionEvent::State { session })) if session.state == SessionState::Failed => {
                panic!("connection failed: {:?}", session.message);
            }
            _ => continue,
        }
    }
    let frame = manager.frame(&snapshot.id).unwrap();
    assert!(frame.is_some(), "no frame was produced");
    manager.cancel(&snapshot.id).unwrap();
    next_state(&mut events, SessionState::Disconnected).await;
}
