# Architecture

Companion to [`../fv-features.md`](../fv-features.md) and [`../README.md`](../README.md).
This document defines the crate boundaries and the invariants that keep the rewrite fast, portable
and testable.

## 1. Principle: separate ingest, state, render

The legacy terminal collapsed all three onto the WPF dispatcher. Three failures followed:

1. every tick mutated a bound grid item (`PropertyChanged` → invalidate → layout);
2. every exchange message was written to a text log on that same path (98.5 % of log volume);
3. order state shared a code path with market data, so a tick burst could delay an ack.

The rewrite enforces the separation at module boundaries:

```
 ┌───────────────┐   decoded frames  ┌──────────────┐    marks    ┌─────────────┐
 │  fv-feed      │──────────────────▶│  fv-engine   │────────────▶│  fv-bridge  │
 │  WS + decode  │                   │  Coalescer   │  30–60 Hz   │  DTOs       │
 └───────┬───────┘                   └──────────────┘   deltas    └──────┬──────┘
         │ order frames (fast path, own channel)                         │
         ▼                                                               ▼
 ┌───────────────┐                                              ┌─────────────────┐
 │ order state   │  ← never blocked by market data              │ src-tauri       │
 └───────────────┘                                              │ commands+events │
                                                                └────────┬────────┘
 ┌───────────────┐   reads    ┌──────────────┐                           │
 │  fv-master    │───────────▶│  fv-store    │──▶ MongoDB                ▼
 └───────────────┘            └──────────────┘   (local cache)   ┌─────────────┐
                                                                 │ ui/ Svelte  │
                                                                 └─────────────┘
```

**The webview never parses a tick.** It receives coalesced `QuoteDto` batches only.

## 2. Layers and responsibilities

| Layer | Owns | Must not |
|---|---|---|
| `fv-core` | `Price` (i64, 6 dp), `SymbolKey`, `Quote`, `CoreError` | touch I/O, network, DB |
| `fv-feed` | frame classification, decoding, `QuoteBook` merge, subscription planning, replay, WS transport | own UI state, know about Mongo |
| `fv-master` | `Instrument`, in-memory index/search/option-chain, Mongo loader | decode market data |
| `fv-store` | cache of record: trackers, snapshots, order journal | decode market data, own display models |
| `fv-engine` | `Coalescer`, `TokenBucket` | perform I/O |
| `fv-bridge` | `MarketData` state, DTOs for the UI, delta assembly | depend on Tauri, format pixels |
| `src-tauri` | Tauri commands/events, task lifecycle, master bootstrap | contain market rules |
| `ui/` | rendering, formatting, interaction | open sockets, parse frames, hold money as float |
| `fv-cli` | harness / measurement | contain business rules |

`fv-bridge` has **no Tauri dependency** on purpose: the entire UI-facing data path is unit-tested
without a webview or a window server, on every platform.

## 3. Why `src-tauri` is a separate workspace

The Tauri build embeds `ui/dist` at compile time, so building `src-tauri` requires a built frontend.
Keeping it out of the root workspace means:

- `cargo test --workspace` stays fast and **never depends on Node**;
- CI can run the Rust gates on a machine with no `npm`;
- the desktop build is an explicit, separate step (`scripts/release.ps1`).

Cost: two `target/` directories and a separate `cargo fmt`/`clippy` invocation. Accepted.

### 3.1 Dev vs production asset resolution (a real trap)

`tauri-build` decides whether the window loads `devUrl` or the embedded assets from
`DEP_TAURI_DEV`, which is `"true"` unless the `tauri` crate is compiled with the **`custom-protocol`**
feature:

```rust
pub fn is_dev() -> bool {
    env::var_os("DEP_TAURI_DEV").expect(..) == "true"
}
```

Only `tauri build` (the CLI) enables that feature. A plain `cargo build --release` therefore produces a
*release binary* that still behaves like a dev build: it loads `http://localhost:5173` and shows
`ERR_CONNECTION_REFUSED` when no dev server is running. The binary still contains the config
(including `devUrl`), so a string search cannot detect this.

The guard is behavioural: `scripts/release.ps1` starts the built app with a TCP listener on port
5173 and fails the release if a connection arrives. `SkipDevUrlProbe` exists for headless CI.

## 4. Invariants (each has a test)

