//! RDP session engine.
//!
//! - [`plan`] turns stored RDP settings into a backend-neutral plan.
//! - [`manager`] owns session records, retries and events.
//! - [`frames`] is a bounded latest-frame store; [`shared`] adds the Windows
//!   named shared-memory ring for the webview transport.
//! - [`backend`] drives IronRDP; [`mstsc`] is the external Windows fallback.

pub mod backend;
pub mod frames;
pub mod manager;
pub mod mstsc;
pub mod plan;
pub mod process;
pub mod process_backend;
pub mod rdp_input;
pub mod session_plan;
pub mod shared;
pub mod state;
pub mod vnc_backend;

pub use backend::IronRdpBackend;
pub use frames::{Frame, FrameTransport, HeapFrameStore, MAX_FRAME_BYTES};
pub use manager::{
    Backend, BackendContext, BackendEvent, BackendOutcome, BackendRegistry, CancelToken,
    SessionCommand, SessionManager, SpecialKey,
};
pub use mstsc::{MstscBackend, rdp_file_content, write_rdp_file};
pub use plan::{
    DEFAULT_RECONNECT_ATTEMPTS, DisplayPlan, EngineChoice, GatewayPlan, RdpPlan, ReconnectPolicy,
    RedirectPlan,
};
pub use process::{ProcessKind, ProcessPlan, expand_runner_template, split_arguments};
pub use process_backend::ProcessBackend;
pub use session_plan::{SessionPlan, VncPlan};
pub use shared::SharedFrameRing;
pub use state::{
    DisconnectReason, FrameInfo, SessionBackend, SessionEvent, SessionSnapshot, SessionState,
};
pub use vnc_backend::VncBackend;
