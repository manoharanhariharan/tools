//! Core, UI-agnostic building blocks shared by every H-Remote crate.

pub mod app_info;
pub mod error;
pub mod id;
pub mod registry;

pub use app_info::AppInfo;
pub use error::{AppError, AppResult, ErrorCode, IpcError};
pub use id::{is_valid_id, new_id, parse_id};
pub use registry::{
    ProtocolCapabilities, ProtocolCategory, ProtocolDescriptor, ProtocolRegistration, SessionKind,
    all_protocols, get_protocol, validate_registry,
};
