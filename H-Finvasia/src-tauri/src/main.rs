// Console window only in debug builds; the release app is windowless.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! H-Finvasia desktop shell.
//!
//! Thin adapter over [`fv_bridge::MarketData`]: owns no market logic itself, only
//! the Tauri plumbing (commands, events, task lifecycle).
//!
//! Threading model — deliberately simple and leak-free:
//! * one producer task feeds decoded [`Frame`]s into an mpsc channel;
//! * one consumer task owns the state lock, absorbs frames, and emits bounded
//!   `quotes:delta` batches at ~60 Hz;
//! * commands take the same lock briefly and never await while holding it.

use fv_bridge::{InstrumentDto, MarketData, PlanDto, QuoteDto, SourceKind, StatusDto};
use fv_core::SymbolKey;
use fv_feed::Frame;
use serde::Serialize;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use tauri::async_runtime::JoinHandle;

mod session;

/// Bundled master used when MongoDB is unavailable, so first-run search works.
const SAMPLE_MASTER: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../crates/fv-master/tests/fixtures/instruments.json"
));

/// Bundled feed capture used by "Start demo feed".
const SAMPLE_FEED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../crates/fv-feed/tests/fixtures/sample_feed.jsonl"
));

pub struct AppState {
    market: Mutex<MarketData>,
    tx: UnboundedSender<Frame>,
    producer: Mutex<Option<JoinHandle<()>>>,
    live_stop: Mutex<Option<mpsc::Sender<()>>>,
}

#[derive(Debug, Clone, Serialize)]
struct WatchChanged {
    keys: Vec<String>,
    quotes: Vec<QuoteDto>,
}

#[derive(Debug, Clone, Serialize)]
struct AppInfo {
    version: String,
    backend: String,
    has_token: bool,
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[tauri::command]
fn app_info(state: State<'_, Arc<AppState>>) -> AppInfo {
    let status = state.market.lock().unwrap().status(Instant::now());
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        backend: status.backend,
        has_token: std::env::var("FV_ACCESS_TOKEN")
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false),
    }
}

#[tauri::command]
fn status(state: State<'_, Arc<AppState>>) -> StatusDto {
    state.market.lock().unwrap().status(Instant::now())
}

#[tauri::command]
fn search(
    state: State<'_, Arc<AppState>>,
    query: String,
    limit: Option<usize>,
) -> Vec<InstrumentDto> {
    let limit = limit.unwrap_or(25).clamp(1, 500);
    state.market.lock().unwrap().search(&query, limit)
}

#[tauri::command]
fn instrument(state: State<'_, Arc<AppState>>, key: String) -> Option<InstrumentDto> {
    state.market.lock().unwrap().instrument(&key)
}

#[tauri::command]
fn watch_list(state: State<'_, Arc<AppState>>) -> Vec<String> {
    state.market.lock().unwrap().watched()
}

#[tauri::command]
fn watch_quotes(state: State<'_, Arc<AppState>>) -> Vec<QuoteDto> {
    state.market.lock().unwrap().watch_quotes()
}

#[tauri::command]
fn snapshot(state: State<'_, Arc<AppState>>, key: String) -> Option<QuoteDto> {
    state.market.lock().unwrap().snapshot(&key)
}

#[tauri::command]
fn watch_add(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    key: String,
) -> Result<PlanDto, String> {
    let parsed = SymbolKey::parse(&key).map_err(|e| e.to_string())?;
    let (plan, payload) = {
        let mut market = state.market.lock().unwrap();
        let plan = market.add_to_watch(parsed);
        let payload = WatchChanged {
            keys: market.watched(),
            quotes: market.watch_quotes(),
        };
        (plan, payload)
    };
    let _ = app.emit("watch:changed", payload);
    Ok(plan)
}

#[tauri::command]
fn watch_remove(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    key: String,
) -> Result<PlanDto, String> {
    let (plan, payload) = {
        let mut market = state.market.lock().unwrap();
        let plan = market.remove_from_watch(&key);
        let payload = WatchChanged {
            keys: market.watched(),
            quotes: market.watch_quotes(),
        };
        (plan, payload)
    };
    let _ = app.emit("watch:changed", payload);
    Ok(plan)
}

