# Test strategy

## 1. Layers

| Layer | Tool | Scope |
|---|---|---|
| Rust unit | `cargo test` in-crate | parsers, schemas, crypto, placement math, stores, themes |
| Rust integration | `crates/*/tests`, `src-tauri` tests | logging process, SQLite lifecycle, app state bootstrap |
| UI unit/component | Vitest + Testing Library + jsdom | stores, dialogs, theme/i18n, wizard, full App shell |
| Browser E2E | Playwright + Vite + mock backend | first run, catalog search/views, inline edit, editor, session connect/reconnect, persistence, theme, language, error report |
| Static checks | scripts | i18n key existence, Phase 0/1/2/3 traceability |

The UI and E2E suites use the same typed IPC contract as the desktop app. The
mock backend (`ui/src/lib/ipc/mock.ts`) persists configuration to
`localStorage` so reload/persistence flows are real browser flows, not component
stubs.

## 2. Commands

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check

npm run check              # svelte-check (0 errors required)
npm test                   # Vitest, 97 tests
npm run test:e2e           # Playwright, 16 tests
npm run check:i18n         # literal keys exist in en-US
npm run check:traceability # all P0–P8 IDs documented
```

Single test examples:

```bash
cargo test -p hr-config store::tests::drop_flushes_pending_updates
cargo test -p hr-crypto sentinel_detects_the_wrong_key
npx vitest run --config ui/vitest.config.ts src/lib/i18n
npx playwright test --config ui/playwright.config.ts -g "language"
```

## 3. What Phases 0–8 cover

Rust (361 tests):

- **hr-core** — error codes/JSON, ULID monotonicity, registry ordering,
  duplicates, OS availability.
- **hr-domain** — protocol JSON round trips (all 11 variants), validation,
  subtitles/connection ids, tag/folder cascades, credential hashes/inheritance,
  LocalApp macros/templates, bounded icon cache, registry capabilities, search
  providers/highlights/natural sort/folder tree, `.rdp` parse/serialize,
  JSON/CSV/RDP transfer, icon ICO merging.
- **hr-storage** — schema v3/migration, encryption codec and sentinel, wrong-key
  rejection, protocol round trips, CRUD/pagination, duplicate credential names,
  read-only credentials, cache stamps, dialect SQL, raw-DB secret assertions,
  folder CRUD with server path rewrites and cycle prevention, legacy DB import,
  credential deletion cascade, additional-source loading/read-only credentials,
  cache-on-failure and encrypted source-password persistence/reload.
- **hr-config** — profile priority/markers/conflicts, preflight probes,
  atomic writes, corrupt quarantine, debounce coalescing, flush-on-drop, watcher
  delivery, saver thread termination, config clamping/forward compatibility,
  locality/tag/connection records, legacy detection/import, auxiliary assets.
- **hr-crypto** — round trip, randomised ciphertext, purpose/salt binding,
  tamper detection, sentinel wrong-key, salt persistence, key-file passphrase,
  malformed input.
- **hr-i18n** — 14 catalogs, negotiation (script/language/exact), scaffold
  fallback, interpolation, missing keys, runtime switching, surface key presence.
- **hr-theme** — ten presets (incl. High Contrast), palette validity, ARGB→RGBA,
  custom validation, font clamps, CSS variables, JSON round trip.
- **hr-observability** — Markdown report content, redaction, truncation,
  unique files, log directory creation, runtime log-level reload + flush.
- **hr-platform** — CLI matrix, hotkey parsing/conflicts/reserved keys, monitor
  placement/signature, shortcut content/creation, autostart specs, capability
  matrix, TCP reachability/latency, working-set trim, DPAPI `.rdp` blobs,
  script execution/environment, update version/manifest rules, Windows secondary
  verification.
- **hr-session** — RDP/VNC/process plans, lifecycle state machine, reconnect,
  manager, bounded frames, seqlock shared memory, IronRDP adapter, VNC RFB
  framebuffer/input mapping, runner-based process launch, inline private-key temp
  files, IronRDP fast-path key/pointer/wheel mapping, RemoteApp `.rdp`,
  `mstsc` fallback and DPAPI. Integration:
  `tests/session_flow.rs` plus an ignored live-host smoke test.
- **hr-files** — SFTP/FTP connection, listing, rename/delete/mkdir, streaming
  upload/download with bounded copy, progress and cancellation.
- **hremote** — profile marker replacement, tray recent rows, state bootstrap
  with a temp profile and launch counters, source/rating guards, update status,
  session-script environment and pre-connect aborts, match-cache keys, shutdown.

UI (97 tests): busy reference counting, dialog resolution/replacement, cleanup
registry, listener detachment, modal focus restore and Escape, password reveal,
theme conversion/application, i18n fallback/interpolation, wizard navigation and
payload, App boot/navigation/language/error/config-event flows, catalog
load/search/selection/editor/bulk/folder/tag/order/export/import/dispose flows,
session connect/events/reconnect/frame polling/timer teardown, tab
close/reorder/grid/layout persistence, snap layouts, VNC input mapping, launcher
filtering/highlighting/keyboard/prefill/quick-connect/history, file browser
listing/navigation/sorting/selection, credential vault listing/creation,
data-source grouping/filtering, the Data Sources settings flow, the
update/rating/legacy dialogs, matcher defaults, refresh coalescing, RDP
input dispatch and display overrides, frame-refresh coalescing, retained frames on
failed polls, remote cursor-shape events, audio redirect fields.

E2E (16): guidance completion, catalog search/tag filters/view switching, inline
address edit, side editor add, session connect/frames/cancel/reconnect/close,
multi-tab grid/layout/keyboard/tab-drag/full-screen flow, launcher
search/prefill/quick-connect,
SFTP browser navigation, legacy JSON import, credential vault creation,
read-only data source + grouped catalog + read-only editor, manual update check +
dialog dismissal, settings persistence across reload, live theme, live language,
unhandled-error report.

## 4. Adding tests

- A new Rust feature needs a unit test for the pure logic and, if it touches
  files/DB/threads, an integration or in-crate test using `tempfile`.
- A new UI feature needs a store/component test; cross-feature flows add an E2E
  test against the mock backend.
- New translation keys must be added to `locales/en-US.ftl` with dots replaced by
  dashes (`scripts/normalize-fluent-ids.mjs` is idempotent).
- New Phase 0/1/2/3/4/5/6/7/8/9/10/11 features must be added to the matching traceability
  document, otherwise `check:traceability` fails.
- Cleanup assertions are mandatory for listeners, threads, files, pools and
  caches (see `docs/MEMORY.md`).

## 5. CI

`.github/workflows/ci.yml` runs:

1. **rust** on Windows/Linux/macOS: fmt, clippy `-D warnings`, workspace tests.
2. **ui**: `svelte-check`, Vitest, production build, i18n check.
3. **e2e**: Playwright Chromium against the mock backend.
4. **traceability**: Phase 0/1/2/3/4/5/6/7/8/9/10/11 ID coverage.

Release builds are produced by `.github/workflows/release.yml` on `v*` tags; see
`docs/BUILD-MATRIX.md` and `docs/RELEASING.md`.

> The workflow lives in `H-Remote/.github/workflows/ci.yml`. In the current
> monorepo GitHub only executes workflows from the repository root, so this file
> runs as-is once H-Remote is extracted into its own repository (or mirrored to
> the root workflow). All jobs are plain commands you can run locally.

Linux installs the Tauri WebKit/GTK development packages before Rust builds.
Playwright is browser-only (no tauri-driver) so it runs on all CI hosts; a
tauri-driver smoke job can be added once the app is packaged (Phase 11).
