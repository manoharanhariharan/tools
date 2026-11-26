# H-Finvasia

Cross-platform rewrite of the Finvasia/Shoonya **NorenTrader (UFT)** dealer terminal, built on the
in-house stack: **Tauri 2 + Rust workspace + Svelte 5**.

> **Read [`fv-features.md`](fv-features.md) first.** It contains the reverse-engineering findings,
> the full feature inventory for migration, the bottleneck analysis, the transport decision, and the
> local-MongoDB data model. This file covers the code that exists today.

---

## Status

| Layer | State |
|---|---|
| `fv-core` — money, symbol keys, quotes | ✅ implemented + tested |
| `fv-feed` — WS framing, diff-merge, subscriptions, replay, live transport | ✅ implemented + tested |
| `fv-master` — Mongo-backed instrument master + in-process index | ✅ implemented + tested |
| `fv-store` — Mongo cache: trackers, snapshots, order journal | ✅ implemented + tested |
| `fv-engine` — tick coalescing, order-rate budget | ✅ implemented + tested |
| `fv-cli` — headless harness, ships as **`h-finvasia`** (`replay`, `bench`, `master`, `doctor`, `feed`) | ✅ working |
| `fv-bridge` + `src-tauri` + `ui/` | ⬜ not started (Phase 2) |
| `fv-market` — REST client | ⬜ not started |

This is the **Phase 0/1 foundation**: everything performance-critical and everything that talks to the
local database. There is deliberately no UI yet — the design goal is that when the UI arrives it
cannot re-introduce the old terminal's bottlenecks.

Verified on Windows 11 / Rust 1.99 against the live local MongoDB 4.4.2.

---

## Releases

`scripts/release.ps1` follows the repo standard (same shape as `h-compare`):

```powershell
.\scripts\release.ps1 -Message "feat(h-finvasia): ..."
.\scripts\release.ps1 -SkipTests -NoPush
.\scripts\release.ps1 -IncludeUntracked     # first release / new files
```

It runs the quality gates, commits source, builds `target/release/h-finvasia.exe`, copies it to
**`releases/`** as `h-finvasia-v<yy.MM.dd>.<n>.exe`, writes `BUILD.txt`, `SHA256SUMS.txt` and
`README.md`, then commits and pushes.

> The output folder is `releases/` by request; the sibling tools use `publish/`. Same contents, same
> committed-artifact convention.

Published binaries report their build date, which the script reads back to name the artifact:

```powershell
.\releases\h-finvasia-v26.11.26.1.exe --version
# h-finvasia 0.1.0 build 2026-11-26
```

---

## Quick start

```bash
cargo test --workspace          # 104 tests, no network, no database required
cargo run -p fv-cli -- doctor   # platform + feature + data-source self-check
```

### Query the instrument master (needs the local `STOCK` database)

```bash
cargo run --release -p fv-cli -- master stats
cargo run --release -p fv-cli -- master search RELIANCE --limit 5
cargo run --release -p fv-cli -- master chain NIFTY --expiry 2026-12-01
```

Measured on this machine against `STOCK.FINVASIA_SYMBOL`:

| Operation | Result |
|---|---|
| load 182,901 instruments | **2.69 s** (release) |
| distinct underlyings | 19,442 |
| `search RELIANCE` | 5 ranked hits, exact → prefix |
| `chain NIFTY 2026-12-01` | 550 legs, strike-ordered |

### Replay and benchmark a capture

```bash
cargo run --release -p fv-cli -- replay crates/fv-feed/tests/fixtures/sample_feed.jsonl
cargo run --release -p fv-cli -- bench  crates/fv-feed/tests/fixtures/sample_feed.jsonl --iters 20000
```

Measured throughput (release, x86-64):

| Stage | Rate | Per frame |
|---|---|---|
| classify (`t`-field fast path) | 105,235,464 ops/s | 0.01 µs |
| parse only | 266,451 ops/s | 3.75 µs |
| parse + merge | 225,849 ops/s | 4.43 µs (46.8 MB/s) |