#[tauri::command]
fn stop_feed(app: AppHandle, state: State<'_, Arc<AppState>>) {
    stop_feed_impl(state.inner());
    let _ = app.emit("feed:changed", "idle");
}

/// Shared teardown used by `stop_feed` and by the start commands.
fn stop_feed_impl(state: &Arc<AppState>) {
    if let Some(handle) = state.producer.lock().unwrap().take() {
        handle.abort();
    }
    if let Some(stop) = state.live_stop.lock().unwrap().take() {
        let _ = stop.try_send(());
    }
    let mut market = state.market.lock().unwrap();
    market.set_source(SourceKind::Idle);
    market.clear_error();
}

/// Start the bundled capture (no `path` = the demo feed), or a provided JSONL capture.
#[tauri::command]
async fn start_replay(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    path: Option<String>,
    hz: Option<u64>,
) -> Result<Vec<String>, String> {
    match path.as_deref().filter(|p| !p.trim().is_empty()) {
        Some(file) => start_capture(app, state.inner().clone(), file, hz.unwrap_or(120)).await,
        None => start_demo(app, state.inner().clone()).await,
    }
}

/// Used by the command and by `FV_AUTOSTART_DEMO=1` (kiosk/demo/CI smoke test).
async fn start_demo(app: AppHandle, state: Arc<AppState>) -> Result<Vec<String>, String> {
    begin_feed(
        app,
        state,
        fv_feed::Replay::from_jsonl_str(SAMPLE_FEED),
        120,
    )
    .await
}

async fn start_capture(
    app: AppHandle,
    state: Arc<AppState>,
    path: &str,
    hz: u64,
) -> Result<Vec<String>, String> {
    let replay =
        fv_feed::Replay::from_path(path).map_err(|e| format!("cannot read capture: {e}"))?;
    begin_feed(app, state, replay, hz).await
}

async fn begin_feed(
    app: AppHandle,
    state: Arc<AppState>,
    replay: fv_feed::Replay,
    hz: u64,
) -> Result<Vec<String>, String> {
    stop_feed_impl(&state);

    if replay.is_empty() {
        return Err("capture contains no frames".into());
    }

    // Pre-scan so the watch list reflects what the capture actually contains.
    let mut keys: Vec<SymbolKey> = Vec::new();
    for raw in replay.frames() {
        if let Ok(fv_feed::Frame::Snapshot(t) | fv_feed::Frame::Diff(t)) = fv_feed::parse(raw) {
            if let Some(key) = t.symbol() {
                if !keys.contains(&key) {
                    keys.push(key);
                }
            }
        }
    }
    keys.sort();

    let payload = {
        let mut market = state.market.lock().unwrap();
        market.set_watch(keys);
        market.set_source(SourceKind::Replay);
        market.clear_error();
        WatchChanged {
            keys: market.watched(),
            quotes: market.watch_quotes(),
        }
    };

    let frames: Vec<Vec<u8>> = replay.frames().to_vec();
    let handle = spawn_replay(state.clone(), frames, hz);
    *state.producer.lock().unwrap() = Some(handle);

    let watched = payload.keys.clone();
    let _ = app.emit("watch:changed", payload);
    let _ = app.emit("feed:changed", "replay");
    tracing::info!(keys = watched.len(), hz, "replay feed started");

    Ok(watched)
}

#[tauri::command]
async fn session_info() -> session::SessionInfo {
    session::peek().await
}

