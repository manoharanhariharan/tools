//! H-Compare entry point, CLI parsing and desktop window setup.

mod actions;
mod app;
mod csvdiff;
mod diff_common;
mod folder;
mod load;
mod model;
mod report;
mod settings;
mod textdiff;
mod ui;

use crate::model::{CompareOptions, EntryStatus};
use crate::ui::theme::ThemeMode;
use eframe::egui;
use std::path::PathBuf;
use std::sync::Arc;

fn main() {
    let args = match parse_args() {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };

    if args.help {
        print_help();
        return;
    }
    if args.version {
        println!(
            "hcompare {} (build {})",
            env!("CARGO_PKG_VERSION"),
            env!("HCOMPARE_BUILD_STAMP")
        );
        return;
    }

    if args.report {
        std::process::exit(run_report(&args));
    }

    let (left, right) = match (args.paths.first(), args.paths.get(1)) {
        (Some(left), Some(right)) => (Some(left.clone()), Some(right.clone())),
        (Some(left), None) => (Some(left.clone()), None),
        _ => (None, None),
    };
    run_gui(left, right, args.open, args.theme);
}

#[derive(Debug, Default)]
struct Args {
    paths: Vec<String>,
    report: bool,
    all: bool,
    help: bool,
    version: bool,
    open: Option<String>,
    theme: Option<ThemeMode>,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args::default();
    let mut items = std::env::args().skip(1);
    while let Some(raw) = items.next() {
        if let Some(rel) = raw.strip_prefix("--open=") {
            args.open = Some(rel.to_string());
            continue;
        }
        if let Some(theme) = raw.strip_prefix("--theme=") {
            args.theme = Some(parse_theme(theme)?);
            continue;
        }
        match raw.as_str() {
            "--report" | "-r" => args.report = true,
            "--all" => args.all = true,
            "--help" | "-h" => args.help = true,
            "--version" | "-V" => args.version = true,
            "--open" => {
                args.open = items.next();
                if args.open.is_none() {
                    return Err(format!("--open needs a relative file path.\n\n{}", usage()));
                }
            }
            "--theme" => {
                let Some(theme) = items.next() else {
                    return Err(format!("--theme needs dark or light.\n\n{}", usage()));
                };
                args.theme = Some(parse_theme(&theme)?);
            }
            other if other.starts_with('-') => {
                return Err(format!("Unknown option: {other}\n\n{}", usage()));
            }
            other => args.paths.push(other.to_string()),
        }
    }
    if args.paths.len() > 2 && !args.help {
        return Err(format!("Expected at most two folder paths.\n\n{}", usage()));
    }
    Ok(args)
}

fn parse_theme(value: &str) -> Result<ThemeMode, String> {
    match value.to_ascii_lowercase().as_str() {
        "dark" => Ok(ThemeMode::Dark),
        "light" => Ok(ThemeMode::Light),
        other => Err(format!(
            "Unknown theme '{other}'. Use dark or light.\n\n{}",
            usage()
        )),
    }
}

fn usage() -> &'static str {
    "Usage:\n  hcompare [LEFT] [RIGHT]        Open the desktop comparison\n  hcompare --report LEFT RIGHT   Compare headlessly and print a report\n\nOptions:\n  -r, --report   Headless report (exit code 1 when differences exist)\n      --all      Include common build folders in a report\n      --open P   Open relative path P after the initial scan\n      --theme T  Start with dark or light theme\n  -h, --help     Show this help\n  -V, --version  Show the version"
}

fn print_help() {
    println!(
        "H-Compare {} (build {})\n\n{}",
        env!("CARGO_PKG_VERSION"),
        env!("HCOMPARE_BUILD_STAMP"),
        usage()
    );
}

