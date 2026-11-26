//! Live WebSocket transport (legacy Noren handshake).
//!
//! Verified against the live venue: `wss://api.shoonya.com/NorenWSTP/` accepts
//! `{"t":"c","uid":..,"actid":..,"susertoken":..,"source":"WEB"}` and replies
//! `{"t":"ck","s":"OK"}`, then streams `tk` snapshots and `tf` diffs.
//!
//! Three properties matter more than raw speed (`fv-features.md` §7.6):
//!
//! 1. **Order updates are never queued behind market data.** The reader peeks the
//!    `t` field and hands `om` frames to the order path before parsing quotes.
//! 2. **The subscription set is live.** [`FeedCommand`] adds/removes instruments
//!    on the open socket, and the full set is re-sent after every reconnect.
//! 3. **Source scope is auto-detected.** A web-minted token only works with
//!    `source:"WEB"`; on `NOT_OK` the session retries once with `"API"`.

use crate::frame::{parse, peek_kind, Frame, FrameKind};
use crate::subscription::{
    connect_frame, subscribe_frames, unsubscribe_frames, DEFAULT_CHUNK, HEARTBEAT_FRAME,
};
use crate::FeedError;
use futures_util::{SinkExt, StreamExt};
use fv_core::SymbolKey;
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

/// Default API websocket endpoint (legacy protocol).
pub const DEFAULT_WS_URL: &str = "wss://api.shoonya.com/NorenWSTP/";

/// A change to the live subscription set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeedCommand {
    Subscribe(Vec<SymbolKey>),
    Unsubscribe(Vec<SymbolKey>),
    Shutdown,
}

/// Lifecycle notifications from [`run_forever`], so the UI can show the *real*
/// connection state instead of an optimistic guess.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeedState {
    /// Handshake accepted and subscriptions sent.
    Connected { source: String },
    /// The venue rejected the token for this scope.
    Rejected { source: String },
    /// Transport or protocol failure.
    Error { message: String },
    /// Waiting before the next attempt.
    Reconnecting { attempt: u32 },
}

/// Connection parameters. Tokens come from the caller; this crate never reads
/// credentials from disk.
#[derive(Debug, Clone)]
pub struct WsConfig {
    pub url: String,
    pub uid: String,
    pub actid: String,
    pub token: String,
    pub source: String,
    pub subscribe_chunk: usize,
    pub handshake_timeout: Duration,
    pub heartbeat: Duration,
}

impl WsConfig {
    pub fn new(uid: impl Into<String>, actid: impl Into<String>, token: impl Into<String>) -> Self {
        WsConfig {
            url: DEFAULT_WS_URL.to_string(),
            uid: uid.into(),
            actid: actid.into(),
            token: token.into(),
            // Web-minted sessions require WEB; flipped automatically on NOT_OK.
            source: "WEB".to_string(),
            subscribe_chunk: DEFAULT_CHUNK,
            handshake_timeout: Duration::from_secs(15),
            heartbeat: Duration::from_secs(3),
        }
    }

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = source.into();
        self
    }

    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = url.into();
        self
    }
}

/// Frames routed by priority. The caller drains `orders` on its own task.
#[derive(Debug, Default)]
pub struct FrameSink {
    pub market: Vec<Frame>,
    pub orders: Vec<Frame>,
    pub control: Vec<Frame>,
}

/// Split a batch of raw frames into priority buckets without parsing twice.
pub fn route_batch(raws: &[Vec<u8>]) -> FrameSink {
    let mut sink = FrameSink::default();
    for raw in raws {
        match peek_kind(raw) {
            FrameKind::OrderUpdate => match parse(raw) {
                Ok(f) => sink.orders.push(f),
                Err(e) => tracing::warn!(error = %e, "dropping undecodable order frame"),
            },
            FrameKind::Unknown => {
                tracing::debug!("unhandled frame type, ignoring");
            }
            _ => match parse(raw) {
                Ok(f) => sink.market.push(f),
                Err(e) => tracing::warn!(error = %e, "dropping undecodable market frame"),
            },
        }
    }
    sink
}

/// One-shot session with a fixed subscription set (used by the CLI harness).
pub async fn run_session<F>(
    cfg: &WsConfig,
    keys: &[SymbolKey],
    on_frame: F,
) -> Result<(), FeedError>
where
    F: FnMut(Frame),
{
    let desired: Arc<Mutex<BTreeSet<SymbolKey>>> =
        Arc::new(Mutex::new(keys.iter().cloned().collect()));
    run_session_inner(cfg, &desired, on_frame, || {}, None).await
}

