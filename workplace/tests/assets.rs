// Static asset integrity: the service worker shell list, manifest icons and
// index references must all point at files that actually ship.

use std::path::Path;

#[test]
fn service_worker_shell_paths_exist() {
    let source = std::fs::read_to_string("web/sw.js").expect("read sw.js");
    let start = source.find("const SHELL = [").expect("SHELL array");
    let end = source[start..].find("];").expect("SHELL terminator") + start;
    let block = &source[start..end];

    let mut checked = 0;
    let mut rest = block;
    while let Some(open) = rest.find('\'') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('\'') else { break };
        let path = &after[..close];
        rest = &after[close + 1..];
        if !path.starts_with('/') {
            continue;
        }
        let relative = path.trim_start_matches('/');
        let file = if relative.is_empty() {
            "web/index.html".to_string()
        } else {
            format!("web/{relative}")
        };
        assert!(
            Path::new(&file).is_file(),
            "service worker lists {path} but {file} does not exist"
        );
        checked += 1;
    }
    assert!(
        checked >= 20,
        "expected the full app shell, checked {checked}"
    );
}

#[test]
fn manifest_icons_and_entry_fields_exist() {
    let manifest = std::fs::read_to_string("web/manifest.webmanifest").expect("manifest");
    let value: serde_json::Value = serde_json::from_str(&manifest).expect("valid manifest JSON");
    assert_eq!(value["name"].as_str(), Some("Daybook — employee earnings"));
    assert_eq!(value["start_url"].as_str(), Some("/#/today"));

    let icons = value["icons"].as_array().expect("icons");
    assert!(icons.len() >= 3);
    for icon in icons {
        let src = icon["src"].as_str().expect("icon src");
        let relative = src.trim_start_matches('/');
        assert!(
            Path::new("web").join(relative).is_file(),
            "manifest icon {src} missing"
        );
        assert!(icon["sizes"].as_str().unwrap_or_default().contains('x'));
        assert_eq!(icon["type"].as_str(), Some("image/png"));
    }
    assert!(
        icons.iter().any(|icon| icon["purpose"] == "maskable"),
        "a maskable icon is required for Android install quality"
    );
}

#[test]
fn index_references_only_shipped_assets() {
    let index = std::fs::read_to_string("web/index.html").expect("index.html");
    for needle in [
        "/css/app.css",
        "/manifest.webmanifest",
        "/js/app.js",
        "/icons/apple-touch-icon.png",
        "/favicon.png",
    ] {
        assert!(index.contains(needle), "index.html is missing {needle}");
        let relative = needle.trim_start_matches('/');
        assert!(
            Path::new("web").join(relative).is_file(),
            "index references missing asset {needle}"
        );
    }
    assert!(index.contains("<meta name=\"theme-color\""));
    assert!(index.contains("apple-mobile-web-app-capable"));
}

#[test]
fn every_view_module_is_referenced_by_the_app() {
    let app = std::fs::read_to_string("web/js/app.js").expect("app.js");
    for module in ["today", "employees", "reports", "sync", "login"] {
        assert!(
            app.contains(&format!("./views/{module}.js")),
            "app.js does not import views/{module}.js"
        );
    }
}