/// Connect to the venue. The session is resolved automatically:
/// env token → persisted Mongo token (validated) → TOTP login.
#[tauri::command]
async fn start_live(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<String>, String> {
    stop_feed_impl(state.inner());

    let keys: Vec<SymbolKey> = {
        let market = state.market.lock().unwrap();
        market
            .watched()
            .iter()
            .filter_map(|k| SymbolKey::parse(k).ok())
            .collect()
    };
    if keys.is_empty() {
        return Err("add at least one instrument to the watch list first".into());
    }

    let resolved = session::resolve_session().await?;
    tracing::info!(
        uid = %resolved.uid,
        actid = %resolved.actid,
        token_source = resolved.source.as_str(),
        "live session resolved"
    );

    let cfg = fv_feed::ws::WsConfig::new(&resolved.uid, &resolved.actid, &resolved.token);
    let (stop_tx, stop_rx) = mpsc::channel::<()>(1);
    *state.live_stop.lock().unwrap() = Some(stop_tx);

    let tx = state.tx.clone();
    let feed_state = state.inner().clone();
    let shared = state.inner().clone();
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let result = fv_feed::ws::run_forever(
            cfg,
            keys,
            move |frame| {
                let _ = tx.send(frame);
            },
            move |event| apply_feed_state(&feed_state, event),
            stop_rx,
        )
        .await;

        let mut market = shared.market.lock().unwrap();
        match result {
            Ok(()) => market.set_source(SourceKind::Idle),
            Err(err) => {
                market.record_error(err.to_string());
                market.set_connected(false);
            }
        }
        drop(market);
        let _ = handle.emit("feed:changed", "idle");
    });

    let watched = {
        let mut market = state.market.lock().unwrap();
        market.set_source(SourceKind::Live);
        // Real connectivity is reported by the feed task via FeedState.
        market.set_connected(false);
        market.clear_error();
        market.watched()
    };
    let _ = app.emit("feed:changed", "live");
    Ok(watched)
}

/// Translate transport lifecycle events into UI state.
fn apply_feed_state(state: &Arc<AppState>, event: fv_feed::ws::FeedState) {
    use fv_feed::ws::FeedState;

    let mut market = state.market.lock().unwrap();
    match event {
        FeedState::Connected { source } => {
            market.set_source(SourceKind::Live);
            market.set_connected(true);
            market.clear_error();
            tracing::info!(%source, "live feed connected");
        }
        FeedState::Rejected { source } => {
            market.set_connected(false);
            market.set_error(format!("session rejected for source '{source}'"));
        }
        FeedState::Error { message } => {
            market.set_connected(false);
            market.set_error(message);
        }
        FeedState::Reconnecting { attempt } => {
            market.set_connected(false);
            tracing::info!(attempt, "feed reconnecting");
        }
    }
}

// ---------------------------------------------------------------------------
// Background tasks
// ---------------------------------------------------------------------------

/// Pace a capture at (approximately) `hz` frames/second.
///
/// A per-frame `sleep(1/hz)` is wrong on Windows: the timer granularity is
/// ~15.6 ms, so anything faster than ~64 Hz silently runs at 64 Hz. That would
/// replay a real high-rate capture far too slowly. Instead, sleep once per tick,
/// measure the *actual* elapsed time, and replenish a frame budget from it, so
/// the average rate self-corrects regardless of timer resolution.
fn spawn_replay(state: Arc<AppState>, frames: Vec<Vec<u8>>, hz: u64) -> JoinHandle<()> {
    let hz = hz.clamp(1, 50_000);
    tauri::async_runtime::spawn(async move {
        const TICK: Duration = Duration::from_millis(5);
        let mut budget = 0.0f64;
        let mut index = 0usize;
        let mut last = Instant::now();

        loop {
            tokio::time::sleep(TICK).await;
            let now = Instant::now();
            let elapsed = now.saturating_duration_since(last).as_secs_f64();
            last = now;

            budget += elapsed * hz as f64;
            // Cap catch-up after a stall so we never emit a huge burst.
            budget = budget.min(hz as f64);

            // Bounded so a pathological hz cannot spin this loop.
            let mut sent = 0u32;
            while budget >= 1.0 && sent < hz as u32 {
                let raw = &frames[index % frames.len()];
                match fv_feed::parse(raw) {
                    Ok(frame) => {
                        if state.tx.send(frame).is_err() {
                            return; // consumer gone: shut down
                        }
                    }
                    Err(err) => {
                        state.market.lock().unwrap().record_error(err.to_string());
                    }
                }
                index = index.wrapping_add(1);
                budget -= 1.0;
                sent += 1;
            }
        }
    })
}

