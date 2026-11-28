// Console window only in debug builds; the release app is windowless.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! H-Finvasia desktop shell.
//!
//! Thin adapter over [`fv_bridge::MarketData`]: owns no market logic itself, only
//! the Tauri plumbing (commands, events, task lifecycle).
//!
//! Threading model ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€¦Ã‚Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â deliberately simple and leak-free:
//! * one producer task feeds decoded [`Frame`]s into an mpsc channel;
//! * one consumer task owns the state lock, absorbs frames, and emits bounded
//!   `quotes:delta` batches at ~60 Hz;
//! * commands take the same lock briefly and never await while holding it.

use fv_bridge::{InstrumentDto, MarketData, PlanDto, QuoteDto, SourceKind, StatusDto};
use fv_core::{Price, SymbolKey};
use fv_feed::Frame;
use serde::Serialize;
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use tauri::async_runtime::JoinHandle;

mod account;
mod broker_log;
mod groups;
mod kite;
mod orders;
mod session;

/// Bundled master used when MongoDB is unavailable, so first-run search works.
const SAMPLE_MASTER: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../crates/fv-master/tests/fixtures/instruments.json"
));

/// NIFTY 50 index key (NSE token 26000), used when the user has no instruments.
const DEFAULT_WATCH_KEY: &str = "NSE|26000";

/// Bundled feed capture used by `FV_AUTOSTART_DEMO=1` and the headless CLI.
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
    /// Trailing stop-loss orders watched by the background modifier.
    trailing: Mutex<Vec<TrailingStop>>,
    /// Set when the exchange rejects the WebSocket handshake. The next explicit
    /// Connect live forces a fresh TOTP login instead of reusing the token.
    feed_auth_failed: AtomicBool,
}

#[derive(Debug, Clone)]
struct TrailingStop {
    order_no: String,
    key: String,
    exchange: String,
    trading_symbol: String,
    side: String,
    qty: i64,
    trail: Price,
    trigger: Price,
    limit: Price,
}

#[derive(Debug, Clone, Serialize)]
struct WatchChanged {
    keys: Vec<String>,
    quotes: Vec<QuoteDto>,
}

fn watch_snapshot(market: &MarketData) -> WatchChanged {
    WatchChanged {
        keys: market
            .market_watch_keys()
            .iter()
            .map(|key| key.as_wire())
            .collect(),
        quotes: market.market_watch_quotes(),
    }
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

/// Screenshot / release-probe mode. Every broker-backed path must check this so
/// a render can never place an order or read an account.
fn offline_mode() -> bool {
    std::env::var("FV_NO_AUTOCONNECT").as_deref() == Ok("1")
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
    state
        .market
        .lock()
        .unwrap()
        .market_watch_keys()
        .iter()
        .map(|key| key.as_wire())
        .collect()
}

#[tauri::command]
fn watch_quotes(state: State<'_, Arc<AppState>>) -> Vec<QuoteDto> {
    state.market.lock().unwrap().market_watch_quotes()
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
        let payload = watch_snapshot(&market);
        (plan, payload)
    };
    forward_plan_to_live(&state, &plan);
    save_watch(&app, state.inner());
    persist_active_container(&app, state.inner());
    sync_group_subscriptions(&app, state.inner());
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
        let payload = watch_snapshot(&market);
        (plan, payload)
    };
    forward_plan_to_live(&state, &plan);
    save_watch(&app, state.inner());
    persist_active_container(&app, state.inner());
    sync_group_subscriptions(&app, state.inner());
    if !ensure_default_instrument(&app, state.inner()) {
        let _ = app.emit("watch:changed", payload);
    }
    Ok(plan)
}

/// Remove an instrument from the active watch list and from every group.
/// Market Watch shows the union, so its x button must clear all owners.
#[tauri::command]
fn remove_everywhere(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    key: String,
) -> Result<PlanDto, String> {
    let plan = {
        let mut market = state.market.lock().unwrap();
        market.remove_from_watch(&key)
    };
    forward_plan_to_live(&state, &plan);
    groups::remove_key_everywhere(&groups_file(&app), &key)?;
    save_watch(&app, state.inner());
    sync_group_subscriptions(&app, state.inner());
    ensure_default_instrument(&app, state.inner());
    let payload = {
        let market = state.market.lock().unwrap();
        watch_snapshot(&market)
    };
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
        watch_snapshot(&market)
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

/// Prepared Kite feed status. Read-only: this command never opens a broker
/// socket or returns the enctoken.
#[tauri::command]
async fn kite_bootstrap() -> kite::KiteBootstrapDto {
    kite::bootstrap().await
}

// ---------------------------------------------------------------------------
// Option chain
// ---------------------------------------------------------------------------

#[tauri::command]
fn chain_underlyings(
    state: State<'_, Arc<AppState>>,
    query: Option<String>,
    limit: Option<usize>,
) -> Vec<String> {
    state
        .market
        .lock()
        .unwrap()
        .chain_underlyings(query.as_deref().unwrap_or(""), limit.unwrap_or(40))
}

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
    chain_id: String,
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
        let plan = market.set_chain(&chain_id, window);
        let rows: Vec<fv_bridge::ChainRowDto> = legs
            .iter()
            .map(|instrument| {
                market.chain_row(
                    instrument,
                    market.chain_contains(&chain_id, &instrument.key),
                )
            })
            .collect();
        (plan, rows)
    };

    forward_plan_to_live(&state, &plan);
    tracing::info!(
        chain_id = %chain_id,
        underlying = %underlying,
        expiry = %expiry,
        legs = rows.len(),
        new_subs = plan.subscribe.len(),
        "option chain loaded"
    );
    Ok(rows)
}

