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
| Order entry / books / holdings | order entry (LMT/MKT-protected/SL-L/TSL, confirm ticket + margin) + positions/order book with square-off; holdings pending |

**What works today:** the desktop app launches, loads 182k instruments from your local Mongo (or a
bundled sample), and shows a live-updating market watch + snap quote fed by a replay capture - no
credentials required. Search, watch-list editing and diagnostics are wired to the real engine.

**Order entry** is wired through a prepared ticket: hover a CE/PE row, press B or S, review the
ticket, then Confirm. Quantity is focused when the ticket opens, and Enter submits the same as
Confirm. The price starts at the LTP that was running when the ticket opened. Types are **LMT**,
**MKT (protected marketable limit)**, **SL-L** and **TSL** (server-side trailing SL). The ticket shows
an **internal margin estimate** (no broker round-trip): option buys use premium, option sells use
max(premium, 15% strike notional), futures 15%, and equity 20/50/100% by product. Defaults
(lots/product/validity/type) are saved locally. Orders are allowed only on the live feed and never
retried automatically. Every terminal order update raises a fixed screen toast: fills show quantity
and average price, rejections show the broker's reason, and cancels are amber. Positions and the order
book live in a resizable bottom dock; right-click a live position to square it off with a protected
market order.

**Still pending:** holdings, product conversion and algo orders. The test suite never sends a real
order.

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

`scripts/release.ps1` does all of this and publishes to `releases/`. It sets `SOURCE_DATE_EPOCH`
to the current UTC day before building, so the artifact tag always follows today's
`YY.MM.DD.<seq>` instead of a stale compile date.

### Layout

The workspace opens on a four-card account strip - **Account details** (user, account, login
state), **Funds available**, **RPNL**, **MTM**. The larger chevron at the right collapses it to a
single-line band with the feed state, account, cash, RPNL, MTM and position count; the same
right-hand control expands it again and the choice is remembered. RPNL and MTM are recomputed from
the position snapshot plus live websocket LTPs, so the header moves with the market. The icon-labelled
top-level tabs are **Index**, **Stocks**, one or more **Option chain** tabs, **Market watch** and
**Settings**. Pinning a chain turns it into its own top-level tab and leaves a fresh Option chain tab
for the next underlying, so there is no second tab bar. Settings contains Order defaults, Appearance
and Diagnostics, so there is no hidden More menu. Positions and the order book live in the bottom
dock, keeping the working area uncluttered.

