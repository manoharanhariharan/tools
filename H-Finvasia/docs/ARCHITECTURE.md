# Architecture

Companion to [`../fv-features.md`](../fv-features.md) and [`../README.md`](../README.md).
This document defines the crate boundaries and the invariants that keep the rewrite fast and
cross-platform.

## 1. Principle: separate ingest, state, render

The legacy terminal collapsed all three onto the WPF dispatcher. Three failures followed:

1. every tick mutated a bound grid item (`PropertyChanged` → invalidate → layout);
2. every exchange message was written to a text log on that same path (98.5 % of log volume);
3. order state shared a code path with market data, so a tick burst could delay an ack.

The rewrite enforces the separation at the crate level:

```
 ┌───────────────┐   raw frames   ┌──────────────┐   marks    ┌─────────────┐
 │  fv-feed      │───────────────▶│  fv-engine   │───────────▶│  fv-bridge  │
 │  WS + decode  │                │  Coalescer   │  30–60 Hz  │  (Tauri)    │
 └───────┬───────┘                └──────────────┘  deltas    └─────────────┘
         │ order frames (fast path, own channel)
         ▼
 ┌───────────────┐
 │ order state   │  ← never blocked by market data
 └───────────────┘

 ┌───────────────┐   reads    ┌──────────────┐
 │  fv-master    │───────────▶│  fv-store    │──▶ MongoDB (local cache of record)
 │  instruments  │            │  trackers    │
 └───────────────┘            │  snapshots   │
                              │  order journal│
                              └──────────────┘
```

## 2. Crate responsibilities

| Crate | Owns | May depend on | Must not |
|---|---|---|---|
| `fv-core` | `Price` (i64, 6 dp), `SymbolKey`, `Quote`, `CoreError` | serde, thiserror | touch I/O, network, DB |
| `fv-feed` | frame classification, decoding, `QuoteBook` merge, subscription planning, replay, WS transport | `fv-core` | own UI state, know about Mongo |
| `fv-master` | `Instrument`, in-memory index/search/option-chain, Mongo loader | `fv-core` | decode market data |
| `fv-store` | cache of record: trackers, snapshots, order journal | `fv-core` | decode market data, own display models |
| `fv-engine` | `Coalescer`, `TokenBucket` | `fv-core`, `fv-feed` | perform I/O |
| `fv-cli` | harness / measurement | all of the above | contain business rules |
| `fv-bridge` *(planned)* | Tauri commands + batched delta events | `fv-engine` | parse frames |
| `src-tauri` *(planned)* | window/shell lifecycle, keychain, updater | `fv-bridge` | hold domain state |

"Must not" is deliberate: it is what prevents the dispatcher coupling from reappearing.

## 3. Invariants (each has a test)

### 3.1 Money
`Price` is `i64` scaled by `1e6`. Strict parse for user input, `parse_rounded` for feed floats.
Formatting rounds half away from zero. **No `f64` arithmetic anywhere in the domain.**

Why: the legacy `PriceConvertor` did paisa↔rupee float round-trips; 2 dp prices survive, but
implied volatility (6 dp) and accumulated amounts do not.

### 3.2 Symbol identity
`SymbolKey` is `(exchange, token)` where **token is a string**. `nfo|158614` normalises to
`NFO|158614`; `BSE|000123` keeps its leading zeros. `new()` rejects empty parts, so an invalid key
cannot be constructed (that is why `SymbolKey` intentionally has no `Default`).

### 3.3 Diff merging
The venue sends one full snapshot (`t:"tk"`) per subscription, then only changed fields (`t:"tf"`).

- `tk` **resets** the quote: previously-known fields not present in the snapshot are cleared, so a
  resubscribe cannot leave a stale high/low behind.
- `tf` merges field-wise; absent means "unchanged", never "zero".
- A `tf` before any `tk` is tolerated (entry created), because ordering across a reconnect is not
  guaranteed by the client's view.
- Frames without a resolvable symbol, and non-quote frames, are `Applied::Ignored`.

### 3.4 Order-frame isolation
`peek_type()` reads `"t"` from the first 96 bytes without allocating (measured 0.01 µs). `om` frames
are parsed and dispatched as order events **before** any market-data work. A malformed neighbour
cannot poison the order path (`route_batch` isolates errors per frame).

Unknown frame types are **validated as JSON and preserved** as `Frame::Unknown`, so corruption is an
error while a venue-side addition is observable rather than silently dropped.

