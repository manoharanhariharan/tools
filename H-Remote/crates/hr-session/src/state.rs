//! Session lifecycle state machine and user-visible events.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum SessionState {
    #[default]
    NotInitialized,
    Initializing,
    Connecting,
    Connected,
    Reconnecting,
    Disconnecting,
    Disconnected,
    Failed,
}

impl SessionState {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Disconnected | Self::Failed)
    }

    pub fn is_active(self) -> bool {
        matches!(
            self,
            Self::Initializing
                | Self::Connecting
                | Self::Connected
                | Self::Reconnecting
                | Self::Disconnecting
        )
    }

    /// Rejects impossible transitions (e.g. `Connected` directly from `Failed`).
    pub fn can_transition_to(self, next: Self) -> bool {
        use SessionState::{
            Connected, Connecting, Disconnected, Disconnecting, Failed, Initializing,
            NotInitialized, Reconnecting,
        };
        match self {
            NotInitialized => matches!(next, Initializing | Connecting | Failed | Disconnected),
            Initializing => matches!(next, Connecting | Failed | Disconnected | Disconnecting),
            Connecting => matches!(
                next,
                Connected | Failed | Disconnected | Reconnecting | Disconnecting
            ),
            Connected => matches!(next, Disconnecting | Disconnected | Reconnecting | Failed),
            Reconnecting => matches!(
                next,
                Connected | Connecting | Failed | Disconnected | Disconnecting
            ),
            Disconnecting => matches!(next, Disconnected | Failed),
            Disconnected | Failed => matches!(next, Reconnecting | Connecting | Initializing),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DisconnectReason {
    LocalCancel,
    RemoteUser,
    RemoteServer,
    IdleTimeout,
    ReplacedByAnotherConnection,
    Logoff,
    ApiDisconnect,
    NetworkLoss,
    AuthenticationFailed,
    ConnectionFailed,
    Timeout,
    NoInfo,
}

impl DisconnectReason {
    pub fn message(self) -> &'static str {
        match self {
            Self::LocalCancel => "Disconnected locally",
            Self::RemoteUser => "The remote user ended the session",
            Self::RemoteServer => "The remote server ended the session",
            Self::IdleTimeout => "The session timed out while idle",
            Self::ReplacedByAnotherConnection => "Another connection replaced this session",
            Self::Logoff => "The remote user logged off",
            Self::ApiDisconnect => "The session was disconnected by an API call",
            Self::NetworkLoss => "The network connection was lost",
            Self::AuthenticationFailed => "Authentication failed",
            Self::ConnectionFailed => "The connection could not be established",
            Self::Timeout => "The connection attempt timed out",
            Self::NoInfo => "Disconnected",
        }
    }

    pub fn is_retryable(self) -> bool {
        matches!(
            self,
            Self::NetworkLoss | Self::ConnectionFailed | Self::Timeout | Self::RemoteServer
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum SessionBackend {
    #[default]
    IronRdp,
    Mstsc,
    Vnc,
    Terminal,
    LocalApp,
    RemoteApp,
    FileTransfer,
    Synthetic,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSnapshot {
    pub id: String,
    pub server_id: String,
    pub server_name: String,
    pub backend: SessionBackend,
    pub state: SessionState,
    pub reason: Option<DisconnectReason>,
    pub message: Option<String>,
    pub attempt: u32,
    pub max_attempts: u32,
    pub width: u32,
    pub height: u32,
    pub frame_count: u64,
    pub started_at: i64,
    pub connected_at: Option<i64>,
    pub ended_at: Option<i64>,
    /// Name of the shared-memory frame ring, if the backend produces frames.
    pub frame_ring: Option<String>,
}

impl SessionSnapshot {
    pub fn is_terminal(&self) -> bool {
        self.state.is_terminal()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
pub enum SessionCursor {
    /// Use the platform's normal pointer.
    Default,
    /// The remote session asked for the pointer to be hidden.
    Hidden,
    /// A cursor bitmap supplied by the remote session. CSS `cursor` URLs are
    /// interpreted by the webview with the hotspot as the click origin.
    Image {
        data_url: String,
        hotspot_x: u16,
        hotspot_y: u16,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
pub enum SessionEvent {
    State {
        session: SessionSnapshot,
    },
    Frame {
        session_id: String,
        seq: u64,
        width: u32,
        height: u32,
    },
    Clipboard {
        session_id: String,
        text: String,
    },
    Cursor {
        session_id: String,
        cursor: SessionCursor,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameInfo {
    pub seq: u64,
    pub width: u32,
    pub height: u32,
}

/// Guards every transition in one place so a backend cannot publish an
/// impossible sequence.
#[derive(Debug, Clone)]
pub struct StateMachine {
    state: SessionState,
}

impl Default for StateMachine {
    fn default() -> Self {
        Self {
            state: SessionState::NotInitialized,
        }
    }
}

impl StateMachine {
    pub fn state(&self) -> SessionState {
        self.state
    }

    pub fn transition(&mut self, next: SessionState) -> bool {
        // Re-announcing the current state is a no-op, not an invalid edge. This
        // keeps duplicate backend notifications from spamming warnings.
        if self.state == next {
            return true;
        }
        if !self.state.can_transition_to(next) {
            tracing::warn!(
                from = ?self.state,
                to = ?next,
                "rejected session state transition"
            );
            return false;
        }
        self.state = next;
        true
    }

    /// Forces a terminal state; used for shutdown/cleanup.
    pub fn force(&mut self, next: SessionState) {
        self.state = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path_transitions_are_allowed() {
        let mut machine = StateMachine::default();
        assert!(machine.transition(SessionState::Initializing));
        assert!(machine.transition(SessionState::Connecting));
        assert!(machine.transition(SessionState::Connected));
        assert!(machine.transition(SessionState::Disconnecting));
        assert!(machine.transition(SessionState::Disconnected));
        assert_eq!(machine.state(), SessionState::Disconnected);
    }

    #[test]
    fn reconnect_path_is_allowed_and_failure_is_terminal() {
        let mut machine = StateMachine::default();
        machine.transition(SessionState::Connecting);
        machine.transition(SessionState::Connected);
        assert!(machine.transition(SessionState::Reconnecting));
        assert!(machine.transition(SessionState::Connecting));
        assert!(machine.transition(SessionState::Failed));
        assert!(machine.state().is_terminal());
    }

    #[test]
    fn impossible_transitions_are_rejected() {
        let mut machine = StateMachine::default();
        assert!(!machine.transition(SessionState::Connected));
        assert_eq!(machine.state(), SessionState::NotInitialized);
        assert!(!SessionState::Connected.can_transition_to(SessionState::Initializing));
        assert!(!SessionState::Disconnected.can_transition_to(SessionState::Connected));
    }

    #[test]
    fn reannouncing_the_current_state_is_a_noop() {
        let mut machine = StateMachine::default();
        assert!(machine.transition(SessionState::Connecting));
        assert!(machine.transition(SessionState::Connected));
        assert!(machine.transition(SessionState::Connected));
        assert!(machine.transition(SessionState::Connected));
        assert_eq!(machine.state(), SessionState::Connected);
    }

    #[test]
    fn reason_messages_and_retryability() {
        assert_eq!(
            DisconnectReason::LocalCancel.message(),
            "Disconnected locally"
        );
        assert!(DisconnectReason::NetworkLoss.is_retryable());
        assert!(!DisconnectReason::LocalCancel.is_retryable());
        assert!(!DisconnectReason::AuthenticationFailed.is_retryable());
    }

    #[test]
    fn cursor_events_serialise_with_a_kind_tag() {
        let event = SessionEvent::Cursor {
            session_id: "s".into(),
            cursor: SessionCursor::Image {
                data_url: "data:image/png;base64,AA".into(),
                hotspot_x: 1,
                hotspot_y: 2,
            },
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["kind"], "cursor");
        assert_eq!(json["cursor"]["kind"], "image");
        assert_eq!(json["cursor"]["hotspotX"], 1);
    }

    #[test]
    fn events_serialise_with_a_kind_tag() {
        let event = SessionEvent::Frame {
            session_id: "s".into(),
            seq: 3,
            width: 800,
            height: 600,
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["kind"], "frame");
        assert_eq!(json["seq"], 3);
        assert_eq!(json["sessionId"], "s");
        assert_eq!(json["width"], 800);
    }
}
