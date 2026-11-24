# Phase 0

> Historical note: the tray icon and minimize-to-tray setting were removed on
> request. The catalog window now exits on close; connections open in the shared
> session window, and the launcher/settings/About paths remain. — Foundation & application shell

**Status: complete (2026-11-22).** All 78 P0 feature IDs have an implementation
and a verification path. Items whose *consumer* legitimately arrives in a later
phase (tab buttons, session icon, copy-port action, update prompts) persist the
setting now and are wired when the relevant feature ships; this is called out
per item in [`TRACEABILITY.md`](TRACEABILITY.md).

## What ships

### Lifecycle

- Profile detection with `Portable`, `AppData`, marker overrides and env overrides.
- Pre-flight write probe for every file/directory before the UI starts.
- Startup trace from process start to first window.
- Logging first, then panic hook, then everything else.
- Single-instance named-pipe activation with argument forwarding and deferred
  processing until the UI signals readiness.
- Start-minimised and tray-only operation.

### Persistence

- `hremote.json` (settings, launcher, matcher list, theme, engagement, DB periods).
- `.locality/.locality.json`, `.tags.json`, `.connection_records.json`,
  `.list_view.json`, `.tree_view.json`.
- `hremote.dataSources.json` and `Protocols/*.json` are discovered, parsed and
  corrupt files quarantined.
- Atomic JSON writes, corruption quarantine, debounced saver thread, flush on
  exit, watcher events.
- SQLite bootstrap with WAL, a meta/migration table and real writability probing.
- Installation salt + OS-keyring/Argon2id vault + AES-256-GCM + encryption
  sentinel.

### Localisation and themes

- 14 Fluent catalogs: English complete, 13 scaffolded with English fallback.
- System-locale negotiation, runtime switch, missing-key logging.
- Tray menu rebuilds on language change; UI labels react without reload.
- Nine authentic 1Remote palettes, custom colours, live font-size clamp 10–20,
  CSS-variable application and dark/light detection.

### Desktop shell

- Custom window chrome: drag region, double-click maximize, min/max/close,
  keyboard/ARIA labels.
- Monitor-aware placement restore with clamping, monitor-name preference,
  screen-layout signature and DPI scale persistence.
- Tray: show, recent submenu (up to 20), toggle recents, reset window, log folder,
  feedback, about, exit; menu rebuild is leak-safe.
- Global hotkey registration, validation, reserved-combination detection and
  conflict reporting.
- Native error dialog for fatal startup failures; recoverable failures stay in
  the UI.
- Working-set trim after startup and when hiding to tray.

### UI

- First-run guidance wizard (intro, profile, language, theme, startup, shortcut,
  session confirmation, optional legacy migration, summary) with live theme
  preview and profile-lock handling.
- Settings: General, Appearance, Launcher, Data, About; persisted immediately.
- Reusable modal/dialog/mask/busy primitives; ref-counted busy overlay; message,
  input and password dialogs with focus trap and focus restore.
- Error report dialog with copy, save-to-logs, open-logs and environment summary.
- Catalog placeholder explains the Phase 1 boundary instead of pretending to
  have data.
- Empty/loading/fatal states; reduced-motion support; keyboard focus styles.

### Platform integration

- Desktop shortcut per OS (`.lnk` / `.desktop` / `.command`).
- Autostart via the Tauri autostart plugin (registry / LaunchAgent / XDG).
- `--install` / `--uninstall` and granular shortcut/startup flags.
- PRemoteM/1Remote settings detection and best-effort import (language, theme,
  general behaviour). Server/credential migration is deferred to Phase 9, where
  the catalog schema exists; the wizard states this explicitly.

## Defaults

| Setting | Default |
|---|---|
| Profile mode | Portable when the executable directory is writable, else AppData |
| Language | System locale, English fallback |
| Theme | Dark |
| Close button | Minimise to tray |
| Confirm closing session | Off (matches reference default) |
| Log level | Warn |
| Launcher enabled | On, `Alt+M` (window arrives in Phase 5) |
| Recently used in tray | Off |
| Font size | 12 px |

## Known Phase 0 deviations

1. **Single-file executable.** Tauri ships installers/app bundles, not a literal
   single-file exe. A single-file distribution would require a self-extracting
   wrapper and is not part of the stable Tauri workflow; the document's P0-02 is
   mapped to Tauri bundling. Revisit in Phase 11 if the distribution requirement
   is strict.
2. **Windows-only features.** DPAPI is replaced by the keyring/vault design;
   Windows Hello and HWND embedding report capability `false` outside Windows.
3. **Legacy server import.** Detection and preference migration ship now; server
   import requires the Phase 1 model and lands with the Phase 9 importers.
4. **Icon picker / data-source selector dialogs.** Shared dialog primitives are
   ready; the actual pickers belong to Phase 2 and Phase 8.
5. **Session-level settings.** Tab header buttons, session icon, copy-port action,
   focus-on-leave and update prompts are persisted and editable now; their
   consumers ship with Phases 3, 4, 5 and 9.

## Acceptance evidence

| Acceptance criterion | Evidence |
|---|---|
| Clean first run, portable + AppData | `hr-config` path tests; `state_initialises_from_a_temporary_profile`; E2E wizard |
| Language/theme switch live | `hr-i18n` runtime tests; `apply.test.ts`; E2E language/theme tests |
| Second launch focuses first | `tauri-plugin-single-instance` wiring; CLI parsing matrix |
| Crash produces a report | `hr-observability` crash tests; E2E error-report test |
| Tray/autostart/shortcut flags | shortcut/autostart unit tests; integration commands |
| No unhandled exception on exit | store flush-on-drop test; `AppState::shutdown`; DB close test |
| All P0 IDs traceable | `scripts/check-traceability.mjs` |

## Verification snapshot

- Rust: 134 tests across crates (unit + integration), clippy `-D warnings`, rustfmt.
- UI: 30 Vitest tests, `svelte-check` 0 errors, production build.
- E2E: 5 Playwright tests (guidance, persistence, theme, language, error report).
- Static checks: i18n key existence, Phase 0 traceability.

## Next phase entry criteria

Phase 1 can begin when the Phase 0 review approves:

- Schema version 1 for `Servers`, `Credentials`, `Configs`.
- Domain models for every protocol in Appendix A with round-trip tests.
- Encryption sentinel stored/verified in SQLite.
- CRUD + cache invalidation + read-only source handling.