At 5,000 frames/s the ingest path costs **~22 ms of CPU per second (~2 % of one core)**. The
order-frame fast path is effectively free, which is what protects order-ack latency during a tick
burst (see `fv-features.md` §7.6).

### Live feed (needs OAuth credentials)

```bash
export FV_ACCESS_TOKEN=...         # never pass secrets as CLI args in shell history
cargo run --release -p fv-cli -- feed \
  --uid AB1234 --actid AB1234 \
  --scrips NSE|2885,NFO|158614 \
  --frames 20
```

---

## Configuration

Everything is environment-driven so macOS/Linux developers need no code changes.

| Variable | Default | Meaning |
|---|---|---|
| `FV_MONGO_URI` | `mongodb://127.0.0.1:27017` | Cache/master database |
| `FV_MONGO_DB` | `STOCK` | Database name |
| `FV_MONGO_MASTER_COLLECTION` | `FINVASIA_SYMBOL` | Master collection |
| `FV_ACCESS_TOKEN` | — | OAuth access token for the live feed |
| `RUST_LOG` | `info` | `tracing` filter |

Collection names for the cache layer (`DATA_STORED_TRACKER`, `SNAPSHOT_INTRADAY`, `FV_ORDERS`) are
overridable in code via `MongoStore::with_collections`.

**No credentials are ever read from disk.** The transport takes a token from the caller; the Tauri
shell will source it from the OS keychain (`keyring`).

---

## Cross-platform

The core crates contain **zero** OS-specific code. Build and test on all three:

```bash
cargo test --workspace          # Windows / macOS / Linux
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

Determinism rules enforced by design (and by tests):

- money is `i64` fixed-point at 6 dp (`fv_core::Price`), never `f64` — no paisa↔rupee float drift;
- symbol identity is `EXCHANGE|TOKEN` with the **token as a string** (leading zeros survive);
- timestamps are UTC at the boundary; IST is a display concern;
- search and delta batches have deterministic ordering, so output never depends on hash iteration.

Windows-only features (the Excel XLL/RTD bridge) are planned behind `cfg(windows)` and excluded from
default builds. TLS is `rustls` everywhere for identical behaviour.

---

## Layout

```
Cargo.toml                 workspace
crates/
  fv-core/                 Price, SymbolKey, Quote, errors
  fv-feed/                 frame parsing, QuoteBook merge, subscriptions, replay, WS
    tests/fixtures/        sample_feed.jsonl (captured-shape frames)
  fv-master/               Instrument, MemoryMaster index, Mongo loader
  fv-store/                Store (memory | mongo), trackers, snapshots, order journal
  fv-engine/               Coalescer, TokenBucket
  fv-cli/                  harness -> binary `h-finvasia`
fv-features.md             reverse engineering + migration plan (source of truth)
docs/ARCHITECTURE.md       crate responsibilities, data flow, invariants
releases/                  committed portable executables (release.ps1 output)
scripts/release.ps1        quality gates -> build -> publish -> commit -> push
```

## Known limitations

- **No live-credential test yet.** `run_session` is exercised by unit tests (routing, backoff,
  handshake JSON) but not against the venue; that needs a real token.
- **Depth feed is not modelled.** Only documented frame types are typed; anything else surfaces as
  `Frame::Unknown` rather than being dropped, so the addition is observable.
- **Master load is 2.7 s**, not <1 s. The cost is Mongo cursor iteration + BSON→struct mapping;
  switching to `RawDocumentBuf` zero-copy reads or a cached binary snapshot is the next optimisation.
- **MongoDB 4.4.2 is EOL.** Driver 3.10 works today; the plan is 7.x. The repository layer avoids
  version-specific features so both work.
- **Local Mongo has dead indexes** (e.g. `INSTRUMENT` indexes on `tradingSymbol` while documents use
  `tradingsymbol`). See `fv-features.md` §7.9 for the verification-and-drop procedure.
