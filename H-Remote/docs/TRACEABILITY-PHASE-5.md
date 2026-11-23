# Feature traceability — Phase 5

Status: **Done**, **Partial** (engine/action ships in a later phase), **Deferred**.

| ID | Status | Implementation | Verification |
|---|---|---|---|
| P5-01 | Done | `launcher.enabled` + `hotkey::apply` | settings launcher tests |
| P5-02 | Done | canonical hotkey config → global shortcut | `hr-platform` hotkey tests; settings |
| P5-03 | Done | `validate_hotkey` + registration conflict error | settings error path; hotkey registration |
| P5-04 | Done | `launcher` window (borderless, always-on-top, skip taskbar, hidden) | `tauri.conf.json`; `show/hide_launcher` |
| P5-05 | Done | focused search, incremental filter, 80 ms resize debounce | `LauncherPanel.test.ts`, E2E launcher |
| P5-06 | Done | recent-first ordering from `ConnectionRecords` | `record_connection`; launcher sort |
| P5-07 | Done | `HighlightedText` name/subtitle ranges | `filters servers and highlights matches` |
| P5-08 | Done | Up/Down, Enter, Tab, Esc | `LauncherPanel.test.ts`, E2E |
| P5-09 | Done | note + credential toggle from config | launcher panel |
| P5-10 | Partial | connect/new window/edit/duplicate/delete/copy/shortcut/export work; SFTP/alternate/runners announce Phase 6 | launcher actions; E2E |
| P5-11 | Done | auto-height 220–760, remembered placement, monitor-aware centre | `launcher_placement_is_centred_and_clamped` |
| P5-12 | Partial | refresh on show/config; selection clamped (not restored by id) | launcher refresh |
| P5-13 | Partial | protocol selector excludes RemoteApp; RDP enabled, others Phase 6 | quick-connect form |
| P5-14 | Done | Tab prefills protocol/address/port/username/password | `Tab into quick connect with prefill`, E2E |
| P5-15 | Done | deduplicated quick-connect history, newest first | `runs a quick connect and keeps history` |
| P5-16 | Partial | save-server toggle + first same-username credential match | `quick_connect` command |
| P5-17 | Done | session starts in the manager and appears as a tab via global events | E2E session workspace |
| P5-18 | Done | quick-connect height fixed/clamped with scrolling history | `resize_launcher` |

## Verification commands

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
npm run check && npm test
npm run test:e2e
npm run check:i18n
npm run check:traceability
```