### 4.1 Money
`Price` is `i64` scaled by `1e6`. Strict parse for user input, `parse_rounded` for feed floats.
Formatting rounds half away from zero. **No `f64` arithmetic in the domain, and no `f64` money over
IPC either** — `PriceDto` carries a display string plus an exact `i64` for sorting.

### 4.2 Symbol identity
`SymbolKey` is `(exchange, token)` where **token is a string**. `nfo|158614` normalises to
`NFO|158614`; `BSE|000123` keeps leading zeros. `SymbolKey` deliberately has no `Default`, so an
invalid key cannot be constructed.

### 4.3 Diff merging
`tk` resets the quote (a resubscribe cannot leave a stale high/low); `tf` merges field-wise; a `tf`
before any `tk` is tolerated; frames with no resolvable symbol are ignored.

The venue's `c` field is the **reference close**, so it populates both `close` and `prev_close` —
otherwise change/trend would always be blank. (Found by a test, not by inspection.)

### 4.4 Order-frame isolation
`peek_type()` reads `"t"` from the first 96 bytes without allocating (measured 0.01 µs). `om` frames
are dispatched before any market-data work, and a malformed neighbour cannot poison the path.
Unknown frame types are validated as JSON and preserved, so corruption is an error while a new
venue frame type stays observable.

### 4.5 Bounded work per frame
`Coalescer` emits at most `max_per_frame` quotes per interval and carries the remainder; marks
collapse per instrument; output order is deterministic. A key marked dirty with no quote yet is
dropped rather than retried forever.

### 4.6 Cache of record
`needs_backfill()` is the single rule preventing re-querying the venue: no row → fetch; same date and
found → skip; `found = false` retried at most 3 times, then abandoned.

### 4.7 UI state is explicit
`QuoteTable` is plain TypeScript (no runes) and is tested with vitest. Components copy `rows()` into
`$state` after each delta. That keeps the update path obvious and avoids a reactive graph that is
hard to reason about at 60 Hz.

Watched instruments get a **placeholder row immediately**, so adding an illiquid scrip shows up
rather than appearing to do nothing until its first tick.

### 4.8 Universe and ladder views are local

The **Stocks** list layout asks for `MarketData::watched_stock_rows()` (only the active container's
NSE EQ symbols) and `search_stocks()` for the flyout. Browsing and searching come from the local
master, so they cost no broker calls. The **Cards** layout calls `container_cards`, which resolves
every stored key through `MarketData::container_row` into a mixed stock/option/future/index row and
attaches whatever quote is already streaming. `container_add`/`container_remove` edit one named
group; when that group is active they also reconcile the live watch set. `container_reorder` stores
the card order in `watch-groups.json` and rejects a list that is missing or duplicating a group.
`container_create_empty` adds a blank card without moving the active pointer, and
`container_reorder_keys` stores the manual per-group order used by the **Custom** sort. Name/price/
insertion/custom sorting is shared pure TypeScript (`ui/src/lib/features/stocks/cards.ts`) across both
layouts; unknown custom keys fall back to insertion order rather than disappearing.

The **Index** workspace uses the same card component but its own container file
(`index-groups.json`) and active pointer, so Stocks and Index edits are independent. Its list view
shows the active Index group's members and adds new ones through `index_container_add`, which rejects
non-index instruments. `MarketData` keeps a separate `index_groups` subscription set; both set
functions share the 100-instrument budget and reconcile minimal plans.

Every instrument row (Index, Stocks list/cards, Market watch, instrument search, option chain)
right-clicks into `InstrumentMenu`, which resolves `order_target` once and emits the same items:
F1/F2 when `tradeable`, the option chain when `has_fno`, then the caller's row actions.

Positions and the order book are data-driven column tables
(`ui/src/lib/table/columns.ts`). Column widths and hidden ids persist in `localStorage`; the header
right-click menu toggles visibility and resets the layout. Drag listeners are attached to `window`
only for the duration of a resize.

