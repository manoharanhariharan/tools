# H-Finvasia

Cross-platform rewrite of the Finvasia/Shoonya **NorenTrader (UFT)** dealer terminal, built on the
in-house stack: **Tauri 2 + Rust workspace + Svelte 5**.

> **Read [`fv-features.md`](fv-features.md) first.** It contains the reverse-engineering findings, the
> full feature inventory for migration, the bottleneck analysis, the transport decision, and the
> local-MongoDB data model. This file covers the code that exists today.

---

## Status

| Layer | State |
|---|---|
| `fv-core` — money, symbol keys, quotes | ✅ |
| `fv-feed` — WS framing, diff-merge, subscriptions, replay, live transport | ✅ |
| `fv-master` — Mongo-backed instrument master + in-process index | ✅ |
| `fv-store` — Mongo cache: trackers, snapshots, order journal | ✅ |
| `fv-engine` — tick coalescing, order-rate budget | ✅ |
| `fv-bridge` — UI-facing market state + DTOs (no Tauri dependency) | ✅ |
| `src-tauri` — Tauri 2 desktop shell | ✅ |
| `ui/` — Svelte 5 terminal UI | ✅ (market watch, snap quote, search, diagnostics) |
| `fv-cli` — headless harness | ✅ |
| `fv-market` — REST client | ⬜ Phase 2b |
| Order entry / books / holdings | order entry (LMT, confirm ticket) + positions/order book in the bottom dock; holdings pending |

**What works today:** the desktop app launches, loads 182k instruments from your local Mongo (or a
bundled sample), and shows a live-updating market watch + snap quote fed by a replay capture - no
credentials required. Search, watch-list editing and diagnostics are wired to the real engine.

**Order entry** is wired through a prepared ticket: hover a CE/PE row, press B or S, review the
ticket, then Confirm. Defaults (lots/product/validity) are saved locally. Orders are LMT only
(Shoonya rejects MKT), allowed only on the live feed, and never retried automatically. Positions
and the order book live in a resizable bottom dock.

**Still pending:** holdings, order modify/cancel, product conversion and algo orders. The test
suite never sends a real order.

---

## Desktop app

### Prerequisites

- Rust 1.85+ (workspace pins `rust-version`)
- Node 20+ (built against Node 24)
- MongoDB on `127.0.0.1:27017` (optional — the app degrades to a bundled sample)
- WebView2 (preinstalled on Windows 10/11; the shipped Finvasia terminal uses it too)

### Run in development

```bash
cd C:\public-repo\tools\H-Finvasia
npm install            # once
npm run tauri:dev      # builds Rust + starts Vite + opens the window
```

First run compiles the Tauri dependency tree, so expect several minutes. Later runs are fast.

### Build a release app

```bash
npm install
npm run tauri:build          # runs build:ui, then `tauri build --no-bundle`
```

The executable lands at `src-tauri/target/release/h-finvasia-desktop.exe` and runs standalone.

> **Do not use `cargo build --release` for the desktop app.** That omits the `custom-protocol`
> feature, so `tauri-build` marks the build as *dev* and the window tries `devUrl`
> (`localhost:5173`) instead of the embedded frontend — you get `ERR_CONNECTION_REFUSED`.
> `tauri.conf.json`'s `devUrl` is only meant for `tauri dev`. `scripts/release.ps1` builds through
> the CLI and then probes port 5173 to prove the shipped app never touches it.

`scripts/release.ps1` does all of this and publishes to `releases/`.

### Layout

The workspace opens on a four-card account strip - **Account details** (user, account, login
state), **Funds available**, **RPNL**, **MTM** - followed by four tabs: **Index**, **Stocks**,
**Option chain**, **Instruments**. Market watch and Groups/Diagnostics sit behind the **More** menu;
Positions and the order book live in the bottom dock, so the working area stays uncluttered.

**Stocks** shows only the watched stocks in the active container. Use the search bar above the list
to find an NSE stock; matches appear in a flyout and can be added directly. The containers sidebar
creates, deletes and renames lists (double-click a name to edit). The active container is the live
watch list, so switching it re-subscribes within the 100-instrument budget; changes are saved
locally and cost no broker calls.

**Option chain** uses the usual quad layout: calls on the left, puts on the right, strike price in
the middle. Each side is mirrored around the strike (trade, OI, d OI, volume, LTP, change, bid qty,
bid, ask, ask qty, then the strike). OI and volume cells carry a proportional background fill. Calls
and puts are tinted by moneyness (ITM / ATM / OTM) around the spot, with the ATM row highlighted.
The outermost column on each side exposes B/S on row hover; both open the same prepared ticket.
The default view is the 10 strikes either side of the ATM; the rest of the chain remains in the
local master but is not streamed.

**Bottom dock** holds the read-only position book and order book side by side. Drag its top edge to
resize the height, drag the middle divider to resize the columns, and collapse it back to a footer
bar. It is collapsed by default so no account REST calls happen until you open it; refreshes are
manual, never on a timer.