/// Core session: handshake, subscribe, then stream while accepting commands.
async fn run_session_inner<F, C>(
    cfg: &WsConfig,
    desired: &Arc<Mutex<BTreeSet<SymbolKey>>>,
    mut on_frame: F,
    mut on_connected: C,
    mut commands: Option<&mut mpsc::UnboundedReceiver<FeedCommand>>,
) -> Result<(), FeedError>
where
    F: FnMut(Frame),
    C: FnMut(),
{
    let (mut ws, _resp) = tokio_tungstenite::connect_async(cfg.url.as_str())
        .await
        .map_err(|e| FeedError::Transport(e.to_string()))?;

    // 1. Handshake.
    ws.send(Message::Text(
        connect_frame(&cfg.uid, &cfg.actid, &cfg.token, &cfg.source).into(),
    ))
    .await
    .map_err(|e| FeedError::Transport(e.to_string()))?;

    let deadline = tokio::time::Instant::now() + cfg.handshake_timeout;
    loop {
        let msg = tokio::time::timeout_at(deadline, ws.next())
            .await
            .map_err(|_| FeedError::HandshakeRejected("timeout waiting for ack".into()))?
            .ok_or_else(|| FeedError::HandshakeRejected("socket closed during handshake".into()))?
            .map_err(|e| FeedError::Transport(e.to_string()))?;

        match msg {
            Message::Text(text) => {
                let bytes = text.as_str().as_bytes().to_vec();
                match parse(&bytes)? {
                    Frame::Ack(a) if a.kind == "ck" || a.kind == "ak" => {
                        if a.is_ok() {
                            break;
                        }
                        return Err(FeedError::HandshakeRejected(
                            a.status.unwrap_or_else(|| "unknown".into()),
                        ));
                    }
                    other => on_frame(other),
                }
            }
            Message::Close(_) => {
                return Err(FeedError::HandshakeRejected(
                    "closed during handshake".into(),
                ))
            }
            _ => {}
        }
    }

    // 2. Subscribe the current set. Every reconnect repeats this.
    let keys: Vec<SymbolKey> = desired.lock().unwrap().iter().cloned().collect();
    for frame in subscribe_frames(&keys, cfg.subscribe_chunk) {
        ws.send(Message::Text(frame.into()))
            .await
            .map_err(|e| FeedError::Transport(e.to_string()))?;
    }
    on_connected();

    // 3. Stream, with keepalive and live subscription commands.
    let mut heartbeat = tokio::time::interval(cfg.heartbeat);
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    heartbeat.tick().await; // consume the immediate first tick

    loop {
        // A future that never completes when no command channel is attached, so
        // the select! below stays uniform.
        let command_future = async {
            match commands.as_deref_mut() {
                Some(rx) => rx.recv().await,
                None => std::future::pending::<Option<FeedCommand>>().await,
            }
        };

        tokio::select! {
            _ = heartbeat.tick() => {
                ws.send(Message::Text(HEARTBEAT_FRAME.into()))
                    .await
                    .map_err(|e| FeedError::Transport(e.to_string()))?;
            }
            incoming = ws.next() => {
                let msg = match incoming {
                    Some(Ok(m)) => m,
                    Some(Err(e)) => return Err(FeedError::Transport(e.to_string())),
                    None => return Ok(()),
                };
                match msg {
                    Message::Text(text) => {
                        let bytes = text.as_str().as_bytes().to_vec();
                        match peek_kind(&bytes) {
                            FrameKind::OrderUpdate => {
                                if let f @ Frame::Order(_) = parse(&bytes)? {
                                    on_frame(f);
                                }
                            }
                            _ => on_frame(parse(&bytes)?),
                        }
                    }
                    Message::Ping(payload) => {
                        ws.send(Message::Pong(payload))
                            .await
                            .map_err(|e| FeedError::Transport(e.to_string()))?;
                    }
                    Message::Close(_) => return Ok(()),
                    _ => {}
                }
            }
            command = command_future => {
                match command {
                    None | Some(FeedCommand::Shutdown) => return Ok(()),
                    Some(FeedCommand::Subscribe(list)) => {
                        let fresh = {
                            let mut set = desired.lock().unwrap();
                            list.into_iter().filter(|k| set.insert(k.clone())).collect::<Vec<_>>()
                        };
                        for frame in subscribe_frames(&fresh, cfg.subscribe_chunk) {
                            ws.send(Message::Text(frame.into()))
                                .await
                                .map_err(|e| FeedError::Transport(e.to_string()))?;
                        }
                        tracing::info!(count = fresh.len(), "subscribed while live");
                    }
                    Some(FeedCommand::Unsubscribe(list)) => {
                        let gone = {
                            let mut set = desired.lock().unwrap();
                            list.into_iter().filter(|k| set.remove(k)).collect::<Vec<_>>()
                        };
                        for frame in unsubscribe_frames(&gone, cfg.subscribe_chunk) {
                            ws.send(Message::Text(frame.into()))
                                .await
                                .map_err(|e| FeedError::Transport(e.to_string()))?;
                        }
                        tracing::info!(count = gone.len(), "unsubscribed while live");
                    }
                }
            }
        }
    }
}

