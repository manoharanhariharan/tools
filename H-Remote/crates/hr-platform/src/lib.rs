//! Platform-specific helpers behind capability detection (P0-05/P0-32/P0-56).

pub mod autostart;
pub mod capabilities;
pub mod cli;
pub mod display;
pub mod dpapi;
pub mod hotkey;
pub mod memory;
pub mod network;
pub mod scripts;
pub mod shortcuts;
pub mod update;
pub mod verification;

pub use capabilities::{Capabilities, ShortcutFlavor, detect_capabilities};
pub use cli::{CliAction, CliInvocation, parse_cli_args};
pub use display::{MonitorInfo, PlacementDecision, screen_layout_signature, validate_placement};
pub use dpapi::{protect_local_machine, rdp_password_blob, unprotect_local_machine};
pub use hotkey::{
    HotkeyError, HotkeyModifier, HotkeySpec, canonical_hotkey, check_hotkey_conflict,
};
pub use memory::{TrimOutcome, trim_working_set};
pub use network::{tcp_latency, tcp_reachable};
pub use scripts::{ScriptEnvironment, ScriptResult, run_script, run_script_with_env};
pub use shortcuts::{
    ShortcutSpec, default_desktop_dir, remove_desktop_shortcut, shortcut_file_name,
    write_desktop_shortcut,
};
pub use update::{
    UpdateDecision, UpdateManifest, Version, evaluate as evaluate_update,
    evaluate_json as evaluate_update_json,
};
pub use verification::{VerificationResult, verify_user};
