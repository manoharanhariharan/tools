//! Logging, crash reporting and startup timing.
//!
//! Initialisation order matters (P0-07): a caller installs the panic hook and
//! logging before touching configuration, so startup failures are always
//! recorded.

pub mod crash;
pub mod logging;
pub mod startup;

pub use crash::{
    CrashReport, format_crash_markdown, install_panic_hook, list_crash_reports, redact_secrets,
    write_crash_report,
};
pub use logging::{LogGuard, current_log_file, init_logging};
pub use startup::{Checkpoint, StartupTrace};