async fn consume(state: Arc<AppState>, mut rx: UnboundedReceiver<Frame>, app: AppHandle) {
    let mut ticker = tokio::time::interval(Duration::from_millis(16));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut last_status = Instant::now();

    loop {
        tokio::select! {
            incoming = rx.recv() => {
                match incoming {
                    Some(frame) => {
                        let ts = now_ms();
                        state.market.lock().unwrap().apply(&frame, ts);
                    }
                    None => return,
                }
            }
            _ = ticker.tick() => {
                let now = Instant::now();
                let (delta, status) = {
                    let mut market = state.market.lock().unwrap();
                    let delta = market.flush(now, now_ms());
                    let status = if last_status.elapsed() >= Duration::from_secs(1) {
                        last_status = Instant::now();
                        Some(market.status(now))
                    } else {
                        None
                    };
                    (delta, status)
                };
                if let Some(delta) = delta {
                    let _ = app.emit("quotes:delta", delta);
                }
                if let Some(status) = status {
                    tracing::debug!(
                        frames = status.frames_total,
                        rate = status.frames_per_sec,
                        watched = status.watched,
                        "status"
                    );
                    let _ = app.emit("status:changed", status);
                }
            }
        }
    }
}

/// Load the instrument master in the background: Mongo first, bundled sample as
/// the degraded fallback. The window is usable immediately either way.
async fn load_master(state: Arc<AppState>, app: AppHandle) {
    let cfg = fv_master::mongo::MasterConfig::from_env();
    tracing::info!(uri = %cfg.uri, db = %cfg.database, collection = %cfg.collection, "loading instrument master");

    match fv_master::mongo::load_master(&cfg).await {
        Ok(master) => {
            let mut market = state.market.lock().unwrap();
            market.set_master(Arc::new(master));
            market.set_backend("mongo");
            let instruments = market.status(Instant::now()).instruments;
            tracing::info!(instruments, "master ready");
        }
        Err(err) => {
            tracing::warn!(error = %err, "mongo master unavailable; using bundled sample");
            match fv_master::index::load_from_json(SAMPLE_MASTER) {
                Ok(master) => {
                    let mut market = state.market.lock().unwrap();
                    market.set_master(Arc::new(master));
                    market.set_backend("in-memory");
                    market.set_error(format!("instrument master offline: {err}"));
                }
                Err(sample_err) => {
                    let mut market = state.market.lock().unwrap();
                    market.set_error(format!("no instrument master: {sample_err}"));
                }
            }
        }
    }

    let status = state.market.lock().unwrap().status(Instant::now());
    let _ = app.emit("status:changed", status);
    let _ = app.emit("master:ready", ());
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    let (tx, rx) = mpsc::unbounded_channel();
    let state = Arc::new(AppState {
        market: Mutex::new(MarketData::new(env!("CARGO_PKG_VERSION"))),
        tx,
        producer: Mutex::new(None),
        live_stop: Mutex::new(None),
    });

    let rx_slot = Arc::new(Mutex::new(Some(rx)));
    let setup_state = state.clone();

    tauri::Builder::default()
        .manage(state)
        .setup(move |app| {
            let handle = app.handle().clone();
            let rx = rx_slot
                .lock()
                .unwrap()
                .take()
                .expect("consumer channel already taken");

            tauri::async_runtime::spawn(consume(setup_state.clone(), rx, handle.clone()));

            let autostart_handle = handle.clone();
            tauri::async_runtime::spawn(load_master(setup_state.clone(), handle));

            // Kiosk/demo/CI switch: start the bundled replay without a click.
            if std::env::var("FV_AUTOSTART_DEMO")
                .map(|v| v == "1")
                .unwrap_or(false)
            {
                let autostart_state = setup_state.clone();
                tauri::async_runtime::spawn(async move {
                    // Let the webview mount its listeners. The frontend also
                    // hydrates from watch_list, so a missed event is harmless.
                    tokio::time::sleep(Duration::from_millis(1_500)).await;
                    if let Err(err) = start_demo(autostart_handle, autostart_state).await {
                        tracing::warn!(error = %err, "autostart demo failed");
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            status,
            search,
            instrument,
            watch_list,
            watch_quotes,
            snapshot,
            watch_add,
            watch_remove,
            start_replay,
            start_live,
            session_info,
            stop_feed,
        ])
        .run(tauri::generate_context!())
        .expect("error while running H-Finvasia");
}
