use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=assets/hcompare.ico");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/index");

    let hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| "unknown".to_string());

    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let date = utc_date(seconds);

    println!("cargo:rustc-env=HCOMPARE_BUILD_DATE={date}");
    println!("cargo:rustc-env=HCOMPARE_GIT_HASH={hash}");
    println!("cargo:rustc-env=HCOMPARE_BUILD_STAMP={date} {hash}");

    compile_windows_resources();
}

#[cfg(windows)]
fn compile_windows_resources() {
    let parts: Vec<u16> = env!("CARGO_PKG_VERSION")
        .split('.')
        .filter_map(|part| part.parse().ok())
        .collect();
    let version = ((parts.first().copied().unwrap_or(0) as u64) << 16)
        | ((parts.get(1).copied().unwrap_or(0) as u64) << 8)
        | parts.get(2).copied().unwrap_or(0) as u64;

    let mut resource = winres::WindowsResource::new();
    resource.set_icon("assets/hcompare.ico");
    resource.set_version_info(winres::VersionInfo::FILEVERSION, version);
    resource.set_version_info(winres::VersionInfo::PRODUCTVERSION, version);
    resource.set("ProductName", "H-Compare");
    resource.set(
        "FileDescription",
        "H-Compare folder, text, Word, image and hex comparison",
    );
    resource.set("OriginalFilename", "hcompare.exe");
    resource.set("InternalName", "hcompare");
    resource.set("LegalCopyright", "Copyright (c) 2026");
    if let Err(error) = resource.compile() {
        panic!("failed to embed the Windows icon and version resources: {error}");
    }
}

#[cfg(not(windows))]
fn compile_windows_resources() {}

/// UTC date as YYYY-MM-DD, without pulling a date crate into the build.
fn utc_date(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Howard Hinnant's civil-from-days algorithm.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}