/// Backoff schedule: 1 s, 2 s, 4 s … capped at 60 s, with ±20 % jitter.
pub fn backoff_delay(attempt: u32, jitter: f64) -> Duration {
    let base_ms = 1_000u64.saturating_mul(2u64.saturating_pow(attempt.min(6)));
    let capped = base_ms.min(50_000) as f64;
    let scale = 0.8 + (jitter.clamp(0.0, 1.0) * 0.4);
    let ms = ((capped * scale) as u64).min(60_000);
    Duration::from_millis(ms)
}

fn apply_command(desired: &Arc<Mutex<BTreeSet<SymbolKey>>>, command: FeedCommand) {
    let mut set = desired.lock().unwrap();
    match command {
        FeedCommand::Subscribe(list) => {
            for key in list {
                set.insert(key);
            }
        }
        FeedCommand::Unsubscribe(list) => {
            for key in list {
                set.remove(&key);
            }
        }
        FeedCommand::Shutdown => {}
    }
}

/// Run forever, reconnecting with backoff and re-subscribing the current set.
///
/// On an authentication rejection the session retries once with the opposite
/// `source` scope, which handles tokens minted by either login path.
/// Commands received while disconnected update the set and take effect on the
/// next connect.
pub async fn run_forever<F, S>(
    mut cfg: WsConfig,
    keys: Vec<SymbolKey>,
    mut on_frame: F,
    mut on_state: S,
    mut commands: mpsc::UnboundedReceiver<FeedCommand>,
) -> Result<(), FeedError>
where
    F: FnMut(Frame) + Send,
    S: FnMut(FeedState) + Send,
{
    let desired: Arc<Mutex<BTreeSet<SymbolKey>>> = Arc::new(Mutex::new(keys.into_iter().collect()));
    let mut attempt: u32 = 0;
    let mut scope_tried = false;

    loop {
        // Drain anything queued while disconnected.
        loop {
            match commands.try_recv() {
                Ok(FeedCommand::Shutdown) | Err(mpsc::error::TryRecvError::Disconnected) => {
                    return Ok(())
                }
                Ok(command) => apply_command(&desired, command),
                Err(mpsc::error::TryRecvError::Empty) => break,
            }
        }

        tracing::info!(
            url = %cfg.url,
            source = %cfg.source,
            keys = desired.lock().unwrap().len(),
            "connecting feed"
        );

        let source = cfg.source.clone();
        let result = run_session_inner(
            &cfg,
            &desired,
            &mut on_frame,
            || {
                on_state(FeedState::Connected {
                    source: source.clone(),
                })
            },
            Some(&mut commands),
        )
        .await;

        match result {
            Ok(()) => {
                tracing::warn!("feed closed by peer");
                attempt = 0;
            }
            Err(err) if err.is_auth_rejected() && !scope_tried => {
                scope_tried = true;
                on_state(FeedState::Rejected {
                    source: cfg.source.clone(),
                });
                cfg.source = if cfg.source.eq_ignore_ascii_case("WEB") {
                    "API".to_string()
                } else {
                    "WEB".to_string()
                };
                tracing::warn!(retry_source = %cfg.source, "scope rejected; retrying");
                continue;
            }
            Err(err) => {
                tracing::warn!(error = %err, "feed error");
                on_state(FeedState::Error {
                    message: err.to_string(),
                });
            }
        }

        let delay = backoff_delay(attempt, pseudo_jitter(attempt));
        on_state(FeedState::Reconnecting { attempt });
        tracing::info!(?delay, "reconnecting");
        let wake = tokio::time::Instant::now() + delay;
        loop {
            tokio::select! {
                _ = tokio::time::sleep_until(wake) => break,
                command = commands.recv() => match command {
                    None | Some(FeedCommand::Shutdown) => return Ok(()),
                    Some(other) => apply_command(&desired, other),
                }
            }
        }
        attempt = attempt.saturating_add(1);
    }
}

