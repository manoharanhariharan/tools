use std::path::Path;

use axum::body::Body;
use axum::extract::State;
use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

use crate::AppState;

#[derive(RustEmbed)]
#[folder = "web/"]
struct Assets;

/// Serves the embedded PWA. `DAYBOOK_WEB_DIR` switches to live files for
/// development without rebuilding the binary.
pub async fn static_handler(State(state): State<AppState>, uri: Uri) -> Response {
    let raw_path = uri.path().trim_start_matches('/');
    let relative = if raw_path.is_empty() || raw_path.ends_with('/') {
        "index.html"
    } else {
        raw_path
    };

    let Some(bytes) = read_asset(&state, relative) else {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    };

    let content_type = content_type_for(relative);
    let cache = if relative == "index.html" || relative == "sw.js" || relative.ends_with(".html") {
        "no-cache"
    } else if relative.starts_with("icons/") {
        "public, max-age=86400"
    } else {
        "no-cache"
    };

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, cache)
        .body(Body::from(bytes))
        .unwrap_or_else(|_| (StatusCode::INTERNAL_SERVER_ERROR, "asset error").into_response())
}

fn read_asset(state: &AppState, relative: &str) -> Option<Vec<u8>> {
    if relative.contains("..") || relative.starts_with('/') || relative.contains('\\') {
        return None;
    }
    if let Some(dir) = &state.config.web_dir {
        return read_from_dir(dir, relative);
    }
    Assets::get(relative).map(|file| file.data.into_owned())
}

fn read_from_dir(dir: &Path, relative: &str) -> Option<Vec<u8>> {
    let base = dir.canonicalize().ok()?;
    let candidate = dir.join(relative).canonicalize().ok()?;
    if !candidate.starts_with(&base) {
        return None;
    }
    std::fs::read(candidate).ok()
}

fn content_type_for(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or_default() {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "json" => "application/json",
        "webmanifest" => "application/manifest+json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "txt" => "text/plain; charset=utf-8",
        _ => mime_guess::from_path(path)
            .first_raw()
            .unwrap_or("application/octet-stream"),
    }
}
