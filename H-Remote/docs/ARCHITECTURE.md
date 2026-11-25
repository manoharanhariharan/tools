# Architecture

## 1. Shape of the system

```
┌────────────────────────────────────────────────────────────────────┐
│ Svelte 5 UI (ui/)                                                  │
│  features/  components/  state/ (runes)  ipc/ (typed contracts)    │
└──────────────────────────────┬─────────────────────────────────────┘
                               │ invoke() / events (typed IPC)
┌──────────────────────────────▼─────────────────────────────────────┐
│ src-tauri (composition root)                                       │
│  commands  window  hotkey  activation  integrations  state       │
└───────┬──────────┬──────────┬──────────┬──────────┬────────────────┘
        │          │          │          │          │
   hr-config   hr-crypto  hr-storage  hr-i18n   hr-observability
        │          │          │          │          │
   hr-theme    hr-platform ──┘      hr-domain ─────┘
        │                              │
   hr-core (errors, IDs, protocol registry)
```

Dependencies point downward only. Crates never depend on Tauri or Svelte, which
keeps them unit-testable and reusable by the future session-host process.

## 2. Crates

| Crate | Responsibilities | Key invariants |
|---|---|---|
| `hr-core` | `AppError`/`IpcError`, ULIDs, `AppInfo`, protocol registry (`inventory`) | UI-facing error codes are stable; registry ids are unique |
| `hr-config` | profile resolution, JSON schemas, atomic writes, debounced stores, locality, legacy import, auxiliary assets | every numeric field is clamped; corrupt files are quarantined; one saver thread per file |
| `hr-crypto` | installation salt, OS keyring master key, Argon2id key file, AES-256-GCM vault, sentinel | plaintext secrets never hit disk; AAD binds purpose + salt |
| `hr-domain` | protocol-typed servers, credentials, tags/folders, LocalApp arguments/macros, icon cache, validation, registry | every Appendix A field round-trips; secrets are redacted in `Debug` |
| `hr-i18n` | Fluent catalogs, negotiation, runtime switch, missing-key tracking | all 14 catalogs parse; fallback to en-US |
| `hr-theme` | ten presets (incl. High Contrast), custom validation, CSS variables | font size clamped 10-20; ARGB→RGBA conversion |
| `hr-observability` | tracing setup + reload, rotating logs, crash reports, startup trace | log worker flushes on drop; reports redact secrets |
| `hr-platform` | capability detection, CLI parsing, hotkey parsing, shortcuts, autostart specs, monitor placement, TCP reachability/latency, script execution/environment, update version/manifest rules, Windows credential verification, working-set trim | pure functions for everything testable |
| `hr-storage` | SQLite bootstrap, schema v3, encrypted JSON codec, repository CRUD, caches, dialect builders, read-only source manager (SQLite/MySQL/PostgreSQL) | pool closed deterministically; wrong key fails explicitly; secondary pools are single-connection, 5 s timeouts and never written |
| `hr-session` | RDP plan, lifecycle state machine, IronRDP adapter, `mstsc` fallback, bounded frame ring | frames are bounded and tear-free; every session ends in a terminal state; secrets never logged |
| `hr-files` | SFTP/FTP sessions, remote listing, transfers, progress/cancellation | transfers are bounded and cancellable; connections close deterministically |
| `hremote` (`src-tauri`) | lifecycle, commands, window chrome, integrations | one `AppState`, no service construction inside commands |

## 3. Composition root and state

`AppState` is created once in `main` before Tauri builds. All stores are
`Arc<JsonStore<T>>`; the database and vault are owned by the state. Commands
receive `State<'_, AppState>` and never create duplicates.

```rust
AppState {
  paths, config, locality, tags, connection_records, list_view, tree_view,
  i18n, capabilities, startup, preflight, db, repository, sources, assets, vault,
  log_guard, sessions, pending_actions, data_ready, start_minimized,
  first_run, ...
}
```

Lifecycle:

1. Install the rustls ring provider once (both `ring` and `aws-lc-rs` features are
   present in the dependency graph, so rustls requires an explicit default before
   the first HTTPS client).
2. Resolve executable directory and profile (`paths`).
3. Initialise logging, then the panic hook (`P0-07`).
4. Write-permission preflight (fail with a native error dialog).
5. Open config/locality stores (sanitising on read).
6. Load catalogs, SQLite, salt/vault, auxiliary assets, then open and verify the
   catalog repository. If the vault is ready, also load the read-only source
   manager from `hremote.dataSources.json`. Launch counters are incremented once.