**Stocks** has two layouts. **List** shows the watched stocks in the active group; use the search
bar above it to find an NSE stock. The groups sidebar creates, deletes and renames lists
(double-click a name to edit), and each group has a color chip that opens the system color picker.
**Cards** renders every group as a card in a three-column, endlessly scrolling grid. Cards can be
dragged to reorder (the order is saved in `watch-groups.json`), collapsed individually, renamed
inline (double-click the name, keeping the same slot), and deleted with the card's **x** button.
Every card has an always-visible search box that adds stocks, futures, indices or options to that
specific group, a per-card **Sort** selector (Name, Price, Added, Custom) and larger **+**/**toggle**
icons. A **New card** tile creates an empty group; the workspace-wide Sort still controls the List
view and is the fallback for new cards. Cards keep their natural content
height and the grid scrolls vertically instead of squeezing row content. Every instrument stored in
any group is subscribed, so all cards stream live - not just the active one. The active group remains
the list view's watch list; switching it changes which list is shown without dropping the other
cards' subscriptions. The combined group/watch/chain set stays within the 100-instrument budget, and
anything beyond it is reported rather than silently dropped.

**Market watch** shows the union of the active watch list and every Stocks group member, so indices
and instruments living only in cards still appear there. Its **x** removes an instrument from every
owner (active list and groups); the per-card **x** removes only from that card. The full-master search
box at the top adds directly to the active group.

**Option chain** starts blank when opened directly. Type an underlying and pick it from the local
suggestions; the expiry, strikes, ATM and filter controls then appear. Opening a chain from a row
context menu pre-fills and loads it immediately. Once loaded, **Pin** keeps that chain alive as its
own top-level tab and leaves a fresh **Option chain** tab for the next underlying. Pinned tabs stay
subscribed even when you switch workspace pages; close a pinned tab to release its legs. Chain tabs share the same 100-instrument
budget as the watch list and Stocks groups. The quad layout has calls on the left, puts on the right
and strike price in the middle. Each side is mirrored around the strike (trade, OI, d OI, volume, LTP, change, bid qty,
bid, ask, ask qty, then the strike). OI and volume cells carry a proportional background fill. Calls
and puts are tinted by moneyness (ITM / ATM / OTM) around the spot, with the ATM row highlighted.
The outermost column on each side exposes B/S on row hover; both open the same prepared ticket.
The default view is the 10 strikes either side of the ATM; the rest of the chain remains in the
local master but is not streamed.

**Bottom dock** holds the position book (live, with right-click square-off) and the read-only order
book side by side. Position LTP, MTM and total P&L are recomputed from the websocket; the order book
refreshes when an order update arrives on the socket. Drag its top edge to
resize the height, drag the middle divider to resize the columns, and collapse it back to a footer
bar. It is collapsed by default so no account REST calls happen until you open it; refreshes are
manual, never on a timer.

**F1 / F2** buy and sell the last hovered or clicked row (or option leg) from Stocks, Market watch
or the option chain. Chain arming is side-aware: a call cell arms its CE and a put cell arms its PE.
The card view highlights the armed row. If the instrument is not tradeable the app explains instead
of opening a ticket.

**Appearance** (Settings -> Appearance) offers eleven themes - Midnight, Graphite, Ocean, Contrast,
Light, Nord, Solarized, Tokyo Night, Forest, Plum and Sand - plus six accent presets and a custom
accent. Selecting a theme or accent applies it live (there is no Apply button); the choice is stored
on this machine and applied before the next first paint.

### Using it

1. Double-click `h-finvasia-v<tag>.exe`.
2. The app restores the last watch list and, when a stored session is valid, connects the live feed
   automatically. The UI has no demo/replay switch; bundled captures remain available to the
   headless CLI and CI (`FV_AUTOSTART_DEMO=1`).
3. Click a row to load the snap quote. **Market watch** now carries the full master search box and
   adds any instrument directly to the watch list. The **Stocks -> Cards** view can also add
   instruments to any specific group with its **+** button.
   Search is relaxed and multi-term: `nifty 25800`, `25800 nifty`, `NIFTY 26NOV 25800` all find the
   25800-strike NIFTY contracts. Adding an instrument while **live** subscribes it on the open socket
   immediately - no reconnect, no manual refresh.
4. **Right-click any row** (Index, Stocks, Market watch) for actions.
   When the instrument's underlying has derivatives the menu offers **Open option chain
   (UNDERLYING)**; choosing it switches to the Option chain tab, selects that underlying's nearest
   expiry, and loads the chain centred on the watched spot (or the entered ATM).
5. **Buy / sell** from the chain: hover a CE/PE row and press **B** or **S**, or hover any tradeable
   row and press **F1** (buy) / **F2** (sell). The ticket is prefilled from your saved defaults and
   the LTP snapshot from when the dialog opened; margin is estimated internally. Nothing is sent
   until **Confirm** or Enter. Orders are allowed only while the feed is live; replay/offline tickets
   open for review with Confirm disabled. Right-click a chain cell: the left-hand child menu lists
   Stocks cards and has an inline new-card field.
6. **Positions & order book** live in the bottom dock. Open it from the footer bar; each panel has its
   own Refresh button. After a confirmed order the order book refreshes automatically if it is open.
7. **Diagnostics** shows backend (`mongo` vs `in-memory`), frame rate, decode errors, and the live
   connect form.

On launch the app restores the last watch list. If there are no instruments anywhere it seeds
**NIFTY 50** (`NSE|26000`) so there is always a live row and a subscription to open. A valid session
already stored in `STOCK.CONFIGS_GLOBAL` (or `FV_ACCESS_TOKEN`) is adopted and the app **connects
automatically**; the public API OAuth handshake is not used. If the stored token no longer validates,
the app runs the same TOTP `QuickAuth` login as the fv-client, then writes the fresh token and date
back to `CONFIGS_GLOBAL` so the next launch can reuse it. Gateway/transport drops are surfaced in the
UI and the websocket reconnects with backoff; if the gateway is unreachable the banner says so
instead of failing silently. `FV_NO_AUTOCONNECT=1` disables startup adoption/login and every
broker-backed probe/account/order path. Verified against the live venue (NTPC/ACC streaming).

### Operating limits (deliberately gentle on the broker)

| Rule | Value |
|---|---|
| Total instruments subscribed at once | **100** (active watch + all group members + option chain combined) |
| Option-chain legs subscribed | only a +/- strikes window (default +/-10 = 42 legs) per chain tab; all pinned tabs share the 100 budget, and other strikes stay in the local master as metadata |
| Websocket connections per session | 1, opened at launch when a valid session exists (or via Settings -> Diagnostics -> Connect live) |
| Automatic broker calls | **one connect at launch** when a valid session exists; the account strip refreshes only on mount, on first connect, or manually - never on a timer |
| Automatic login | adopts a valid stored/env token; if none validates, performs the fv-client TOTP login and stores the fresh token in `CONFIGS_GLOBAL` |

| Order type | LMT, MKT (protected limit), SL-L, TSL; one explicit Confirm per ticket; no automatic retry |
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
- **MKT is a protected marketable limit.** Shoonya rejects raw MKT on API orders, so the app adds a
  1% buffer (minimum one tick) to the LTP and sends LMT. The order book therefore shows LMT. Modify/
  cancel, product conversion, holdings and algo orders are absent.
- **TSL runs only while the app is live.** The initial order is SL-L; a background monitor moves the
  trigger every two seconds as LTP improves. Closing the app stops further trailing. A failed modify
  drops the monitor rather than retrying ambiguously.
- **Order send is live-feed only and never retried automatically.** Replay/offline tickets open for
  review with Confirm disabled. A network timeout is ambiguous; reconcile by the unique `remarks` tag
  in the order book before retrying.
- **No real order is sent by the automated tests.** Payload/response and the live/offline guards are
  unit-tested; the first live confirmation should be checked in the order book.
- **The bottom dock is collapsed by default.** Positions/order book load only when it is opened;
  there is no polling.
- **The Stocks list-view search flyout is NSE cash equities only.** Groups are generic watch lists:
  the card view search (`+` on a group) deliberately searches the full master so options, futures
  and indices can be added there. All group members are subscribed, so cards stream live even when
  their group is not active; the combined group/watch/chain union is capped at 100 instruments.
- **F1/F2 act on the last hovered or clicked row/leg.** The shortcut target is shown in the tabs
  bar; remapping keys is not configurable yet.
- **Themes and group colours are stored on this machine** (localStorage / `watch-groups.json`).
  They are not synced to other devices.
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
