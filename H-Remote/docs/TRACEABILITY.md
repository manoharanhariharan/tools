# Feature traceability — Phase 0

Status legend: **Done** (implemented + verified), **Infra** (implemented; consumer
ships in the noted phase), **Deferred** (explicitly assigned to a later phase).

| ID | Status | Implementation | Verification |
|---|---|---|---|
| P0-01 | Done | `src-tauri/tauri.conf.json`, README stack | `cargo check`, CI matrix |
| P0-02 | Done (Tauri bundles) | `bundle` config, README Build | `docs/PHASE-0.md` deviation 1 |
| P0-03 | Done | Svelte runes stores + typed IPC (`ui/src/lib/state`, `ipc`) | Vitest store tests |
| P0-04 | Done | `src-tauri/src/state.rs` composition root | `state_initialises_from_a_temporary_profile` |
| P0-05 | Done | `hr-core/src/registry.rs` (`inventory`) | registry tests |
| P0-06 | Done | `paths.rs`, `hr-crypto` salt init | salt/path tests |
| P0-07 | Done | `main.rs` logging → panic hook first | logging integration test |
| P0-08 | Done | `paths.rs` marker/env detection | 6 profile tests |
| P0-09 | Done | `preflight.rs` | 4 preflight tests |
| P0-10 | Done | `GuidanceWizard.svelte` | wizard test + E2E |
| P0-11 | Done (settings) | `legacy.rs` detect/import | legacy tests; servers → P9 |
| P0-12 | Done | `hr-i18n` negotiation (`sys-locale`) | negotiation tests |
| P0-13 | Done | `assets.rs`, stores, `state.rs` | asset tests; startup state test |
| P0-14 | Done (global cache P1) | theme/hotkey/SQLite in `main.rs` | state test; DB tests |
| P0-15 | Done | `hr-platform::cli`, single-instance plugin | CLI matrix |
| P0-16 | Done | `start_minimized` in state/main | CLI + window restore |
| P0-17 | Done | `config.rs` | config tests |
| P0-18 | Done | `assets.rs` | asset tests |
| P0-19 | Done | `assets.rs` (`Protocols/*.json`) | asset tests |
| P0-20 | Done | `locality.rs` | locality tests |
| P0-21 | Done | `TagsLocality` | tag tests |
| P0-22 | Done | `ConnectionRecords` | record ordering tests |
| P0-23 | Done | `atomic.rs` | atomic/corruption tests |
| P0-24 | Done | `store.rs` watch + debounce | debounce/watcher tests |
| P0-25 | Done | wizard steps | wizard test + E2E |
| P0-26 | Done | wizard profile + marker command | wizard test; marker tests |
| P0-27 | Done | wizard language | wizard test |
| P0-28 | Done | wizard theme with preview | wizard test |
| P0-29 | Done | wizard startup toggle | wizard test |
| P0-30 | Done | wizard shortcut + integrations | integrations tests; E2E |
| P0-31 | Done | wizard confirm toggle | wizard test |
| P0-32 | Done | legacy settings import step | legacy import test |
| P0-33 | Done | `complete_guidance` marker/shortcut/autostart | command wiring + tests |
| P0-34 | Done | `I18n::set_locale`, `i18n.setLocale` | runtime switch tests |
| P0-35 | Done | `locales/*.ftl` (14) | catalog count test |
| P0-36 | Done | missing-key tracking in Rust/JS | missing-key tests |
| P0-37 | Done | config watcher; runes labels | i18n tests; watcher code |
| P0-38 | Done | `hr-theme` ten presets (nine reference + High Contrast) | preset tests |
| P0-39 | Done | `AppearanceSettings.svelte` | theme tests + E2E |
| P0-40 | Done | `theme/apply.ts` CSS variables | theme tests |
| P0-41 | Done | `TitleBar.svelte`, decorations false | shell E2E; manual drag |
| P0-42 | Done | `display.rs`, `window.rs` | placement tests |
| P0-43 | Done | locality + restore/save | placement + locality tests |
| P0-44 | Done | `Modal.svelte`, dialog host | dialog tests |
| P0-45 | Done | `busy.svelte.ts`, `BusyOverlay.svelte` | ref-count tests |
| P0-46 | Done | dialog store + `DialogHost` | dialog tests |
| P0-47 | Done (input/password) | `DialogHost`; pickers → P2/P8 | dialog tests |
| P0-48 | Done | `hotkey.rs` + plugin | hotkey tests |
| P0-49 | Done | `window::flash` command | code path; manual check |
| P0-50 | Done | `logging.rs` | logging integration test |
| P0-51 | Done (local) | `tracing` abstraction, local crash reports | logging/crash tests |
| P0-52 | Done | `ErrorReportDialog.svelte` | app error test + E2E |
| P0-53 | Done | panic hook + JS error/rejection handlers | app error test |
| P0-54 | Done | `hr-platform::memory` | trim test; startup call |
| P0-55 | Done | single-instance plugin | CLI tests + wiring |
| P0-56 | Done | `parse_cli_args` | full CLI matrix |
| P0-57 | Done | pending queue + `ui_ready` | activation code path |
| P0-58 | Removed | tray icon removed on request | no tray process or tests |
| P0-59 | Removed | recent tray submenu removed | launcher recents remain |
| P0-60 | Removed | tray check item removed | no tray menu |
| P0-61 | Done | reset window setting/command | placement tests |
| P0-62 | Removed | tray feedback/about/exit removed | About/Settings remain |
| P0-63 | Done | config watcher | watcher + i18n tests |
| P0-64 | Done | language setting | E2E language test |
| P0-65 | Done | autostart plugin | integration wiring |
| P0-66 | Removed | close always exits (no tray) | window close event |
| P0-67 | Done (persisted; used P4) | general setting | settings E2E/persistence |
| P0-68 | Infra (P4) | session-icon setting | settings persistence |
| P0-69 | Infra (P4) | tab header settings | settings persistence |
| P0-70 | Removed | recent-in-tray setting removed | launcher recents remain |
| P0-71 | Infra (P2) | note-in-list setting | settings persistence |
| P0-72 | Done | log level + reload layer | logging integration test |
| P0-73 | Infra (P2/P4) | copy-port setting | settings persistence |
| P0-74 | Infra (P4) | close-left setting | settings persistence |
| P0-75 | Infra (P3) | focus-local setting | settings persistence |
| P0-76 | Infra (P9) | update-check setting | settings persistence |
| P0-77 | Done | open log folder command/settings | code path |
| P0-78 | Done | immediate flush in `update_config` | config/store tests |

## Verification commands

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
npm run check
npm test
npm run test:e2e
npm run check:i18n
npm run check:traceability
```

The traceability script fails if a P0 ID from the specification is missing here.