7. Parse CLI args and queue them (not processed until `ui_ready`).
8. Build Tauri, restore the main window, register the hotkey, and start the
   source watcher (initial load, periodic health probe, reconnect) and the
   update watcher (periodic manifest check).
9. On exit: stop the watchers, drop the repository, close every source pool, flush
   every store, close the local pool, drop the vault, stop logging.

## 4. Persistence model

- **Global config** - `hremote.json`, schema-versioned, unknown top-level fields
  preserved. Missing fields use defaults.
- **Locality** — `.locality/.locality.json`, `.tags.json`,
  `.connection_records.json`, `.list_view.json`, `.tree_view.json`.
- **Atomic writes** — temp file in the target directory → `fsync` → rename,
  with bounded retries for Windows sharing violations.
- **SQLite** — schema v3: `Servers(Id, Protocol, ClassVersion, Json, UpdatedAt)`,
  `Credentials(Id, Name, Hash, Json, UpdatedAt)`, `Configs(Key, Value, UpdatedAt)`,
  `Folders(Id, Path, UpdatedAt)` (JSON array path, unique) and
  `TableMeta(TableName, UpdatedAt, RowCount)`. Schema upgrades run in one
  transaction chain from any older version.
- **Encryption** — secrets are replaced inside the JSON envelope with
  `{"$enc":"v1:..."}`; the column remains readable for non-secret metadata.
  `Configs.EncryptionTest` verifies the key; a mismatch yields
  `ErrorCode::EncryptionKey`.
- **Caches** — server/credential snapshots are `Arc<Vec<_>>` keyed by table
  stamp. Writes invalidate; external writers bump `TableMeta.UpdatedAt`.
- **Search** — `catalog_view` caches match hits for 24 h in a bounded 64-entry
  map keyed by query, providers, source and catalog revision; every local server
  write/import/source refresh bumps the revision. Large catalogs (>300 servers)
  run matching on a blocking task when parallel matching is enabled.
- **Read-only sources** — `SourceManager` owns one optional pool per additional
  source (SQLite opened `read_only`, MySQL/PostgreSQL single-connection) plus a
  cached server/credential snapshot and `SourceStatus`. The catalog merges local
  and remote snapshots (local wins on id collisions); writes are rejected for any
  server whose `sourceId` is not `local`. A background watcher probes (`SELECT 1`)
  and reconnects on the configured cadence, and failures keep the cache.
- **Session automation** — `connect_server` allocates the session id, runs the
  pre-connect script synchronously (non-zero aborts), starts the plan with that
  id, and spawns a watcher that runs the post-disconnect script when the session
  reaches a terminal state. Both scripts receive `SESSION_ID` and `SERVER_*`
  environment variables.
- **Runner keys** — inline PEM material becomes a per-launch temp file expanded as
  `{privateKey}`; the process backend writes it before spawn and deletes it on
  exit, cancellation or failure.
- **Custom protocols** — `Server::Custom` resolves an external runner by
  `customProtocolName`; `extraFields` strings become `{field}` macros and
  `CUSTOM_*` environment variables. Built-in protocol names are rejected by
  validation and again at launch.
- **Corruption policy** — parse failure renames the file to
  `.corrupt-<millis>` and returns defaults; the path is surfaced in the UI.
- **Mixed-key policy** — a row-level secret that cannot be decrypted is cleared
  for display (never written back automatically) and logged with its id.
  `Repository::open` refuses to stamp a new sentinel over existing rows. The
  explicit `repair_catalog_secrets` command re-encrypts readable secrets, clears
  unreadable ones and replaces the sentinel, then reopens the repository.
- **Debounce** — updates go to a single worker thread; the latest snapshot wins.
  `flush()` writes immediately and waits; `Drop` flushes and joins the worker.
- **Watchers** — `tokio::sync::watch` publishes persisted snapshots. The app owns
  exactly one watcher task that emits `config:changed`.

## 4a. Session pipeline

```
command (connect_server / quick_connect)
  → SessionPlan (RDP, VNC, Process)
  → SessionManager (snapshot + broadcast events + retry policy)
  → Backend::run
      ├─ IronRdpBackend: dedicated thread → SharedFrameRing
      ├─ VncBackend: vnc-rs → RGBA framebuffer → SharedFrameRing
      ├─ ProcessBackend: runner/mstsc/local exe → exit watch
      ├─ FileManager: SFTP/FTP browser → transfer queue
      └─ MstscBackend: generated .rdp → mstsc.exe
  → `session:event` (state/frame/clipboard) → Svelte store
  → SessionWorkspace renders frames, process state or VNC input
```