#[tauri::command]
fn chain_close(app: AppHandle, state: State<'_, Arc<AppState>>, chain_id: String) {
    let plan = { state.market.lock().unwrap().clear_chain(&chain_id) };
    forward_plan_to_live(&state, &plan);
    let _ = app.emit("chain:closed", chain_id);
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
    if offline_mode() {
        return Ok(account::PositionsView {
            funds: None,
            positions: Vec::new(),
            totals: account::PositionTotals::default(),
        });
    }
    let shared = state.inner().clone();
    let mut session = ensure_session(&shared).await?;
    let result = match account::positions(
        &shared.rest,
        &session.uid,
        &session.actid,
        &session.token,
    )
    .await
    {
        Ok(view) => Ok(view),
        Err(err) if looks_like_session_error(&err) => {
            // The cached token aged out: drop it and resolve once more.
            *shared.session.lock().unwrap() = None;
            session = ensure_session(&shared).await?;
            account::positions(&shared.rest, &session.uid, &session.actid, &session.token).await
        }
        Err(err) => Err(err),
    };
    let view = result?;
    sync_position_keys(&shared, &view);
    Ok(view)
}

/// Subscribe the position-book instruments so the UI can recompute LTP/MTM/P&L
/// from the websocket instead of re-polling positions.
fn sync_position_keys(state: &Arc<AppState>, view: &account::PositionsView) {
    let keys: Vec<SymbolKey> = view
        .positions
        .iter()
        .filter_map(|position| {
            SymbolKey::parse(&format!("{}|{}", position.exchange, position.token)).ok()
        })
        .collect();
    let plan = { state.market.lock().unwrap().set_position_keys(keys) };
    forward_plan_to_live(state, &plan);
}