### Using it

1. Double-click `h-finvasia-v<tag>.exe`.
2. **More -> Demo feed** (or *Diagnostics -> Start demo feed*) replays a bundled capture at
   ~120 frames/s; the watch list auto-populates from the capture.
3. Click a row to load the snap quote; **Instruments** searches the master and adds scrips.
   Search is relaxed and multi-term: `nifty 25800`, `25800 nifty`, `NIFTY 26NOV 25800` all find the
   25800-strike NIFTY contracts. Adding an instrument while **live** subscribes it on the open socket
   immediately - no reconnect, no manual refresh.
4. **Right-click any row** (Index, Stocks, Instruments, Market watch) for actions.
   When the instrument's underlying has derivatives the menu offers **Open option chain
   (UNDERLYING)**; choosing it switches to the Option chain tab, selects that underlying's nearest
   expiry, and loads the chain centred on the watched spot (or the entered ATM).
5. **Buy / sell** from the chain: hover a CE/PE row and press **B** or **S**. The ticket is prefilled
   from your saved defaults and the current ask/bid. Nothing is sent until **Confirm**. Orders are
   allowed only while the feed is live; replay/offline tickets open for review with Confirm disabled.
6. **Positions & order book** live in the bottom dock. Open it from the footer bar; each panel has its
   own Refresh button. After a confirmed order the order book refreshes automatically if it is open.
7. **Diagnostics** shows backend (`mongo` vs `in-memory`), frame rate, decode errors, and the live
   connect form.

On launch the app restores the last watch list and, if a valid session is already stored,
**connects automatically** - no click needed. No OAuth flow is created or needed: the app adopts the
token already persisted in `STOCK.CONFIGS_GLOBAL` (or `FV_ACCESS_TOKEN`); the public API OAuth
handshake is not used. Your morning login remains the only login. The stored token is read-only, so
other tools keep their session. A fresh TOTP login happens only when you explicitly press Connect and
no valid stored token exists. `FV_NO_AUTOCONNECT=1` disables startup adoption and every
broker-backed probe/account/order path. Verified against the live venue (NTPC/ACC streaming).

### Operating limits (deliberately gentle on the broker)

| Rule | Value |
|---|---|
| Total instruments subscribed at once | **100** (watch + option chain combined) |
| Option-chain legs subscribed | only a +/- strikes window (default +/-10 = 42 legs); other strikes stay in the local master as metadata |
| Websocket connections per session | 1, opened at launch when a valid session exists (or via Diagnostics -> Connect live) |
| Automatic broker calls | **one connect at launch** when a valid session exists; the account strip refreshes only on mount, on first connect, or manually - never on a timer |
| Automatic login | never - startup only *adopts* a valid stored/env token; a fresh TOTP login happens only when you press Connect |

| Order type | LMT only; one explicit Confirm per ticket; no automatic retry |
| Account/book calls | positions and order book load only when the bottom dock is opened or refreshed |

Overflow is **refused and reported**, never silently dropped. The status bar shows
`subscribed N/100` so the budget is always visible.


---

## Releases

```powershell
.\scripts\release.ps1 -Message "feat(h-finvasia): ..."
.\scripts\release.ps1 -SkipTests -NoPush
```

Runs frontend + Rust gates, commits source, builds both binaries, publishes to **`releases/`**:

| Artifact | What it is |
|---|---|
| `h-finvasia-v<tag>.exe` | **desktop app** — double-click |
| `h-finvasia-cli-v<tag>.exe` | headless CLI |

plus `BUILD.txt`, `SHA256SUMS.txt`, `README.md`. (Sibling tools use `publish/`; `releases/` is a
deliberate choice for this project.)

---

## CLI

```bash
cargo run -p fv-cli -- doctor
cargo run -p fv-cli -- master stats
cargo run -p fv-cli -- master search RELIANCE --limit 10
cargo run -p fv-cli -- master chain NIFTY --expiry 2026-12-01
cargo run -p fv-cli -- replay crates/fv-feed/tests/fixtures/sample_feed.jsonl
cargo run -p fv-cli -- bench  crates/fv-feed/tests/fixtures/sample_feed.jsonl --iters 20000
```

Measured against `STOCK.FINVASIA_SYMBOL` on this machine:

| Operation | Result |
|---|---|
| load 182,901 instruments | **2.69 s** (release) |
| distinct underlyings | 19,442 |
| `chain NIFTY 2026-12-01` | 550 legs, strike-ordered |
| parse + merge throughput | 225,849 frames/s (4.43 µs/frame) |
| order-frame fast path | 0.01 µs/frame |

---

## Configuration