`SharedFrameRing` is a single-slot seqlock: the sequence is odd while writing,
readers retry, and the frame capacity is bounded at 4096×4096×4 bytes. The ring
name is `Local\HRemoteFrame-<session id>`; non-Windows hosts use the bounded heap
store. The shared `sessions` window is declared hidden in `tauri.conf.json`
(like the launcher), loads the same SPA with `index.html?window=sessions`, and
polls only its visible panes (one in focus mode, all tabs in grid mode). Every
connection shows or focuses that window, so the catalog remains a catalog and
session tabs/grids always render in a window built for them. The window is
never created from an IPC command, avoiding the white-window/hang caused by
building a webview while the main thread was still dispatching the command.
Closing it hides the webview instead of destroying it. PNG frame encoding runs
on the blocking pool, never on Tauri's main thread, so a busy desktop cannot
prevent the session window from painting or responding.

The launcher is a second always-on-top webview (`launcher`) loading
`index.html?window=launcher`. The hotkey toggles it; it loads the catalog and
history in parallel, filters locally, starts sessions through the same manager,
and hides itself. Quick connect records deduplicated history in
`.connection_records.json` and optionally saves a normal server.

## 5. IPC contract
`ui/src/lib/ipc/contracts.ts` mirrors the Rust DTOs. Field names are camelCase
on both sides (`#[serde(rename_all = "camelCase")]`). Errors cross as
`{ code, message, retryable }`; the UI has a single `CommandError` normaliser.

Events:

| Event | Payload | Consumer |
|---|---|---|
| `config:changed` | `Configuration` | theme/locale state |
| `ui:navigate` | `"settings" \| "about"` | route |
| `activation:connect` | `CliAction` | catalog/launcher (Phases 2/5) |
| `cli:errors` | `string[]` | notice bar |
| `integration:warnings` | `string[]` | notice bar |
| `hotkey:error` | `string` | notice bar |
| `app:ready`, `app:bootstrapped` | `()` | diagnostics |
| `launcher:toggle` | `()` | Phase 5 |

The mock backend (`ui/src/lib/ipc/mock.ts`) implements the same command names
for browser development, Vitest and Playwright.

## 6. UI modules

```
ui/src/lib/
  components/      Button, Icon, Toggle, TextField, SelectField, Modal,
                   TitleBar, Sidebar, BusyOverlay, DialogHost, NoticeBar
  features/        guidance/, settings/, about/, catalog/, errors/
  state/           app.svelte.ts, busy.svelte.ts, dialogs.svelte.ts
  ipc/             contracts.ts, client.ts, events.ts, mock.ts, runtime.ts
  i18n/            catalogs.ts, index.svelte.ts
  theme/           apply.ts
```

Rules:

- Presentational components take props and emit callbacks; they do not invoke
  commands or read the app store.
- Feature containers own data loading and state transitions.
- Cross-feature state lives only in `state/`.
- Every listener is registered through `CleanupRegistry`; see `docs/MEMORY.md`.

## 7. Theming and window chrome

The Rust theme crate resolves a persisted `ThemeConfig` and returns validated
colours. The UI applies them as CSS custom properties on `<html>` and sets
`data-theme` / `data-color-scheme`. Window chrome is custom (`decorations:
false`) with a drag region and accessible minimize/maximize/close buttons.

Monitor-aware placement is pure logic in `hr-platform::display`: stored geometry
is validated against the current monitor list, clamped, and recentered if the
display disappeared. The Tauri layer only converts API values.

## 8. Future: embedded sessions (Phase 3/4)

The chosen design streams frames from a Rust session host into a WebGL canvas:

```
IronRDP/VNC client (session host process)
   → decoded BGRA frame → fixed-size shared-memory ring buffer
   → Tauri IPC descriptor → WebGL texture upload
   ← input events (normalised coordinates, key codes) via IPC
```

The ring buffer has a fixed number of slots and explicit producer/consumer
handles; a dropped session closes the mapping and reaps the process. This avoids
HWND airspace/embedding problems and makes tabs/grid/snap layouts pure DOM. The
contract is intentionally not implemented in Phase 0; no placeholder socket or
thread is created early.

## 9. Startup trace

`StartupTrace` records named checkpoints (`process`, `logging`, `preflight`,
`configuration`, `localisation`, `database`, `vault`, `state`, `window`,
`ready`) and exposes them through `bootstrap`/`startup_checkpoints`. They appear
in Settings → Data and the About page, and are included in diagnostics.
