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

The Tauri build embeds `ui/dist` at compile time, so `cargo build` in `src-tauri` requires a built
frontend. Keeping it out of the root workspace means:

- `cargo test --workspace` stays fast and **never depends on Node**;
- CI can run the Rust gates on a machine with no `npm`;
- the desktop build is an explicit, separate step (`scripts/release.ps1` builds UI → desktop).

Cost: two `target/` directories and a separate `cargo fmt`/`clippy` invocation. Accepted.

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

## 6. Cross-platform rules

| Rule | Reason |
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
4. user starts demo (replay) or live → watch set reconciled → deltas flow
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
