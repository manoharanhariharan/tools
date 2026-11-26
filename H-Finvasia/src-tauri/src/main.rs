// Console window only in debug builds; the release app is windowless.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! H-Finvasia desktop shell.
//!
//! Thin adapter over [`fv_bridge::MarketData`]: owns no market logic itself, only
//! the Tauri plumbing (commands, events, task lifecycle).
//!
//! Threading model ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬ÃƒÂ¢Ã¢â€šÂ¬Ã‚Â deliberately simple and leak-free:
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
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use tauri::async_runtime::JoinHandle;

mod account;
mod groups;
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
    /// Commands to the running live session (subscribe/unsubscribe/shutdown).
    feed_cmd: Mutex<Option<mpsc::UnboundedSender<fv_feed::ws::FeedCommand>>>,
    /// Cached broker session, so read-only screens do not re-validate every call.
    session: Mutex<Option<session::Session>>,
    rest: account::RestClient,
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
    forward_plan_to_live(&state, &plan);
    save_watch(&app, state.inner());
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
    forward_plan_to_live(&state, &plan);
    save_watch(&app, state.inner());
    let _ = app.emit("watch:changed", payload);
    Ok(plan)
}

/// Push a watch-list change to the open live socket, so a newly added
/// instrument starts ticking immediately instead of waiting for a reconnect.
///
/// No-op when no live session is running (replay/idle).
fn forward_plan_to_live(state: &Arc<AppState>, plan: &PlanDto) {
    let Some(sender) = state.feed_cmd.lock().unwrap().clone() else {
        return;
    };

    let to_keys = |wire: &[String]| -> Vec<SymbolKey> {
        wire.iter()
            .filter_map(|k| SymbolKey::parse(k).ok())
            .collect()
    };

    if !plan.subscribe.is_empty() {
        let _ = sender.send(fv_feed::ws::FeedCommand::Subscribe(to_keys(
            &plan.subscribe,
        )));
    }
    if !plan.unsubscribe.is_empty() {
        let _ = sender.send(fv_feed::ws::FeedCommand::Unsubscribe(to_keys(
            &plan.unsubscribe,
        )));
    }
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
    if let Some(cmd) = state.feed_cmd.lock().unwrap().take() {
        let _ = cmd.send(fv_feed::ws::FeedCommand::Shutdown);
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

// ---------------------------------------------------------------------------
// Option chain
// ---------------------------------------------------------------------------

#[tauri::command]
fn chain_expiries(state: State<'_, Arc<AppState>>, underlying: String) -> Vec<String> {
    state
        .market
        .lock()
        .unwrap()
        .master()
        .map(|master| master.expiries(&underlying))
        .unwrap_or_default()
}

/// Load an option chain: subscribe the legs (separate from the watch list) and
/// return metadata plus any quotes already known.
#[tauri::command]
fn chain_load(
    state: State<'_, Arc<AppState>>,
    underlying: String,
    expiry: String,
    strikes: Option<usize>,
    atm: Option<String>,
) -> Result<Vec<fv_bridge::ChainRowDto>, String> {
    let (plan, rows) = {
        let mut market = state.market.lock().unwrap();

        let legs: Vec<fv_master::Instrument> = match market.master() {
            Some(master) => master
                .option_chain(&underlying, &expiry)
                .into_iter()
                .cloned()
                .collect(),
            None => return Err("instrument master is still loading".into()),
        };
        if legs.is_empty() {
            return Err(format!("no contracts for {underlying} {expiry}"));
        }

        // Subscribe only a narrow window around the money. The full chain is
        // still returned as metadata so the grid can be browsed without asking
        // the venue for hundreds of instruments.
        let centre = atm.and_then(|value| fv_core::Price::parse(value.trim()).ok());
        let window = fv_bridge::chain_window(&legs, strikes.unwrap_or(10).clamp(1, 20), centre);
        let plan = market.set_chain(window);
        let rows: Vec<fv_bridge::ChainRowDto> = legs.iter().map(|i| market.chain_row(i)).collect();
        (plan, rows)
    };

    forward_plan_to_live(&state, &plan);
    tracing::info!(
        underlying = %underlying,
        expiry = %expiry,
        legs = rows.len(),
        new_subs = plan.subscribe.len(),
        "option chain loaded"
    );
    Ok(rows)
}

#[tauri::command]
fn chain_close(app: AppHandle, state: State<'_, Arc<AppState>>) {
    let plan = { state.market.lock().unwrap().clear_chain() };
    forward_plan_to_live(&state, &plan);
    let _ = app.emit("chain:closed", ());
}

// ---------------------------------------------------------------------------
// Read-only account views (positions / funds / order book)
// ---------------------------------------------------------------------------

fn looks_like_session_error(message: &str) -> bool {
    message.contains("SESSION")
        || message.contains("Session Expired")
        || message.contains("Invalid Session")
}

async fn ensure_session(state: &Arc<AppState>) -> Result<session::Session, String> {
    if let Some(current) = state.session.lock().unwrap().clone() {
        return Ok(current);
    }
    let resolved = session::resolve_session().await?;
    *state.session.lock().unwrap() = Some(resolved.clone());
    Ok(resolved)
}

#[tauri::command]
async fn positions_load(state: State<'_, Arc<AppState>>) -> Result<account::PositionsView, String> {
    let shared = state.inner().clone();
    let mut session = ensure_session(&shared).await?;
    match account::positions(&shared.rest, &session.uid, &session.actid, &session.token).await {
        Ok(view) => Ok(view),
        Err(err) if looks_like_session_error(&err) => {
            // The cached token aged out: drop it and resolve once more.
            *shared.session.lock().unwrap() = None;
            session = ensure_session(&shared).await?;
            account::positions(&shared.rest, &session.uid, &session.actid, &session.token).await
        }
        Err(err) => Err(err),
    }
}

#[tauri::command]
async fn orders_load(state: State<'_, Arc<AppState>>) -> Result<Vec<account::OrderDto>, String> {
    let shared = state.inner().clone();
    let mut session = ensure_session(&shared).await?;
    match account::orders(&shared.rest, &session.uid, &session.token).await {
        Ok(rows) => Ok(rows),
        Err(err) if looks_like_session_error(&err) => {
            *shared.session.lock().unwrap() = None;
            session = ensure_session(&shared).await?;
            account::orders(&shared.rest, &session.uid, &session.token).await
        }
        Err(err) => Err(err),
    }
}

// ---------------------------------------------------------------------------
// Watch groups (local, broker-free)
// ---------------------------------------------------------------------------

fn config_dir(app: &AppHandle) -> std::path::PathBuf {
    app.path()
        .app_config_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
}

fn groups_file(app: &AppHandle) -> std::path::PathBuf {
    groups::file_path(&config_dir(app))
}

fn watch_file(app: &AppHandle) -> std::path::PathBuf {
    groups::watch_path(&config_dir(app))
}

/// Persist the current watch list so the next launch can resume it.
fn save_watch(app: &AppHandle, state: &Arc<AppState>) {
    let keys = state.market.lock().unwrap().watched();
    if let Err(err) = groups::write_watch(&watch_file(app), keys) {
        tracing::warn!(error = %err, "could not persist the watch list");
    }
}

#[tauri::command]
fn groups_list(app: AppHandle) -> Vec<groups::GroupSummary> {
    groups::list(&groups_file(&app))
}

#[tauri::command]
fn group_save(app: AppHandle, state: State<'_, Arc<AppState>>, name: String) -> Result<(), String> {
    let keys = state.market.lock().unwrap().watched();
    if keys.is_empty() {
        return Err("the watch list is empty, nothing to save".into());
    }
    groups::save(&groups_file(&app), &name, keys)
}

#[tauri::command]
fn group_load(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    name: String,
) -> Result<PlanDto, String> {
    let stored = groups::load(&groups_file(&app), &name)?;
    let parsed: Vec<SymbolKey> = stored
        .iter()
        .filter_map(|key| SymbolKey::parse(key).ok())
        .collect();
    if parsed.is_empty() {
        return Err(format!("group \"{name}\" has no valid instruments"));
    }

    let (plan, payload) = {
        let mut market = state.market.lock().unwrap();
        let plan = market.set_watch(parsed);
        let payload = WatchChanged {
            keys: market.watched(),
            quotes: market.watch_quotes(),
        };
        (plan, payload)
    };
    forward_plan_to_live(&state, &plan);
    save_watch(&app, state.inner());
    let _ = app.emit("watch:changed", payload);
    Ok(plan)
}

#[tauri::command]
fn group_delete(app: AppHandle, name: String) -> Result<(), String> {
    groups::delete(&groups_file(&app), &name)
}

// ---------------------------------------------------------------------------
// Index watch
// ---------------------------------------------------------------------------

#[tauri::command]
fn indices_list(state: State<'_, Arc<AppState>>) -> Vec<fv_bridge::IndexRowDto> {
    state.market.lock().unwrap().index_rows(300)
}

/// Connect to the venue. The session is resolved automatically:
/// env token ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã‚Â ÃƒÂ¢Ã¢â€šÂ¬Ã¢â€žÂ¢ persisted Mongo token (validated) ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã‚Â ÃƒÂ¢Ã¢â€šÂ¬Ã¢â€žÂ¢ TOTP login.
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
    begin_live(app, state.inner().clone(), resolved, keys).await
}

/// Start the socket for an already-resolved session. Shared by the manual
/// command and the startup auto-connect.
async fn begin_live(
    app: AppHandle,
    state: Arc<AppState>,
    resolved: session::Session,
    keys: Vec<SymbolKey>,
) -> Result<Vec<String>, String> {
    let cfg = fv_feed::ws::WsConfig::new(&resolved.uid, &resolved.actid, &resolved.token);
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<fv_feed::ws::FeedCommand>();
    *state.feed_cmd.lock().unwrap() = Some(cmd_tx);

    let tx = state.tx.clone();
    let feed_state = state.clone();
    let shared = state.clone();
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let result = fv_feed::ws::run_forever(
            cfg,
            keys,
            move |frame| {
                let _ = tx.send(frame);
            },
            move |event| apply_feed_state(&feed_state, event),
            cmd_rx,
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
        feed_cmd: Mutex::new(None),
        session: Mutex::new(None),
        rest: account::RestClient::new().expect("rest client"),
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

            let demo_handle = handle.clone();
            tauri::async_runtime::spawn(load_master(setup_state.clone(), handle.clone()));

            let demo_mode = std::env::var("FV_AUTOSTART_DEMO")
                .map(|v| v == "1")
                .unwrap_or(false);

            // Resume the previous watch list and, if a session is already valid,
            // connect without any manual step.
            if !demo_mode {
                let resume_state = setup_state.clone();
                let resume_handle = handle.clone();
                tauri::async_runtime::spawn(async move {
                    let stored = groups::read_watch(&watch_file(&resume_handle));
                    let keys: Vec<SymbolKey> = stored
                        .iter()
                        .filter_map(|key| SymbolKey::parse(key).ok())
                        .collect();
                    if keys.is_empty() {
                        return;
                    }

                    let payload = {
                        let mut market = resume_state.market.lock().unwrap();
                        market.set_watch(keys.clone());
                        WatchChanged {
                            keys: market.watched(),
                            quotes: market.watch_quotes(),
                        }
                    };
                    let _ = resume_handle.emit("watch:changed", payload);

                    // Only adopt an existing session; never log in silently at
                    // startup, because that could invalidate another tool.
                    if let Some(resolved) = session::resolve_existing().await {
                        tracing::info!(
                            keys = keys.len(),
                            "auto-connecting with an existing session"
                        );
                        if let Err(err) =
                            begin_live(resume_handle.clone(), resume_state.clone(), resolved, keys)
                                .await
                        {
                            tracing::warn!(error = %err, "auto-connect failed");
                        }
                    }
                });
            }

            // Kiosk/demo/CI switch: start the bundled replay without a click.
            if demo_mode {
                let autostart_state = setup_state.clone();
                tauri::async_runtime::spawn(async move {
                    // Let the webview mount its listeners. The frontend also
                    // hydrates from watch_list, so a missed event is harmless.
                    tokio::time::sleep(Duration::from_millis(1_500)).await;
                    if let Err(err) = start_demo(demo_handle, autostart_state).await {
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
            chain_expiries,
            chain_load,
            chain_close,
            positions_load,
            orders_load,
            groups_list,
            group_save,
            group_load,
            group_delete,
            indices_list,
            stop_feed,
        ])
        .run(tauri::generate_context!())
        .expect("error while running H-Finvasia");
}
