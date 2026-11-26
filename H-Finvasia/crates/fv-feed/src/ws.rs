//! Live WebSocket transport.
//!
//! Two properties matter more than raw speed (see `fv-features.md` §7.6):
//!
//! 1. **Order updates are never queued behind market data.** The socket reader
//!    peeks the `t` field first and hands `om` frames straight to the order
//!    channel before any market-data parsing happens.
//! 2. **Subscription set is restored on every (re)connect** — the venue does not
//!    persist subscriptions across sockets.
//!
//! Reconnection uses exponential backoff with jitter so many clients cannot
//! synchronise their retries after a venue restart.

use crate::frame::{parse, peek_kind, Frame, FrameKind};
use crate::subscription::{connect_frame, subscribe_frames, DEFAULT_CHUNK};
use crate::FeedError;
use futures_util::{SinkExt, StreamExt};
use fv_core::SymbolKey;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

/// Connection parameters. Tokens come from the OAuth flow and must be supplied
/// by the caller — this crate never reads credentials from disk.
#[derive(Debug, Clone)]
pub struct WsConfig {
    pub url: String,
    pub uid: String,
    pub actid: String,
    pub access_token: String,
    pub source: String,
    pub subscribe_chunk: usize,
    pub handshake_timeout: Duration,
}

impl WsConfig {
    pub fn new(
        uid: impl Into<String>,
        actid: impl Into<String>,
        access_token: impl Into<String>,
    ) -> Self {
        WsConfig {
            url: "wss://api.shoonya.com/NorenWSAPI/".to_string(),
            uid: uid.into(),
            actid: actid.into(),
            access_token: access_token.into(),
            source: "API".to_string(),
            subscribe_chunk: DEFAULT_CHUNK,
            handshake_timeout: Duration::from_secs(15),
        }
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
///
/// This is the pure, testable core of the fast path.
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

/// One connected session. Returns when the socket closes or errors.
pub async fn run_session<F>(
    cfg: &WsConfig,
    keys: &[SymbolKey],
    mut on_frame: F,
) -> Result<(), FeedError>
where
    F: FnMut(Frame),
{
    let (mut ws, _resp) = tokio_tungstenite::connect_async(cfg.url.as_str())
        .await
        .map_err(|e| FeedError::Transport(e.to_string()))?;

    // 1. Connect handshake.
    ws.send(Message::Text(
        connect_frame(&cfg.uid, &cfg.actid, &cfg.access_token, &cfg.source).into(),
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
                    Frame::Ack(a) if a.kind == "ak" || a.kind == "ck" => {
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
            Message::Ping(_) => { /* tungstenite auto-responds on read */ }
            Message::Close(_) => {
                return Err(FeedError::HandshakeRejected(
                    "closed during handshake".into(),
                ))
            }
            _ => {}
        }
    }

    // 2. Subscribe. Every reconnect repeats this — nothing persists server-side.
    for frame in subscribe_frames(keys, cfg.subscribe_chunk) {
        ws.send(Message::Text(frame.into()))
            .await
            .map_err(|e| FeedError::Transport(e.to_string()))?;
    }

    // 3. Read loop.
    loop {
        let msg = match ws.next().await {
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
}

/// Backoff schedule: 1 s, 2 s, 4 s … capped at 60 s, with ±20 % jitter.
///
/// The base is capped at 50 s so that the +20 % jitter band still lands at or
/// below the 60 s ceiling; the final value is clamped as a hard guarantee.
///
/// Deterministic in tests via an injected `jitter` in `[0,1]` so the schedule is
/// verifiable without a RNG dependency.
pub fn backoff_delay(attempt: u32, jitter: f64) -> Duration {
    let base_ms = 1_000u64.saturating_mul(2u64.saturating_pow(attempt.min(6)));
    let capped = base_ms.min(50_000) as f64;
    // jitter in [0.8, 1.2]
    let scale = 0.8 + (jitter.clamp(0.0, 1.0) * 0.4);
    let ms = ((capped * scale) as u64).min(60_000);
    Duration::from_millis(ms)
}

/// Run forever, reconnecting with backoff and re-subscribing each time.
///
/// `on_frame` is invoked from the socket task; it must be cheap or forward to a
/// channel. Blocking here stalls the connection.
pub async fn run_forever<F>(
    cfg: WsConfig,
    keys: Vec<SymbolKey>,
    mut on_frame: F,
    shutdown: mpsc::Receiver<()>,
) -> Result<(), FeedError>
where
    F: FnMut(Frame) + Send,
{
    let mut shutdown = shutdown;
    let mut attempt: u32 = 0;
    loop {
        if shutdown.try_recv().is_ok() {
            return Ok(());
        }
        tracing::info!(attempt, keys = keys.len(), "connecting feed");
        let result = run_session(&cfg, &keys, &mut on_frame).await;
        match result {
            Ok(()) => {
                tracing::warn!("feed closed by peer");
                attempt = 0;
            }
            Err(e) => {
                tracing::warn!(error = %e, "feed error");
            }
        }
        let delay = backoff_delay(attempt, pseudo_jitter(attempt));
        tracing::info!(?delay, "reconnecting");
        tokio::select! {
            _ = tokio::time::sleep(delay) => {}
            _ = shutdown.recv() => return Ok(()),
        }
        attempt = attempt.saturating_add(1);
    }
}

/// Cheap jitter without pulling in a RNG dependency: mixes the attempt counter
/// with the process clock. Only used to de-synchronise retries.
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
        let a0 = backoff_delay(0, 0.5);
        let a1 = backoff_delay(1, 0.5);
        let a2 = backoff_delay(2, 0.5);
        assert!(a0 < a1 && a1 < a2);
        assert!(a2 <= Duration::from_millis(60_000));
        let capped = backoff_delay(20, 1.0);
        assert!(capped <= Duration::from_millis(60_000));
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
    fn connect_frame_is_valid_json() {
        let cfg = WsConfig::new("AB1234", "AB1234", "token");
        let f: serde_json::Value = serde_json::from_str(&connect_frame(
            &cfg.uid,
            &cfg.actid,
            &cfg.access_token,
            &cfg.source,
        ))
        .unwrap();
        assert_eq!(f["t"], "a");
        assert_eq!(f["source"], "API");
    }
}