Containers are named watch lists with a persisted `active` pointer; `watch_add`/`watch_remove` write
the live list back into the active container, keeping the sidebar count and the watch list in sync.
The table view uses that active watch list, but the feed subscribes to `MarketData::set_group_keys`
as well: every instrument in every group is in the desired-key union, so all cards stream live even
when their group is not active. Group membership is re-read after add/remove/delete and at startup.
The union (group members + active watch + option chain) is capped at the 100-instrument budget, with
excess keys reported in the plan rather than silently growing the socket. Switching a container
changes which list is shown; it does not drop the other cards' subscriptions. Market Watch renders
the same watch+group union (option-chain legs excluded), so instruments present only in a card still
appear there; its remove action calls `remove_everywhere`, while a card's x is scoped to that group.

The option chain's `groupStrikes`, `pickAtm`, `windowAround`, `deltaOi`, `fillPercent` and
`defaultTicketPrice` are pure TypeScript in `ui/src/lib/features/chain/chain.ts` and unit-tested. The
component only maps data to the quad table; the ATM reference (watched spot, typed ATM, or middle
strike) is resolved by those functions rather than scattered through markup.

Chain tabs are keyed by a UI-generated `chainId` and rendered directly in the top-level workspace tab
row (there is no secondary tab bar). Pinning marks the tab and opens a fresh Option-chain tab; the
App keeps every chain view mounted (hidden when inactive) so pinned rows and their `onDelta` handlers
survive page switches. `MarketData` stores a `BTreeMap<chainId, keys>`; `set_chain`/`clear_chain`
operate on one tab, and the union of all pinned chains participates in the same 100-instrument
budget as the watch list and groups. Closing a pinned tab calls `chain_close(chainId)`.

### 4.9 The bottom dock is lazy

Positions and the order book are mounted only while the bottom dock is open. Opening it is the
explicit signal to make the account REST calls; closing it unmounts both panels and releases their
listeners. There is no polling. `FV_NO_AUTOCONNECT=1` makes both commands return empty results, so
screenshots and release probes stay broker-free.

After `positions_load`, the position instruments are added to a dedicated subscription set, so the
websocket streams their LTPs. The UI keeps the REST row (including realised `rpnl`) and recomputes
LTP, MTM and total P&L from each quote without compounding. The account strip uses the same live
totals. `Frame::Order` updates are emitted as `orders:changed`, which refreshes the REST order book
and positions; the socket remains the trigger, not a polling timer.

### 4.10 One order target for every tradeable row

Rows do not build tickets themselves. They publish `(key, label)` on hover and click; the App asks
`order_target` for the instrument metadata and quote, then opens the single global ticket. F1/F2
resolve through the same path. This keeps lot size, exchange, symbol and price handling identical
for stocks, futures and options, and lets non-live sources disable confirmation in one place. The
ticket freezes that LTP snapshot as its default price, so later ticks cannot move the price under
the user. `buildOrderRequest` is pure and unit-tested; `margin_required` fetches the broker margin
on a debounce, never on every keystroke.

Right-clicking an option-chain leg opens a context menu whose left child submenu lists existing
Stocks cards and an inline "new card" creator (no modal). Choosing a card calls `container_add`;
creating one calls `container_create_empty` then `container_add`. The card file is the single owner
of group membership; active-group edits reconcile the live watch set.

Appearance is applied before mount (`applyStoredTheme` in `main.ts`) so the first paint matches the
saved theme. The picker also applies and persists each click immediately instead of waiting for an
Apply action. `THEMES` carries the display metadata for the 30-theme catalogue (11 terminal themes
plus the Visual Studio Code family); the actual colour tokens live in `app.css` under
`[data-theme=...]`. Theme/accent live in localStorage; group colours and card order live in
`watch-groups.json`. Neither is a broker concern. The collapsed header band and per-card collapsed
flags are UI-only localStorage preferences (`layoutPrefs.ts`), not broker or watch state. The shared
`Modal` handles Escape in its body as well as on the backdrop, so a focused input cannot swallow the
close key.

## 5. Threading and lifecycle (src-tauri)

```
producer task ──frames──▶ mpsc ──▶ consumer task ──deltas──▶ webview (event emit)
   ▲                                    │
   │ owns pacing                        └─ owns the state lock, absorbs frames,
   │                                       flushes every 16 ms, emits status 1 Hz
commands ── brief lock ──▶ MarketData
```

Rules:

- the state lock is **never held across an `.await`**;
- `stop_feed` aborts the producer task and signals the live socket, then resets the source;
- starting a feed always stops the previous one first (no orphan tasks);
- the frontend registers every event listener through one `track()` helper and unlistens all of them
  in `onMount`'s teardown, so navigation does not leak handlers;
