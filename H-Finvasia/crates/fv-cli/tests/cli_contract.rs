//! Integration tests for the shipped binary's CLI contract.
//!
//! These guard the two things `scripts/release.ps1` relies on:
//! * `--version` prints `h-finvasia <semver> build YYYY-MM-DD`;
//! * `replay` works as a standalone process with no database and no network.

use std::path::PathBuf;
use std::process::Command;

fn exe() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_h-finvasia"))
}

#[test]
fn version_reports_package_and_build_date() {
    let output = Command::new(exe())
        .arg("--version")
        .output()
        .expect("failed to run --version");
    assert!(output.status.success(), "--version must exit 0");

    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert!(
        text.starts_with("h-finvasia "),
        "--version must name the tool, got {text:?}"
    );

    // Contract parsed by scripts/release.ps1: `build (\d{4}-\d{2}-\d{2})`.
    let after_build = text
        .split(" build ")
        .nth(1)
        .unwrap_or_else(|| panic!("missing `build <date>` in {text:?}"));
    let date = after_build
        .split_whitespace()
        .next()
        .expect("missing date token");
    let parts: Vec<&str> = date.split('-').collect();
    assert_eq!(
        parts.len(),
        3,
        "build date must be YYYY-MM-DD, got {date:?}"
    );

    let year: i32 = parts[0].parse().expect("year");
    let month: u32 = parts[1].parse().expect("month");
    let day: u32 = parts[2].parse().expect("day");
    assert_eq!(parts[0].len(), 4, "year must be zero-padded");
    assert_eq!(parts[1].len(), 2, "month must be zero-padded");
    assert_eq!(parts[2].len(), 2, "day must be zero-padded");
    assert!((2020..=2100).contains(&year), "implausible year {year}");
    assert!((1..=12).contains(&month), "implausible month {month}");
    assert!((1..=31).contains(&day), "implausible day {day}");
}

#[test]
fn help_lists_every_subcommand() {
    let output = Command::new(exe())
        .arg("--help")
        .output()
        .expect("failed to run --help");
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    for expected in ["replay", "bench", "master", "doctor"] {
        assert!(text.contains(expected), "--help must mention {expected}");
    }
}

#[test]
fn replay_runs_offline_and_reports_no_errors() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("fv-feed")
        .join("tests")
        .join("fixtures")
        .join("sample_feed.jsonl");

    let output = Command::new(exe())
        .args(["replay"])
        .arg(&fixture)
        .args(["--hz", "30", "--show", "2"])
        .output()
        .expect("failed to run replay");
    assert!(
        output.status.success(),
        "replay failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("errors"), "replay must report errors");
    assert!(
        text.contains("errors          0"),
        "fixture must replay without errors, got:\n{text}"
    );
    assert!(
        text.contains("instruments     3"),
        "fixture must yield 3 instruments, got:\n{text}"
    );
}

#[test]
fn master_without_mongo_feature_fails_with_actionable_message() {
    // Only meaningful when the binary was built without the mongo feature.
    if cfg!(feature = "mongo") {
        return;
    }
    let output = Command::new(exe())
        .args(["master", "stats"])
        .output()
        .expect("failed to run master stats");
    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stderr);
    assert!(
        text.contains("--features mongo"),
        "error must explain how to enable mongo, got: {text}"
    );
}