/// Cheap jitter without a RNG dependency: mixes the attempt counter with the
/// process clock. Only used to de-synchronise retries.
fn pseudo_jitter(attempt: u32) -> f64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0) as f64;
    let mixed = (nanos + (attempt as f64) * 7.0) % 1000.0;
    mixed / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(wire: &str) -> SymbolKey {
        SymbolKey::parse(wire).unwrap()
    }

    #[test]
    fn routing_sends_orders_to_the_order_bucket_only() {
        let raws = vec![
            br#"{"t":"tf","e":"NSE","tk":"22","lp":"1.00"}"#.to_vec(),
            br#"{"t":"om","norenordno":"1","status":"COMPLETE"}"#.to_vec(),
            br#"{"t":"tk","e":"NSE","tk":"23","lp":"2.00"}"#.to_vec(),
        ];
        let sink = route_batch(&raws);
        assert_eq!(sink.orders.len(), 1);
        assert_eq!(sink.market.len(), 2);
        assert!(matches!(sink.orders[0], Frame::Order(_)));
    }

    #[test]
    fn routing_isolates_bad_frames() {
        let raws = vec![
            b"{broken".to_vec(),
            br#"{"t":"om","norenordno":"1"}"#.to_vec(),
        ];
        let sink = route_batch(&raws);
        assert_eq!(sink.orders.len(), 1, "good order survives a bad neighbour");
        assert_eq!(sink.market.len(), 0);
    }

    #[test]
    fn backoff_grows_and_caps() {
        assert!(backoff_delay(0, 0.5) < backoff_delay(1, 0.5));
        assert!(backoff_delay(1, 0.5) < backoff_delay(2, 0.5));
        assert!(backoff_delay(20, 1.0) <= Duration::from_millis(60_000));
    }

    #[test]
    fn backoff_jitter_stays_within_band() {
        for attempt in 0..6 {
            let low = backoff_delay(attempt, 0.0).as_millis() as f64;
            let high = backoff_delay(attempt, 1.0).as_millis() as f64;
            assert!(high >= low);
            assert!(
                (high / low - 1.5).abs() < 1e-9,
                "band must be 0.8–1.2 (attempt {attempt}: {low} → {high})"
            );
        }
    }

    #[test]
    fn default_config_targets_the_legacy_endpoint() {
        let cfg = WsConfig::new("u", "a", "t");
        assert_eq!(cfg.url, DEFAULT_WS_URL);
        assert_eq!(cfg.source, "WEB");
        assert_eq!(cfg.heartbeat, Duration::from_secs(3));
    }

    #[test]
    fn auth_rejection_is_detected_for_scope_fallback() {
        assert!(FeedError::HandshakeRejected("NOT_OK".into()).is_auth_rejected());
        assert!(!FeedError::HandshakeRejected("timeout".into()).is_auth_rejected());
        assert!(!FeedError::Transport("x".into()).is_auth_rejected());
    }

    #[test]
    fn commands_update_the_desired_set() {
        let desired: Arc<Mutex<BTreeSet<SymbolKey>>> =
            Arc::new(Mutex::new([key("NSE|1")].into_iter().collect()));

        apply_command(&desired, FeedCommand::Subscribe(vec![key("NSE|2")]));
        assert_eq!(desired.lock().unwrap().len(), 2);

        // Re-subscribing is idempotent.
        apply_command(&desired, FeedCommand::Subscribe(vec![key("NSE|2")]));
        assert_eq!(desired.lock().unwrap().len(), 2);

        apply_command(&desired, FeedCommand::Unsubscribe(vec![key("NSE|1")]));
        assert_eq!(desired.lock().unwrap().len(), 1);
        assert!(desired.lock().unwrap().contains(&key("NSE|2")));
    }
}