- the instrument-metadata fetch uses a `cancelled` flag so a stale response cannot overwrite a newer
  selection;
- replay pacing measures **actual elapsed time** and replenishes a frame budget instead of sleeping
  per frame. Windows' timer granularity is ~15.6 ms, so a per-frame `sleep(1/hz)` silently caps a
  120 Hz capture at ~64 Hz — measured at 65/s before the fix, 120/s after. This matters because a
  real capture may run at thousands of frames/s;
- `FV_AUTOSTART_DEMO=1` starts the bundled replay after the window mounts, which is how the full
  producer → consumer → delta pipeline is smoke-tested without a click.

The Mongo driver requires an explicit `shutdown()`; `fv-store` exposes it and the CLI calls it on
every path. Without it the driver panics when the runtime is dropped.

### 5.1 Session resolution (live feed)

```
FV_ACCESS_TOKEN?  ──yes──▶ use it
        │ no
        ▼
CONFIGS_GLOBAL.FINVASIA_LOGIN_DETAIL ──validate (GetQuotes)──▶ use it
        │ invalid / absent
        ▼
fv-cred.json ──TOTP + QuickAuth──▶ persist fresh token/date to CONFIGS_GLOBAL ──▶ use it
```

Rules:

- A stored token is only adopted after it validates. If it does not, startup performs the same
  `QuickAuth` TOTP login as the fv-client and **writes the fresh token and date back** to
  `CONFIGS_GLOBAL` so the next launch (and the other tools) can reuse it.
- An empty install seeds `NSE|26000` (NIFTY 50) into the active group so there is always a
  subscription to open and a live row to show.
- `actid` is the **user id**, not `report_schoonya` (that field holds a password).
- The credential file is found via `FV_CRED_FILE`, then beside the exe, then `~/.fv/`, then
  `C:/repo/`; secrets are never logged.
- The REST client tries `NorenWClientWeb` first for the web-scoped account/order routes, falling back
  to `NorenWClientAPI` on a 401; `NorenWClientTP` is 502. Order writes keep `jData` as raw JSON and
  escape only embedded `&` characters; fully percent-encoding the JSON makes the venue reject it.
- **One websocket per token.** The app opens a single socket and reports lifecycle through
  `FeedState::{Connected, Rejected, Error, Reconnecting}`; it never opens a probe socket first, and
  `connected` in the status bar only becomes true after the handshake is accepted. Transport/gateway
  errors are emitted as `feed:error` so the UI shows a banner, while `run_forever` keeps reconnecting
  with backoff; a successful reconnect clears the banner.
- Web-minted sessions need `source:"WEB"`; the client retries once with `"API"` on `NOT_OK` and then
  surfaces `Rejected` to the UI instead of pretending to be live.

### 5.2 Subscription budget (politeness rule)

The operator asked that the broker be touched with **at most 100 instruments at a time**. That is a
product rule, not a venue limit, and it is enforced in three places:

| Place | Rule |
|---|---|
| `MarketData` | `MAX_TOTAL_SUBSCRIPTIONS = 100` across active watch, all group members **and** chain; `set_chain`/`set_group_keys` clamp, `set_watch` trims the chain group |
| `chain_load` | subscribes only `chain_window(...)` (+/-N strikes around the ATM, default +/-10), never the whole expiry |
| UI | nothing is subscribed on page open; *Load and subscribe* or an explicit context-menu action is required, and the status bar shows `subscribed N/100` |

Overflow is returned in the `PlanDto` (`rejected`, `truncated`) and shown, never silently dropped.
The rest of the expiry stays in the local master and is served as metadata; it is not streamed.

### 5.3 Order write path

`PlaceOrder` is the only write route. The path is deliberately narrow:

- The UI prepares a ticket first; `place_order` re-validates instrument, lot size, side, product,
  price/trigger/quantity before building `jData`. `ordersource` is `API` and `remarks` carries a
  unique reconciliation tag.
- Product is instrument-aware: cash equities use CNC/CNC-MIS; options/futures use NRML/MIS. The
  backend rejects CNC on NFO/BFO/CDS/MCX because the venue answers
  `ORA:Product CNC not enabled on exchange NFO`.