fn run_report(args: &Args) -> i32 {
    let (Some(left), Some(right)) = (args.paths.first(), args.paths.get(1)) else {
        eprintln!(
            "hcompare --report requires two folder paths.\n\n{}",
            usage()
        );
        return 2;
    };
    let mut options = CompareOptions::default();
    if args.all {
        options.skip_build_dirs = false;
    }
    match folder::compare_folders(
        PathBuf::from(left).as_path(),
        PathBuf::from(right).as_path(),
        &options,
    ) {
        Ok(tree) => {
            println!("H-Compare folder report");
            println!("Left:  {}", tree.left_root.display());
            println!("Right: {}", tree.right_root.display());
            println!();
            println!(
                "{} files: {} same, {} different, {} left-only, {} right-only, {} errors",
                tree.counts.total(),
                tree.counts.same,
                tree.counts.different,
                tree.counts.left_only,
                tree.counts.right_only,
                tree.counts.error
            );
            let differences = tree.differing_files(tree.root, usize::MAX);
            if differences.is_empty() {
                println!("No differences.");
                return 0;
            }
            println!();
            for idx in differences {
                let node = tree.node(idx);
                let marker = match node.status {
                    EntryStatus::Different => "~",
                    EntryStatus::LeftOnly => "-",
                    EntryStatus::RightOnly => "+",
                    EntryStatus::TypeMismatch => "!",
                    EntryStatus::Error => "!",
                    EntryStatus::Same => "=",
                };
                println!("{marker} {:8} {}", node.status.label(), node.rel);
            }
            println!("\n{} differing file(s).", tree.counts.differences());
            1
        }
        Err(error) => {
            eprintln!("Comparison failed: {error}");
            2
        }
    }
}

fn run_gui(
    left: Option<String>,
    right: Option<String>,
    open: Option<String>,
    theme: Option<ThemeMode>,
) {
    hide_own_console();
    let icon = make_icon();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("H-Compare")
            .with_min_inner_size([1020.0, 660.0])
            .with_maximized(true)
            .with_icon(Arc::new(icon)),
        // Window geometry persists by default; restoring an old non-maximized
        // size together with the maximized flag leaves a partially filled
        // window on Windows. Start maximized instead.
        persist_window: false,
        ..Default::default()
    };
    let result = eframe::run_native(
        "H-Compare",
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, left, right, open, theme)))),
    );
    if let Err(error) = result {
        eprintln!("H-Compare could not start: {error}");
        let _ = rfd::MessageDialog::new()
            .set_title("H-Compare could not start")
            .set_description(error.to_string())
            .set_level(rfd::MessageLevel::Error)
            .show();
    }
}

fn make_icon() -> egui::IconData {
    let size = 64usize;
    let mut rgba = vec![0u8; size * size * 4];
    let mut set = |x: usize, y: usize, color: [u8; 4]| {
        if x < size && y < size {
            let idx = (y * size + x) * 4;
            rgba[idx..idx + 4].copy_from_slice(&color);
        }
    };
    for y in 0..size {
        for x in 0..size {
            let base = if (x as i32 - y as i32).abs() < 20 {
                [26, 34, 46, 255]
            } else {
                [20, 22, 26, 255]
            };
            set(x, y, base);
        }
    }
    // Accent border.
    for x in 2..62 {
        set(x, 2, [90, 169, 255, 255]);
        set(x, 61, [90, 169, 255, 255]);
    }
    for y in 2..62 {
        set(2, y, [90, 169, 255, 255]);
        set(61, y, [90, 169, 255, 255]);
    }
    // Left and right diff bars.
    for y in 14..50 {
        let left_len = 18 + ((y * 7) % 9);
        let right_len = 18 + ((y * 11) % 9);
        for x in 9..9 + left_len {
            set(x, y, [224, 122, 122, 255]);
        }
        for x in 55 - right_len..55 {
            set(x, y, [100, 197, 140, 255]);
        }
    }
    // Central H.
    for y in 20..44 {
        set(28, y, [235, 240, 248, 255]);
        set(34, y, [235, 240, 248, 255]);
    }
    for x in 28..35 {
        set(x, 31, [235, 240, 248, 255]);
        set(x, 32, [235, 240, 248, 255]);
    }
    egui::IconData {
        rgba,
        width: size as u32,
        height: size as u32,
    }
}

#[cfg(target_os = "windows")]
fn hide_own_console() {
    const SW_HIDE: i32 = 0;
    unsafe extern "system" {
        fn GetConsoleWindow() -> isize;
        fn GetConsoleProcessList(process_list: *mut u32, count: u32) -> u32;
        fn ShowWindow(window: isize, command: i32) -> i32;
    }
    let mut processes = [0u32; 2];
    let count = unsafe { GetConsoleProcessList(processes.as_mut_ptr(), processes.len() as u32) };
    // A console shared with the launching shell must stay visible. When this
    // process owns a freshly allocated console, hide it for a clean GUI launch.
    if count <= 1 {
        unsafe {
            let _ = ShowWindow(GetConsoleWindow(), SW_HIDE);
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn hide_own_console() {}