| Variable | Default | Meaning |
|---|---|---|
| `FV_CRED_FILE` | `fv-cred.json` beside the exe, `%USERPROFILE%\.fv\`, or `C:\repo\` | Credentials (`authparams.userid/password/api_secret/f2token`) |
| `FV_ACCESS_TOKEN` | — | Explicit session override (skips resolution) |
| `FV_UID` / `FV_ACTID` | from the credential file | Used when no credential file is present |
| `FV_MONGO_URI` | `mongodb://127.0.0.1:27017` | Master + cache + persisted session |
| `FV_MONGO_DB` | `STOCK` | Database name |
| `FV_MONGO_MASTER_COLLECTION` | `FINVASIA_SYMBOL` | Master collection |
| `FV_AUTOSTART_DEMO` | — | `1` starts the bundled replay at launch (kiosk / CI smoke test) |
| `RUST_LOG` | `info` | `tracing` filter |

**Credentials are never written back** to Mongo or to disk. `report_schoonya` in the existing
credential file is a *password*, not an account id — it is deliberately ignored.

**No credentials are read from disk.** The token comes from the environment; the planned keychain
integration will replace that without changing call sites.

---

## Tests and quality gates

```bash
cargo test --workspace                       # Rust core (no Node, no Mongo needed)
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
npm run check && npm test                    # svelte-check + vitest
npm run build:ui
```

`src-tauri` is a **separate workspace** so the Rust gates never require a built frontend; it is
formatted/linted/tested separately (see `docs/ARCHITECTURE.md` §3).

---

## Layout

```
Cargo.toml                 core workspace (crates/*)
crates/
  fv-core/                 Price, SymbolKey, Quote, errors
  fv-feed/                 frame parsing, QuoteBook merge, subscriptions, replay, WS
    tests/fixtures/        sample_feed.jsonl
  fv-master/               Instrument, MemoryMaster index, Mongo loader
  fv-store/                Store (memory | mongo), trackers, snapshots, order journal
  fv-engine/               Coalescer, TokenBucket
  fv-bridge/               MarketData + UI DTOs (no Tauri)
  fv-cli/                  harness -> binary `h-finvasia`
src-tauri/                 Tauri 2 desktop shell (separate workspace)
ui/                        Svelte 5 + Vite
fv-features.md             reverse engineering + migration plan (source of truth)
docs/ARCHITECTURE.md       boundaries, invariants, threading, cross-platform rules
releases/                  committed builds
scripts/release.ps1        gates -> build -> publish -> commit -> push
scripts/make-icon.ps1      regenerates src-tauri/icons
```

## Known limitations

- **Desktop builds must go through `npm run tauri:build`.** A plain `cargo build --release` produces a
  binary that tries to load `localhost:5173` and shows `ERR_CONNECTION_REFUSED`. The release script
  enforces this with a live port probe.
- **Order entry is LMT only.** Shoonya rejects MKT; the ticket and defaults expose LMT. Modify/cancel,
  product conversion, holdings and algo orders are absent.
- **Order send is live-feed only and never retried automatically.** Replay/offline tickets open for
  review with Confirm disabled. A network timeout is ambiguous; reconcile by the unique `remarks` tag
  in the order book before retrying.
- **No real order is sent by the automated tests.** Payload/response and the live/offline guards are
  unit-tested; the first live confirmation should be checked in the order book.
- **The bottom dock is collapsed by default.** Positions/order book load only when it is opened;
  there is no polling.
- **The Stocks search flyout is NSE cash equities only.** Containers are generic watch lists and may
  hold indices/options added from other tabs.
- **Live feed is verified for touchline ticks** (`tk`/`tf`). Depth (`dk`/`df`) and order updates
  (`om`) are parsed but not yet wired to a UI panel.
- **One websocket per session token.** Opening a second connection while the first is live makes the
  venue go silent, so the app never probes and connects — it opens a single socket and reports the
  real state through `FeedState`.
- **REST split.** General calls use `NorenWClientAPI`; `Limits`/`PositionBook` use `NorenWClientWeb` because the persisted token is web-scoped. The older `NorenWClientTP` host returns 502.
- **Stocks covers the NSE EQ series** (currently 2,786 symbols, including ETFs). BSE listings and other NSE series (BE, bonds, SMS) are not in the list yet.
- **Option-chain IV is not shown** because the WebSocket frames we receive do not carry it; d OI is computed from `oi` minus the venue `poi`.
- **Spot/ATM requires the underlying to be watched.** The chain centres on the watched spot; without it (or a typed ATM) it falls back to the middle strike of the full chain.
- **Master load is 2.7 s**, not the <1 s target. Next step: `RawDocumentBuf` zero-copy reads.
- **MongoDB 4.4.2 is EOL**; driver 3.10 works today, plan is 7.x.
- **Dead indexes** exist in the local database (`INSTRUMENT` indexes `tradingSymbol`, documents use
  `tradingsymbol`). See `fv-features.md` §7.9.
- **Chain publishes `tcp://` in the legacy terminal**; the new client uses `wss://` only.