### 3.5 Bounded work per frame
`Coalescer` emits at most `max_per_frame` quotes per interval and carries the remainder. Marks
collapse per instrument, so 1,000 ticks on one scrip produce one delta. Output order is deterministic
(`BTreeSet`), which makes batches diffable and tests stable.

A key marked dirty with no quote yet is dropped from the dirty set rather than retried forever.

### 3.6 Explicit rate budgets
`TokenBucket` models the documented venue limits (~10 order requests/s; 1 WS connection per session).
The app self-limits instead of collecting `Rate_Limited` errors. `wait_time()` exists so callers can
back off precisely rather than sleeping blindly.

### 3.7 Cache of record
`needs_backfill()` is the single rule that prevents re-querying the venue:

| Tracker state | Result |
|---|---|
| no row | fetch |
| same date + found | skip |
| different date | fetch |
| found = false, retries < 3 | fetch |
| found = false, retries ≥ 3 | **give up** (stop hammering for a non-existent symbol) |

Live ticks are **never** sourced from Mongo. Mongo answers: history, master, reference data,
and "have I already done this?".

## 4. Cross-platform rules

| Rule | Reason |
|---|---|
| No `cfg(windows)` outside the optional Excel crate | core must compile identically everywhere |
| `directories` for paths, never string concat of `%APPDATA%` | `~/Library/Application Support`, `$XDG_DATA_HOME` |
| `keyring` for secrets | Credential Manager / Keychain / Secret Service |
| `rustls` for all TLS | identical validation and cipher behaviour per OS |
| ship fonts | Linux may lack a UI font; never assume `Segoe UI` |
| UTC at the boundary | never rely on host timezone |
| explicit little-endian if a binary format is added | 32/64-bit and endian-safe |
| no `usize` in persisted formats | portability |

CI matrix: `windows-latest`, `macos-latest`, `ubuntu-latest` ×
`cargo test --workspace`, `clippy -D warnings`, `fmt --check`.
All Mongo-dependent behaviour has an in-memory equivalent, so the suite passes with no database.

## 5. Data flow: startup

```
1. Store::from_env()                  → Mongo if reachable, else in-memory (warn, continue)
2. MongoConfig::from_env()            → uri / db / collection
3. load_master()                      → 182,901 docs → MemoryMaster (2.7 s measured)
4. UI subscribes to an initial watch set
5. WatchSet::reconcile(target)         → minimal subscribe plan
6. run_forever(...)                    → connect, handshake, subscribe, read loop
7. Coalescer.flush_due() @ 30–60 Hz    → DeltaBatch → Tauri event
```

Shutdown is explicit: `Store::shutdown()` joins the Mongo driver workers. Without it the driver
panics at runtime teardown (observed and fixed — see `README.md`).

## 6. Data flow: order event

```
socket → peek_kind == OrderUpdate
      → parse::<OrderUpdate>()
      → order channel (dedicated task)
      → journal (Store::journal_order, idempotent on event_key)
      → UI event (not coalesced: order state is never delayed)
```

`event_key = noren_order_no|report_type|status|fill_id` makes reconnect replays idempotent while
still distinguishing multiple partial fills of one order.

## 7. Mongo collections used

See `fv-features.md` §7.9 for counts, shapes and index findings. Summary:

| Collection | Direction | Purpose |
|---|---|---|
| `FINVASIA_SYMBOL` | read | primary master (`token` as string) |
| `INSTRUMENT` | read (cross-ref) | Kite-shaped master (`instrument_token` numeric) |
| `SNAPSHOT_INTRADAY` | read/write | intraday OHLC/OI cache |
| `DATA_STORED_TRACKER` | read/write | backfill ledger |
| `FV_ORDERS` | write | order event journal (app-owned) |
| `EXPIRY_LOT_DETAILS`, `HOLIDAY_LIST`, `SCRIP` | read | reference data |

The two master shapes are normalised through one mapping function with type-tolerant field access
(Int32/Int64/Double/String/DateTime all accepted), because the same logical field has different
physical types in each collection.

## 8. What is deliberately absent

- **No CozyMQ client.** Decision record DR-1: the official WS API. This removed a 5,485-field
  protobuf schema recovery and an access-control question.
- **No protobuf.** Documented JSON with a cheap discriminator beats an undocumented binary protocol
  that only one vendor implementation understands.
- **No UI in Phase 0/1.** Adding a webview now would invite the exact "parse on the UI thread" mistake
  this architecture exists to prevent.