- **MKT is converted to a protected limit.** Shoonya blocks raw MKT on API orders, so `MKT` sends
  `prctyp: "LMT"` at the LTP plus a 1% buffer (minimum one tick). SL-L and TSL send `prctyp: "SL-L"`
  with `trgprc`. The margin calculator uses the same protected conversion because it also rejects MKT.
  Generated prices are aligned to the instrument tick from the master/quote (fallback 0.05 for
  NFO/BFO/MCX, 0.01 cash, 0.0001 CDS): protected buys round up, protected sells round down, and
  explicit limits/triggers snap to the nearest tick. This prevents
  `ORA:Price ... is not a multiple of tick size`.
- **TSL is an SL-L plus a monitor.** After a successful TSL placement the order is registered in an
  in-memory list; every two seconds the monitor reads the live LTP, moves the trigger favourably, and
  calls `ModifyOrder` (`post_write`). A failed modify drops the monitor instead of retrying blindly.
  Closing the app ends trailing; the resting order remains at the broker.
- **Square-off** right-click on a position sends the opposite side for the absolute net quantity
  with a protected marketable limit; it removes any trailing monitor for that key. It calls
  `ensure_session` so an expired cached session is re-resolved, falls back to the row's LTP when the
  websocket has not delivered a quote, and normalises CNC to NRML for derivatives.
- The feed source must be `Live`; replay/idle and `FV_NO_AUTOCONNECT=1` are rejected. The order command
  never logs in interactively: `ensure_session` reuses the cached live session or resolves one from the
  configured credentials.
- `post_write` keeps `jData` raw JSON, escaping only `&` inside values (symbols such as `M&M`) so the
  form body cannot split; full percent-encoding caused `jData is not valid json object`. It may retry
  once on a 401 against the other REST host. Network failures are **not** retried, so an ambiguous
  send cannot be duplicated; the user reconciles via the unique remarks in the order book.
- Broker REST calls log route, host, redacted `jData`, status, elapsed time and a redacted response at
  DEBUG to a rotating file under `%APPDATA%\com.hfinvasia.desktop\logs`. `broker_log` redacts
  `jKey`, `susertoken`, passwords, TOTP and API secrets before writing; write responses are logged up
  to 16 KiB and read responses up to 4 KiB so a large order/position book cannot flood the file.
  Websocket `om` order frames are logged with order number, status, report type and symbol. The
  desktop crate defaults to DEBUG (set `RUST_LOG=info` or `warn` to quieten it).
- The ticket is the confirmation step. There is no one-click live order and no background trading
  other than the explicit trailing-SL modifier.

## 6. Cross-platform rules| Rule | Reason |
|---|---|
| No `cfg(windows)` outside the optional Excel crate | core compiles identically everywhere |
| `#[cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` | no stray console in release builds, logs still visible in dev |
| `directories` for paths | `~/Library/Application Support`, `$XDG_DATA_HOME` |
| `keyring` for secrets | Credential Manager / Keychain / Secret Service |
| `rustls` for all TLS | identical validation and cipher behaviour per OS |
| ship fonts / no OS font assumptions | Linux may lack a UI font |
| UTC at the boundary | never rely on host timezone |
| no `usize` in persisted formats | 32/64-bit portability |

## 7. Data flow: startup

```
1. Tauri window opens immediately (no blocking work in setup)
2. background: load master  → Mongo FINVASIA_SYMBOL, else bundled sample
              → emit status:changed + master:ready
3. frontend: hydrate watch list + quotes + status
4. auto-connect adopts a valid stored session, or the user connects live → watch set reconciled →
   deltas flow. Replay is reserved for the CLI/CI (`FV_AUTOSTART_DEMO=1`), not a UI switch.
```

Nothing in step 1 waits on the database, so a slow or absent Mongo degrades instead of hanging the
window.

## 8. What is deliberately absent

- **No CozyMQ client** (decision record DR-1): the official WS API removed a 5,485-field protobuf
  recovery and an access-control question.
- **No protobuf**: documented JSON with a cheap discriminator beats an undocumented binary protocol.
- **No order entry yet**: it needs the REST client and an authorised session. It is shown as
  explicitly pending rather than faked, because a button that cannot place an order is worse than no
  button.