#[tauri::command]
async fn orders_load(state: State<'_, Arc<AppState>>) -> Result<Vec<account::OrderDto>, String> {
    if offline_mode() {
        return Ok(Vec::new());
    }
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

fn index_groups_file(app: &AppHandle) -> std::path::PathBuf {
    groups::index_file_path(&config_dir(app))
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

/// Mirror the live watch list into the active container. Called after explicit
/// add/remove actions; replay/demo hydration deliberately does not use it, so a
/// demo never overwrites the user's saved container.
fn persist_active_container(app: &AppHandle, state: &Arc<AppState>) {
    let keys = state.market.lock().unwrap().watched();
    let path = groups_file(app);
    let result =
        groups::ensure_default(&path, keys.clone()).and_then(|_| groups::save_active(&path, keys));
    if let Err(err) = result {
        tracing::warn!(error = %err, "could not persist the active container");
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
    let path = groups_file(&app);
    groups::save(&path, &name, keys)?;
    groups::set_active(&path, &name)?;
    sync_group_subscriptions(&app, state.inner());
    Ok(())
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
        let payload = watch_snapshot(&market);
        (plan, payload)
    };
    forward_plan_to_live(&state, &plan);
    save_watch(&app, state.inner());
    let _ = groups::set_active(&groups_file(&app), &name);
    let _ = app.emit("watch:changed", payload);
    Ok(plan)
}

#[tauri::command]
fn group_delete(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    name: String,
) -> Result<(), String> {
    let path = groups_file(&app);
    let (_, next) = groups::remove(&path, &name)?;
    if let Some(active) = next {
        let stored = groups::read(&path)
            .groups
            .get(&active)
            .cloned()
            .unwrap_or_default();
        apply_watch(&app, state.inner(), parse_watch_keys(&stored));
    }
    sync_group_subscriptions(&app, state.inner());
    Ok(())
}

// ---------------------------------------------------------------------------
// Containers (named watch lists used by the Stocks tab)
// ---------------------------------------------------------------------------

fn parse_watch_keys(stored: &[String]) -> Vec<SymbolKey> {
    stored
        .iter()
        .filter_map(|key| SymbolKey::parse(key).ok())
        .collect()
}

fn apply_watch(app: &AppHandle, state: &Arc<AppState>, keys: Vec<SymbolKey>) -> PlanDto {
    let (plan, payload) = {
        let mut market = state.market.lock().unwrap();
        let plan = market.set_watch(keys);
        let payload = watch_snapshot(&market);
        (plan, payload)
    };
    forward_plan_to_live(state, &plan);
    save_watch(app, state);
    let _ = app.emit("watch:changed", payload);
    plan
}

/// Every instrument in every group, de-duplicated. This is what the feed
/// subscribes to in addition to the active watch list, so all cards are live.
fn group_subscription_keys(app: &AppHandle) -> Vec<SymbolKey> {
    let file = groups::read(&groups_file(app));
    let mut keys = BTreeSet::new();
    for stored in file.groups.values() {
        for key in stored {
            if let Ok(parsed) = SymbolKey::parse(key) {
                keys.insert(parsed);
            }
        }
    }
    keys.into_iter().collect()
}

/// Reconcile the group subscription set with the live feed.
fn sync_group_subscriptions(app: &AppHandle, state: &Arc<AppState>) -> PlanDto {
    let keys = group_subscription_keys(app);
    let plan = {
        let mut market = state.market.lock().unwrap();
        market.set_group_keys(keys)
    };
    forward_plan_to_live(state, &plan);
    if plan.truncated {
        tracing::warn!(
            dropped = plan.rejected.len(),
            "group subscriptions exceeded the 100-instrument budget"
        );
    }
    plan
}

fn index_group_subscription_keys(app: &AppHandle) -> Vec<SymbolKey> {
    let file = groups::read(&index_groups_file(app));
    let mut keys = BTreeSet::new();
    for stored in file.groups.values() {
        for key in stored {
            if let Ok(parsed) = SymbolKey::parse(key) {
                keys.insert(parsed);
            }
        }
    }
    keys.into_iter().collect()
}

/// Reconcile the Index-tab card subscription set independently of the Stocks
/// groups, while sharing the same 100-instrument budget.
fn sync_index_subscriptions(app: &AppHandle, state: &Arc<AppState>) -> PlanDto {
    let keys = index_group_subscription_keys(app);
    let plan = {
        let mut market = state.market.lock().unwrap();
        market.set_index_group_keys(keys)
    };
    forward_plan_to_live(state, &plan);
    if plan.truncated {
        tracing::warn!(
            dropped = plan.rejected.len(),
            "index group subscriptions exceeded the 100-instrument budget"
        );
    }
    plan
}

/// If the user has no instruments anywhere, seed NIFTY 50 so the app can
/// connect and show a live row instead of sitting disabled.
fn ensure_default_instrument(app: &AppHandle, state: &Arc<AppState>) -> bool {
    let watched = state.market.lock().unwrap().watched();
    if !watched.is_empty() {
        return false;
    }
    if !group_subscription_keys(app).is_empty() {
        return false;
    }
    let Ok(key) = SymbolKey::parse(DEFAULT_WATCH_KEY) else {
        return false;
    };
    apply_watch(app, state, vec![key]);
    persist_active_container(app, state);
    sync_group_subscriptions(app, state);
    true
}

#[tauri::command]
fn containers_list(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<groups::ContainersView, String> {
    let keys = state.market.lock().unwrap().watched();
    let path = groups_file(&app);
    match groups::ensure_default(&path, keys) {
        Ok(view) => Ok(view),
        Err(err) => {
            tracing::warn!(error = %err, "could not initialise containers");
            Ok(groups::view(&path))
        }
    }
}

#[tauri::command]
fn container_create(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    name: String,
) -> Result<groups::ContainersView, String> {
    let keys = state.market.lock().unwrap().watched();
    let path = groups_file(&app);
    groups::ensure_default(&path, keys.clone())?;
    let view = groups::create(&path, &name, keys)?;
    sync_group_subscriptions(&app, state.inner());
    Ok(view)
}

/// Create a blank group without changing the active watch list. Used by the
/// Stocks card view's "New card" action.
#[tauri::command]
fn container_create_empty(app: AppHandle, name: String) -> Result<groups::ContainersView, String> {
    groups::create_empty(&groups_file(&app), &name)
}

#[tauri::command]
fn container_activate(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    name: String,
) -> Result<PlanDto, String> {
    let path = groups_file(&app);
    let stored = groups::read(&path)
        .groups
        .get(&name)
        .cloned()
        .ok_or_else(|| format!("container \"{name}\" does not exist"))?;
    groups::set_active(&path, &name)?;
    Ok(apply_watch(&app, state.inner(), parse_watch_keys(&stored)))
}

#[tauri::command]
fn container_rename(
    app: AppHandle,
    old: String,
    new: String,
) -> Result<groups::ContainersView, String> {
    groups::rename(&groups_file(&app), &old, &new)
}

#[tauri::command]
fn container_delete(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    name: String,
) -> Result<groups::ContainersView, String> {
    let path = groups_file(&app);
    let (view, next) = groups::remove(&path, &name)?;
    if let Some(active) = next {
        let stored = groups::read(&path)
            .groups
            .get(&active)
            .cloned()
            .unwrap_or_default();
        apply_watch(&app, state.inner(), parse_watch_keys(&stored));
    }
    sync_group_subscriptions(&app, state.inner());
    Ok(view)
}

#[tauri::command]
fn container_color(
    app: AppHandle,
    name: String,
    color: String,
) -> Result<groups::ContainersView, String> {
    groups::set_color(&groups_file(&app), &name, &color)
}

/// Every group as a card with resolved instrument metadata and any quote that is
/// already streaming. One call keeps the Stocks-tab card view cheap.
#[tauri::command]
fn container_cards(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Vec<fv_bridge::ContainerCardDto> {
    let path = groups_file(&app);
    let file = groups::read(&path);
    let view = groups::view(&path);
    let market = state.market.lock().unwrap();
    view.groups
        .into_iter()
        .map(|group| {
            let keys = file
                .groups
                .get(&group.name)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            let custom = file.custom.get(&group.name).cloned().unwrap_or_default();
            market.container_card(&group.name, group.color, keys, custom)
        })
        .collect()
}

/// Add a stock, option or other instrument to a named group. When that group is
/// active the live watch list is updated (and re-subscribed) too.
#[tauri::command]
fn container_add(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    name: String,
    key: String,
) -> Result<groups::ContainersView, String> {
    let key = key.trim().to_string();
    SymbolKey::parse(&key).map_err(|e| e.to_string())?;
    if state.market.lock().unwrap().instrument(&key).is_none() {
        return Err(format!("instrument {key} is not in the master yet"));
    }
    let path = groups_file(&app);
    let keys = groups::add_key(&path, &name, &key)?;
    let view = groups::view(&path);
    if view.active.as_deref() == Some(name.as_str()) {
        apply_watch(&app, state.inner(), parse_watch_keys(&keys));
    }
    sync_group_subscriptions(&app, state.inner());
    Ok(view)
}

/// Remove one instrument from a named group, mirroring the active watch list.
#[tauri::command]
fn container_remove(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    name: String,
    key: String,
) -> Result<groups::ContainersView, String> {
    let path = groups_file(&app);
    let keys = groups::remove_key(&path, &name, &key)?;
    let view = groups::view(&path);
    if view.active.as_deref() == Some(name.as_str()) {
        apply_watch(&app, state.inner(), parse_watch_keys(&keys));
    }
    sync_group_subscriptions(&app, state.inner());
    ensure_default_instrument(&app, state.inner());
    Ok(view)
}

#[tauri::command]
fn container_reorder(app: AppHandle, order: Vec<String>) -> Result<groups::ContainersView, String> {
    groups::reorder(&groups_file(&app), order)
}

/// Persist the manual ("Custom" sort) instrument order for one group.
#[tauri::command]
fn container_reorder_keys(
    app: AppHandle,
    name: String,
    keys: Vec<String>,
) -> Result<groups::ContainersView, String> {
    let path = groups_file(&app);
    groups::set_custom_order(&path, &name, keys)?;
    Ok(groups::view(&path))
}

// ---------------------------------------------------------------------------
// Index containers (independent from the Stocks watch groups)
// ---------------------------------------------------------------------------

fn ensure_index_containers(path: &std::path::Path) -> Result<groups::ContainersView, String> {
    let view = groups::view(path);
    if view.groups.is_empty() {
        groups::create_empty(path, "Indices")
    } else {
        Ok(view)
    }
}

/// Create the default Index card with every index the master knows about.
/// Only runs once: after the first successful seed, `seeded` is persisted so a
/// deliberately emptied or renamed card is never repopulated.
fn ensure_index_containers_seeded(
    app: &AppHandle,
    state: &Arc<AppState>,
) -> Result<groups::ContainersView, String> {
    let path = index_groups_file(app);
    let mut file = groups::read(&path);
    let mut created = false;
    if file.groups.is_empty() {
        groups::create_empty(&path, "Indices")?;
        file = groups::read(&path);
        created = true;
    }
    let default_empty = file.groups.len() == 1
        && file
            .groups
            .get("Indices")
            .map(|keys| keys.is_empty())
            .unwrap_or(false);
    if !file.seeded && (created || default_empty) {
        let keys: Vec<String> = state
            .market
            .lock()
            .unwrap()
            .index_rows(300)
            .into_iter()
            .map(|row| row.key)
            .collect();
        if !keys.is_empty() {
            for key in keys {
                groups::add_key(&path, "Indices", &key)?;
            }
            file = groups::read(&path);
            file.seeded = true;
            groups::write(&path, &file)?;
            let _ = sync_index_subscriptions(app, state);
        }
    }
    Ok(groups::view(&path))
}

#[tauri::command]
fn index_containers_list(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> groups::ContainersView {
    match ensure_index_containers_seeded(&app, state.inner()) {
        Ok(view) => view,
        Err(err) => {
            tracing::warn!(error = %err, "could not initialise index containers");
            groups::view(&index_groups_file(&app))
        }
    }
}

#[tauri::command]
fn index_container_create_empty(
    app: AppHandle,
    name: String,
) -> Result<groups::ContainersView, String> {
    let path = index_groups_file(&app);
    ensure_index_containers(&path)?;
    groups::create_empty(&path, &name)
}

#[tauri::command]
fn index_container_activate(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    name: String,
) -> Result<PlanDto, String> {
    let path = index_groups_file(&app);
    ensure_index_containers(&path)?;
    if !groups::read(&path).groups.contains_key(&name) {
        return Err(format!("index container \"{name}\" does not exist"));
    }
    groups::set_active(&path, &name)?;
    Ok(sync_index_subscriptions(&app, state.inner()))
}

#[tauri::command]
fn index_container_rename(
    app: AppHandle,
    old: String,
    new: String,
) -> Result<groups::ContainersView, String> {
    let path = index_groups_file(&app);
    ensure_index_containers(&path)?;
    groups::rename(&path, &old, &new)
}

#[tauri::command]
fn index_container_delete(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    name: String,
) -> Result<groups::ContainersView, String> {
    let path = index_groups_file(&app);
    ensure_index_containers(&path)?;
    let (view, _next) = groups::remove(&path, &name)?;
    sync_index_subscriptions(&app, state.inner());
    Ok(view)
}

#[tauri::command]
fn index_container_color(
    app: AppHandle,
    name: String,
    color: String,
) -> Result<groups::ContainersView, String> {
    let path = index_groups_file(&app);
    ensure_index_containers(&path)?;
    groups::set_color(&path, &name, &color)
}

#[tauri::command]
fn index_container_cards(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Vec<fv_bridge::ContainerCardDto> {
    let path = index_groups_file(&app);
    let _ = ensure_index_containers_seeded(&app, state.inner());
    let file = groups::read(&path);
    let view = groups::view(&path);
    let market = state.market.lock().unwrap();
    view.groups
        .into_iter()
        .map(|group| {
            let keys = file
                .groups
                .get(&group.name)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            let custom = file.custom.get(&group.name).cloned().unwrap_or_default();
            market.container_card(&group.name, group.color, keys, custom)
        })
        .collect()
}

#[tauri::command]
fn index_container_add(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    name: String,
    key: String,
) -> Result<groups::ContainersView, String> {
    let key = key.trim().to_string();
    SymbolKey::parse(&key).map_err(|e| e.to_string())?;
    let instrument = state
        .market
        .lock()
        .unwrap()
        .instrument(&key)
        .ok_or_else(|| format!("instrument {key} is not in the master yet"))?;
    if !instrument.is_index {
        return Err("only index instruments can be added to Index cards".into());
    }
    let path = index_groups_file(&app);
    ensure_index_containers(&path)?;
    groups::add_key(&path, &name, &key)?;
    sync_index_subscriptions(&app, state.inner());
    Ok(groups::view(&path))
}

#[tauri::command]
fn index_container_remove(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    name: String,
    key: String,
) -> Result<groups::ContainersView, String> {
    let path = index_groups_file(&app);
    ensure_index_containers(&path)?;
    groups::remove_key(&path, &name, &key)?;
    sync_index_subscriptions(&app, state.inner());
    Ok(groups::view(&path))
}

#[tauri::command]
fn index_container_reorder(
    app: AppHandle,
    order: Vec<String>,
) -> Result<groups::ContainersView, String> {
    let path = index_groups_file(&app);
    ensure_index_containers(&path)?;
    groups::reorder(&path, order)
}

#[tauri::command]
fn index_container_reorder_keys(
    app: AppHandle,
    name: String,
    keys: Vec<String>,
) -> Result<groups::ContainersView, String> {
    let path = index_groups_file(&app);
    ensure_index_containers(&path)?;
    groups::set_custom_order(&path, &name, keys)?;
    Ok(groups::view(&path))
}

// ---------------------------------------------------------------------------
// Orders
// ---------------------------------------------------------------------------

fn order_defaults_file(app: &AppHandle) -> std::path::PathBuf {
    orders::file_path(&config_dir(app))
}

#[tauri::command]
fn order_defaults_get(app: AppHandle) -> orders::OrderDefaults {
    orders::read_defaults(&order_defaults_file(&app))
}

#[derive(Debug, Clone, Serialize)]
struct OrderTargetDto {
    key: String,
    trading_symbol: String,
    exchange: String,
    expiry: Option<String>,
    strike: String,
    strike_type: String,
    lot_size: i64,
    instrument_type: String,
    is_index: bool,
    tradeable: bool,
    underlying: Option<String>,
    has_fno: bool,
    quote: Option<QuoteDto>,
}

/// Resolve one instrument into everything the generic order ticket needs.
/// Works for stocks, futures and options; indices are returned with
/// `tradeable: false` so F1/F2 can explain rather than guess.
#[tauri::command]
fn order_target(state: State<'_, Arc<AppState>>, key: String) -> Option<OrderTargetDto> {
    let parsed = SymbolKey::parse(&key).ok()?;
    let market = state.market.lock().unwrap();
    let instrument = market.instrument(&key)?;
    let tradeable = !instrument.is_index
        && instrument.lot_size > 0
        && !instrument.trading_symbol.trim().is_empty();
    Some(OrderTargetDto {
        key,
        trading_symbol: instrument.trading_symbol,
        exchange: parsed.exchange().to_string(),
        expiry: instrument.expiry,
        strike: instrument.strike,
        strike_type: instrument.strike_type,
        lot_size: instrument.lot_size,
        instrument_type: instrument.instrument_type,
        is_index: instrument.is_index,
        tradeable,
        underlying: instrument.underlying,
        has_fno: instrument.has_fno,
        quote: market.snapshot(&parsed.as_wire()),
    })
}

/// Pre-trade margin for the ticket using Shoonya's `GetBasketMargin`.
#[tauri::command]
async fn margin_required(
    state: State<'_, Arc<AppState>>,
    request: orders::PlaceOrderRequest,
) -> Result<orders::MarginDto, String> {
    if offline_mode() {
        return Err("margin is unavailable in offline/probe mode".into());
    }
    let shared = state.inner().clone();
    let session = ensure_session(&shared).await?;
    let (instrument, exchange, tick) = {
        let market = shared.market.lock().unwrap();
        let instrument = market
            .instrument(&request.key)
            .ok_or_else(|| format!("instrument {} is not in the master", request.key))?;
        let key = SymbolKey::parse(&request.key).map_err(|e| e.to_string())?;
        (
            instrument,
            key.exchange().to_string(),
            market.tick_size(&request.key),
        )
    };
    let payload = orders::build_margin_payload(
        &request,
        &session.uid,
        &session.actid,
        &exchange,
        &instrument.trading_symbol,
        instrument.lot_size,
        tick,
    )?;
    orders::margin(&shared.rest, &session.token, &payload).await
}

#[tauri::command]
fn order_defaults_set(
    app: AppHandle,
    defaults: orders::OrderDefaults,
) -> Result<orders::OrderDefaults, String> {
    let path = order_defaults_file(&app);
    orders::write_defaults(&path, &defaults)?;
    Ok(orders::read_defaults(&path))
}

/// Prepare and submit one live order. The UI has already shown a ticket; this
/// command validates again and never logs in or retries an ambiguous send.
#[tauri::command]
async fn place_order(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    request: orders::PlaceOrderRequest,
) -> Result<orders::OrderResult, String> {
    if offline_mode() {
        return Err("live orders are disabled in offline/probe mode".into());
    }
    let shared = state.inner().clone();
    let source = shared.market.lock().unwrap().status(Instant::now()).source;
    if source != SourceKind::Live {
        return Err("orders are allowed only on the live feed (not replay/idle)".into());
    }
    let session = shared
        .session
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| "no active session; connect to the venue first".to_string())?;

    let (instrument, exchange, tick) = {
        let market = shared.market.lock().unwrap();
        let instrument = market
            .instrument(&request.key)
            .ok_or_else(|| format!("instrument {} is not in the master", request.key))?;
        let key = SymbolKey::parse(&request.key).map_err(|e| e.to_string())?;
        (
            instrument,
            key.exchange().to_string(),
            market.tick_size(&request.key),
        )
    };

    if orders::is_derivative(
        &exchange,
        &instrument.instrument_type,
        instrument.expiry.as_deref(),
    ) && request.product.trim().eq_ignore_ascii_case("C")
    {
        return Err("CNC product is not enabled for F&O; use M - NRML or I - MIS".into());
    }

    let payload = orders::build_payload(
        &request,
        &session.uid,
        &session.actid,
        &exchange,
        &instrument.trading_symbol,
        instrument.lot_size,
        tick,
    )?;

    tracing::debug!(
        key = %request.key,
        symbol = %instrument.trading_symbol,
        exchange = %exchange,
        side = %request.side,
        product = %request.product,
        price_type = %request.price_type,
        lots = request.lots,
        tick = tick.map(|value| value.format(2)),
        "placing order"
    );
    let result = orders::submit(&shared.rest, &session.token, &payload).await?;
    tracing::info!(
        order_no = %result.order_no,
        key = %request.key,
        side = %request.side,
        lots = request.lots,
        price_type = %request.price_type,
        "order submitted"
    );

    if request.price_type.trim().eq_ignore_ascii_case("TSL") {
        let parse = |raw: Option<&str>, label: &str| -> Result<Price, String> {
            let text = raw
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| format!("{label} is required"))?;
            Price::parse_rounded(text).map_err(|_| format!("{label} is not valid"))
        };
        let trail = parse(request.trail.as_deref(), "trail amount")?;
        let trigger = parse(request.trigger_price.as_deref(), "trigger price")?;
        let limit = parse(request.price.as_deref(), "limit price")?;
        let qty = request
            .lots
            .checked_mul(instrument.lot_size)
            .filter(|qty| *qty > 0)
            .ok_or_else(|| "quantity is out of range".to_string())?;
        shared.trailing.lock().unwrap().push(TrailingStop {
            order_no: result.order_no.clone(),
            key: request.key.clone(),
            exchange: exchange.clone(),
            trading_symbol: instrument.trading_symbol.clone(),
            side: request.side.trim().to_ascii_uppercase(),
            qty,
            trail,
            trigger,
            limit,
        });
        tracing::info!(
            order_no = %result.order_no,
            trail = %trail.format(2),
            trigger = %trigger.format(2),
            "trailing stop registered"
        );
    }

    let _ = app.emit("orders:changed", &result.order_no);
    Ok(result)
}

/// Close an open position with a marketable protected limit order.
#[tauri::command]
async fn square_off(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    request: orders::SquareOffRequest,
) -> Result<orders::OrderResult, String> {
    if offline_mode() {
        return Err("live orders are disabled in offline/probe mode".into());
    }
    let shared = state.inner().clone();
    let source = shared.market.lock().unwrap().status(Instant::now()).source;
    if source != SourceKind::Live {
        return Err("square-off is allowed only on the live feed".into());
    }
    let session = ensure_session(&shared).await?;
    let (instrument, exchange, ltp, tick) = {
        let market = shared.market.lock().unwrap();
        let instrument = market
            .instrument(&request.key)
            .ok_or_else(|| format!("instrument {} is not in the master", request.key))?;
        let key = SymbolKey::parse(&request.key).map_err(|e| e.to_string())?;
        let tick = market.tick_size(&request.key);
        let ltp = market
            .snapshot(&request.key)
            .and_then(|quote| quote.ltp)
            .map(|price| price.text)
            .or_else(|| {
                request
                    .price
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
            })
            .ok_or_else(|| {
                "no live quote for this position yet; try again after the next tick".to_string()
            })?;
        (instrument, key.exchange().to_string(), ltp, tick)
    };
    let derivative = orders::is_derivative(
        &exchange,
        &instrument.instrument_type,
        instrument.expiry.as_deref(),
    );
    // A square-off must never be blocked by a stale CNC product on a position.
    let product = orders::normalize_product(&request.product, derivative)?;
    let side = request.side.trim().to_ascii_uppercase();
    if !matches!(side.as_str(), "B" | "S") {
        return Err("side must be B or S".into());
    }
    let quantity = request
        .quantity
        .checked_abs()
        .filter(|qty| *qty > 0)
        .ok_or_else(|| "square-off quantity must be non-zero".to_string())?;
    tracing::debug!(
        key = %request.key,
        symbol = %instrument.trading_symbol,
        exchange = %exchange,
        side = %side,
        quantity,
        product = %product,
        reference_price = %ltp,
        tick = tick.map(|value| value.format(2)),
        "square-off request"
    );
    let ticket = orders::PlaceOrderRequest {
        key: request.key.clone(),
        side,
        lots: 1,
        product: product.to_string(),
        price_type: "MKT".into(),
        price: Some(ltp),
        trigger_price: None,
        trail: None,
        validity: "DAY".into(),
    };
    let payload = orders::build_payload_for_qty(
        &ticket,
        &session.uid,
        &session.actid,
        &exchange,
        &instrument.trading_symbol,
        quantity,
        tick,
    )?;
    let result = orders::submit(&shared.rest, &session.token, &payload).await?;
    tracing::info!(
        order_no = %result.order_no,
        status = %result.status,
        key = %request.key,
        "square-off submitted"
    );
    shared
        .trailing
        .lock()
        .unwrap()
        .retain(|stop| stop.key != request.key);
    let _ = app.emit("orders:changed", &result.order_no);
    Ok(result)
}

// ---------------------------------------------------------------------------
// Index watch
// ---------------------------------------------------------------------------

#[tauri::command]
fn indices_list(state: State<'_, Arc<AppState>>) -> Vec<fv_bridge::IndexRowDto> {
    state.market.lock().unwrap().index_rows(300)
}

#[tauri::command]
fn stocks_list(state: State<'_, Arc<AppState>>) -> Vec<fv_bridge::StockRowDto> {
    state.market.lock().unwrap().stock_rows()
}

#[tauri::command]
fn stocks_watched(state: State<'_, Arc<AppState>>) -> Vec<fv_bridge::StockRowDto> {
    state.market.lock().unwrap().watched_stock_rows()
}

#[tauri::command]
fn stocks_search(
    state: State<'_, Arc<AppState>>,
    query: String,
    limit: Option<usize>,
) -> Vec<fv_bridge::StockRowDto> {
    state
        .market
        .lock()
        .unwrap()
        .search_stocks(&query, limit.unwrap_or(20))
}

// ---------------------------------------------------------------------------
// Account strip summary
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
struct AccountSummaryDto {
    session: session::SessionInfo,
    funds: Option<account::FundsDto>,
    totals: account::PositionTotals,
    positions: usize,
    /// Feed socket state, distinct from "session valid".
    connected: bool,
}

/// One call for the header cards. Never logs in: it uses the session that
/// auto-connect already adopted, or an existing one.
#[tauri::command]
async fn account_summary(state: State<'_, Arc<AppState>>) -> Result<AccountSummaryDto, String> {
    let shared = state.inner().clone();
    let connected = shared
        .market
        .lock()
        .unwrap()
        .status(Instant::now())
        .connected;

    // Screenshot and release probes set this so a render never touches the
    // broker. `session::peek` only reads the local session file/Mongo record.
    if offline_mode() {
        return Ok(AccountSummaryDto {
            session: session::peek().await,
            funds: None,
            totals: account::PositionTotals::default(),
            positions: 0,
            connected,
        });
    }

    if shared.session.lock().unwrap().is_none() {
        if let Some(resolved) = session::resolve_existing().await {
            *shared.session.lock().unwrap() = Some(resolved);
        }
    }

    let cached = shared.session.lock().unwrap().clone();
    let (funds, totals, positions) = match cached {
        Some(current) => {
            match account::positions(&shared.rest, &current.uid, &current.actid, &current.token)
                .await
            {
                Ok(view) => {
                    sync_position_keys(&shared, &view);
                    (view.funds, view.totals, view.positions.len())
                }
                Err(err) => {
                    tracing::warn!(error = %err, "account summary unavailable");
                    (None, account::PositionTotals::default(), 0)
                }
            }
        }
        None => (None, account::PositionTotals::default(), 0),
    };

    Ok(AccountSummaryDto {
        session: session::peek().await,
        funds,
        totals,
        positions,
        connected,
    })
}

/// Connect to the venue. The session is resolved automatically:
/// env token ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¾Ãƒâ€šÃ‚Â¢ persisted Mongo token (validated) ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¾Ãƒâ€šÃ‚Â¢ TOTP login.
#[tauri::command]
async fn start_live(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<String>, String> {
    stop_feed_impl(state.inner());

    let keys: Vec<SymbolKey> = {
        let market = state.market.lock().unwrap();
        market.subscription_keys()
    };
    if keys.is_empty() {
        return Err("add at least one instrument to the watch list first".into());
    }

    let refresh = state.feed_auth_failed.swap(false, Ordering::Relaxed);
    if refresh {
        *state.session.lock().unwrap() = None;
    }
    let resolved = if refresh {
        tracing::warn!("refreshing the session after a WebSocket auth rejection");
        match session::login_fresh().await {
            Ok(session) => session,
            Err(err) => {
                state.feed_auth_failed.store(true, Ordering::Relaxed);
                return Err(err);
            }
        }
    } else {
        session::resolve_session().await?
    };
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
    // Include every group card member, not just the active watch list.
    let keys = {
        let market = state.market.lock().unwrap();
        let union = market.subscription_keys();
        if union.is_empty() {
            keys
        } else {
            union
        }
    };
    let cfg = fv_feed::ws::WsConfig::new(&resolved.uid, &resolved.actid, &resolved.token);
    *state.session.lock().unwrap() = Some(resolved.clone());
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<fv_feed::ws::FeedCommand>();
    *state.feed_cmd.lock().unwrap() = Some(cmd_tx);

    let tx = state.tx.clone();
    let feed_state = state.clone();
    let shared = state.clone();
    let handle = app.clone();
    let notice_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let result = fv_feed::ws::run_forever(
            cfg,
            keys,
            move |frame| {
                let _ = tx.send(frame);
            },
            move |event| apply_feed_state(&feed_state, &notice_handle, event),
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
fn apply_feed_state(state: &Arc<AppState>, app: &AppHandle, event: fv_feed::ws::FeedState) {
    use fv_feed::ws::FeedState;

    let notice = {
        let mut market = state.market.lock().unwrap();
        match event {
            FeedState::Connected { source } => {
                market.set_source(SourceKind::Live);
                market.set_connected(true);
                market.clear_error();
                state.feed_auth_failed.store(false, Ordering::Relaxed);
                tracing::info!(%source, "live feed connected");
                // Clear any "Live feed: ..." banner once the socket is back.
                Some(String::new())
            }
            FeedState::Rejected { source } => {
                state.feed_auth_failed.store(true, Ordering::Relaxed);
                let message = format!(
                    "session rejected by the exchange for source '{source}'. Use Connect live to refresh the session."
                );
                market.set_connected(false);
                market.set_error(message.clone());
                Some(message)
            }
            FeedState::Error { message } => {
                market.set_connected(false);
                if message.contains("handshake rejected") || message.contains("NOT_OK") {
                    state.feed_auth_failed.store(true, Ordering::Relaxed);
                    let actionable = "session rejected by the exchange. Use Connect live to refresh the session.";
                    market.set_error(actionable.to_string());
                    Some(actionable.to_string())
                } else {
                    market.set_error(message.clone());
                    Some(message)
                }
            }
            FeedState::Reconnecting { attempt } => {
                market.set_connected(false);
                tracing::info!(attempt, "feed reconnecting");
                None
            }
        }
    };
    if let Some(message) = notice {
        let _ = app.emit("feed:error", message);
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
                        if let Frame::Order(order) = &frame {
                            tracing::debug!(
                                order_no = order.noren_order_no.as_deref().unwrap_or(""),
                                status = order.status.as_deref().unwrap_or(""),
                                report_type = order.report_type.as_deref().unwrap_or(""),
                                symbol = order.trading_symbol.as_deref().unwrap_or(""),
                                "order update frame"
                            );
                            let order_no = order.noren_order_no.clone().unwrap_or_default();
                            let _ = app.emit("orders:changed", order_no);
                        }
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

/// Server-side trailing stop-loss monitor.
///
/// The initial order is an SL-L; every two seconds the trigger is moved in the
/// favourable direction and the broker order is modified. Only the trigger move
/// is automated: a failed modify drops the monitor instead of retrying blindly,
/// so a filled/cancelled order is never modified by mistake.
async fn trailing_monitor(state: Arc<AppState>) {
    let mut ticker = tokio::time::interval(Duration::from_secs(2));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let min_move = Price::parse("0.05").unwrap_or(Price::ZERO);

    loop {
        ticker.tick().await;
        let stops = { state.trailing.lock().unwrap().clone() };
        if stops.is_empty() {
            continue;
        }
        let source = state.market.lock().unwrap().status(Instant::now()).source;
        if source != SourceKind::Live {
            continue;
        }
        let session = match state.session.lock().unwrap().clone() {
            Some(session) => session,
            None => continue,
        };

        let mut updates: Vec<(String, Price, Price)> = Vec::new();
        let mut drop: Vec<String> = Vec::new();

        for stop in stops {
            let ltp = {
                let market = state.market.lock().unwrap();
                market
                    .snapshot(&stop.key)
                    .and_then(|quote| quote.ltp)
                    .map(|price| Price::from_scaled(price.e6))
            };
            let Some(ltp) = ltp else { continue };

            let candidate = if stop.side == "S" {
                ltp.checked_add(stop.trail)
            } else {
                ltp.checked_sub(stop.trail)
            };
            let Some(candidate) = candidate else { continue };

            let improved = if stop.side == "S" {
                candidate
                    .checked_sub(stop.trigger)
                    .map(|move_by| move_by.scaled() >= min_move.scaled())
                    .unwrap_or(false)
            } else {
                stop.trigger
                    .checked_sub(candidate)
                    .map(|move_by| move_by.scaled() >= min_move.scaled())
                    .unwrap_or(false)
            };
            if !improved {
                continue;
            }

            let offset = stop.trigger.checked_sub(stop.limit).unwrap_or(Price::ZERO);
            let new_limit = if stop.side == "S" {
                candidate.checked_sub(offset)
            } else {
                candidate.checked_add(offset)
            };
            let Some(new_limit) = new_limit else { continue };

            let payload = orders::build_modify_payload(
                &stop.order_no,
                &session.uid,
                &stop.exchange,
                &stop.trading_symbol,
                stop.qty,
                new_limit,
                candidate,
            );
            match orders::modify(&state.rest, &session.token, &payload).await {
                Ok(()) => {
                    tracing::info!(
                        order_no = %stop.order_no,
                        trigger = %candidate.format(2),
                        "trailing stop moved"
                    );
                    updates.push((stop.order_no.clone(), candidate, new_limit));
                }
                Err(err) => {
                    tracing::warn!(order_no = %stop.order_no, error = %err, "trailing stop modify failed");
                    drop.push(stop.order_no.clone());
                }
            }
        }

        if !updates.is_empty() || !drop.is_empty() {
            let mut list = state.trailing.lock().unwrap();
            for (order_no, trigger, limit) in updates {
                if let Some(stop) = list.iter_mut().find(|stop| stop.order_no == order_no) {
                    stop.trigger = trigger;
                    stop.limit = limit;
                }
            }
            list.retain(|stop| !drop.contains(&stop.order_no));
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
    // Detailed broker traffic (requests/rejections) is logged by default while
    // order flow is being stabilised. Drop these targets back to `warn` once the
    // behaviour is verified.
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,h_finvasia_desktop=debug"));

    let mut log_dir = broker_log::log_dir();
    if std::fs::create_dir_all(&log_dir).is_err() {
        log_dir = std::env::temp_dir().join("h-finvasia-logs");
        let _ = std::fs::create_dir_all(&log_dir);
    }
    let appender = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("broker")
        .filename_suffix("log")
        .max_log_files(7)
        .build(&log_dir)
        .unwrap_or_else(|_| tracing_appender::rolling::never(&log_dir, "broker.log"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_ansi(false)
        .with_writer(appender)
        .init();
    tracing::info!(dir = %log_dir.display(), "broker diagnostics logging enabled");

    let (tx, rx) = mpsc::unbounded_channel();
    let state = Arc::new(AppState {
        market: Mutex::new(MarketData::new(env!("CARGO_PKG_VERSION"))),
        tx,
        producer: Mutex::new(None),
        feed_cmd: Mutex::new(None),
        session: Mutex::new(None),
        rest: account::RestClient::new().expect("rest client"),
        trailing: Mutex::new(Vec::new()),
        feed_auth_failed: AtomicBool::new(false),
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
            tauri::async_runtime::spawn(trailing_monitor(setup_state.clone()));

            let demo_handle = handle.clone();
            tauri::async_runtime::spawn(load_master(setup_state.clone(), handle.clone()));

            let demo_mode = std::env::var("FV_AUTOSTART_DEMO")
                .map(|v| v == "1")
                .unwrap_or(false);

            // Resume the previous watch list and, if a session is already valid,
            // connect without any manual step. `FV_NO_AUTOCONNECT=1` disables
            // this entirely (release smoke tests, CI) so those runs never touch
            // the broker.
            let auto_connect = std::env::var("FV_NO_AUTOCONNECT")
                .map(|value| value != "1")
                .unwrap_or(true);

            if !demo_mode {
                let resume_state = setup_state.clone();
                let resume_handle = handle.clone();
                tauri::async_runtime::spawn(async move {
                    let mut stored = groups::read_watch(&watch_file(&resume_handle));
                    // Initialise containers from the persisted watch list before
                    // any demo/replay can replace the in-memory watch set.
                    if let Err(err) =
                        groups::ensure_default(&groups_file(&resume_handle), stored.clone())
                    {
                        tracing::warn!(error = %err, "could not initialise containers");
                    }
                    if let Err(err) =
                        groups::save_active(&groups_file(&resume_handle), stored.clone())
                    {
                        tracing::warn!(error = %err, "could not sync the active container");
                    }
                    // Every card should stream, not just the active group.
                    sync_group_subscriptions(&resume_handle, &resume_state);
                    // Index cards are a separate, equally live set.
                    sync_index_subscriptions(&resume_handle, &resume_state);

                    // Empty install: seed NIFTY 50 so there is always something
                    // live and auto-connect has a subscription to open.
                    if stored.is_empty() && group_subscription_keys(&resume_handle).is_empty() {
                        if let Ok(key) = SymbolKey::parse(DEFAULT_WATCH_KEY) {
                            stored = vec![DEFAULT_WATCH_KEY.to_string()];
                            apply_watch(&resume_handle, &resume_state, vec![key]);
                            persist_active_container(&resume_handle, &resume_state);
                            sync_group_subscriptions(&resume_handle, &resume_state);
                        }
                    }

                    let keys: Vec<SymbolKey> = stored
                        .iter()
                        .filter_map(|key| SymbolKey::parse(key).ok())
                        .collect();

                    // Restoring the watch list is local and always happens.
                    if !keys.is_empty() {
                        let payload = {
                            let mut market = resume_state.market.lock().unwrap();
                            market.set_watch(keys.clone());
                            watch_snapshot(&market)
                        };
                        let _ = resume_handle.emit("watch:changed", payload);
                    }

                    if !auto_connect {
                        return;
                    }

                    let connect_keys = {
                        let market = resume_state.market.lock().unwrap();
                        market.subscription_keys()
                    };
                    if connect_keys.is_empty() {
                        return;
                    }

                    // Adopt a valid stored token first. If none validates, use
                    // the same TOTP QuickAuth flow as the fv-client and persist
                    // the new token for the next launch.
                    let resolved = match session::resolve_existing().await {
                        Some(session) => Some(session),
                        None => match session::resolve_session().await {
                            Ok(session) => {
                                tracing::info!(source = session.source.as_str(), "auto-login");
                                Some(session)
                            }
                            Err(err) => {
                                tracing::warn!(error = %err, "auto-connect login failed");
                                let _ = resume_handle.emit("feed:error", err);
                                None
                            }
                        },
                    };

                    if let Some(resolved) = resolved {
                        tracing::info!(keys = connect_keys.len(), "auto-connecting the live feed");
                        if let Err(err) = begin_live(
                            resume_handle.clone(),
                            resume_state.clone(),
                            resolved,
                            connect_keys,
                        )
                        .await
                        {
                            tracing::warn!(error = %err, "auto-connect failed");
                            let _ = resume_handle.emit("feed:error", err);
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
            remove_everywhere,
            start_replay,
            start_live,
            session_info,
            kite_bootstrap,
            chain_underlyings,
            chain_expiries,
            chain_load,
            chain_close,
            positions_load,
            orders_load,
            groups_list,
            group_save,
            group_load,
            group_delete,
            containers_list,
            container_create,
            container_create_empty,
            container_activate,
            container_rename,
            container_delete,
            container_color,
            container_cards,
            container_add,
            container_remove,
            container_reorder,
            container_reorder_keys,
            index_containers_list,
            index_container_create_empty,
            index_container_activate,
            index_container_rename,
            index_container_delete,
            index_container_color,
            index_container_cards,
            index_container_add,
            index_container_remove,
            index_container_reorder,
            index_container_reorder_keys,
            order_defaults_get,
            order_defaults_set,
            order_target,
            margin_required,
            place_order,
            square_off,
            indices_list,
            stocks_list,
            stocks_watched,
            stocks_search,
            account_summary,
            stop_feed,
        ])
        .run(tauri::generate_context!())
        .expect("error while running H-Finvasia");
}
