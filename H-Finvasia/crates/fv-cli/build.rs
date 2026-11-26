//! Build stamp for `--version`.
//!
//! Emits `FV_BUILD_DATE=YYYY-MM-DD` so a released binary can report the day it
//! was built, which `scripts/release.ps1` reads back to name the artifact.
//!
//! Dependency-free and host-agnostic: the calendar conversion lives in
//! `src/date.rs` (shared with the crate's tests) and `SOURCE_DATE_EPOCH` is
//! honoured for reproducible builds.

include!("src/date.rs");

use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/date.rs");
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");

    let epoch_secs = std::env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
        });

    let (year, month, day) = civil_from_days((epoch_secs / 86_400) as i64);
    println!("cargo:rustc-env=FV_BUILD_DATE={year:04}-{month:02}-{day:02}");
}
