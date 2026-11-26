//! Shoonya WebSocket feed: framing, diff-merge, subscription planning, replay.
//!
//! Reference: <https://www.shoonya.com/api-documentation/websocket-overview>
//!
//! Frame types handled explicitly (documented by the venue):
//! * `t:"a"`  connect request (client → server)
//! * `t:"ak"` connect acknowledgement
//! * `t:"ck"` socket-open acknowledgement
//! * `t:"t"`  touchline subscribe (client → server)
//! * `t:"tk"` touchline full snapshot (once per subscribed instrument)
//! * `t:"tf"` touchline incremental diff (only changed fields)
//! * `t:"u"` / `t:"uk"` unsubscribe request / ack
//! * `t:"om"` order lifecycle update
//!
//! Anything else is surfaced as [`Frame::Unknown`] rather than dropped, so a
//! venue-side addition is observable instead of silently lost.

pub mod book;
pub mod frame;
pub mod replay;
pub mod subscription;
#[cfg(feature = "ws")]
pub mod ws;

pub use book::{Applied, QuoteBook};
pub use frame::{parse, peek_kind, Frame, FrameKind, OrderUpdate, Touchline};
pub use replay::{replay_into, Replay, ReplayStats};
pub use subscription::{subscribe_frames, unsubscribe_frames, Plan, WatchSet};

/// Errors raised while decoding or transporting frames.
#[derive(Debug, thiserror::Error)]
pub enum FeedError {
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid frame: {0}")]
    InvalidFrame(String),
    #[error("handshake rejected: {0}")]
    HandshakeRejected(String),
    #[error("websocket: {0}")]
    Transport(String),
}
