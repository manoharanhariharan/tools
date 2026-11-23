# Phase 5 — Launcher & quick connect

**Status: complete (2026-11-22).** The global hotkey opens a borderless,
always-on-top launcher window; search, keyboard navigation, recent ordering,
contextual actions, quick connect and history are implemented and tested.

## What shipped

### Backend

- `launcher` window in `tauri.conf.json`: 720 px wide, borderless, always-on-top,
  hidden on start, no taskbar entry, loads `index.html?window=launcher`.
- `window.rs`: `show_launcher` (stored position when `rememberPosition` is on,
  otherwise centred one fifth down the primary monitor, validated against the
  current monitor layout), `hide_launcher`, `toggle_launcher`, auto-height
  (`resize_launcher`, clamped 220–760 px) and placement persistence.
- The launcher window hides on close instead of being destroyed, so the hotkey
  keeps reusing one window.
- `hotkey::on_pressed` now toggles the launcher (falling back to the main window
  if the launcher cannot open) and emits `launcher:toggle`.
- Commands: `show_launcher`, `hide_launcher`, `resize_launcher`, `quick_connect`,
  `quick_connect_history`, `focus_server_in_catalog`.
- `connect_server` records connection history (recent ordering in the launcher).
- `quick_connect` validates RDP input, applies stored-credential matching,
  starts a session, appends deduplicated history (bounded by
  `launcher.recentLimit`) and optionally saves the ad-hoc server.

### UI

- `LauncherPanel.svelte` (rendered only in the launcher window):
  - search box focused on show, incremental keyword/tag filtering, highlighted
    name/subtitle fragments, recent-connect ordering, tags and optional note;
  - keyboard: Up/Down select, Enter connect, Tab quick connect, Esc hide;
  - optional credential line (`launcher.showCredentials`) and note
    (`launcher.showNote`);
  - contextual actions: connect, new window, edit (opens the catalog side
    panel), duplicate, delete, shortcut, `.rdp` export, copy host/user/password,
    Open SFTP (Phase 6 notice), alternate credentials/runners (Phase 6 notices);
  - auto-resizes the window height with the result count;
  - quick-connect mode: protocol selector (address protocols, RemoteApp
    excluded; non-RDP engines disabled until Phase 6), address/port/username/
    password, prefill from the selected server, save-server and
    match-credential toggles, recent history list;
  - `launcher:shown`/`launcher:toggle` listeners refocus the search box;
    Escape hides the window.
- Main window: `catalog:edit-server` event loads the catalog and opens the
  editor for a launcher selection.
- Settings copy now describes the real launcher behaviour instead of the Phase 0
  placeholder.

## Defaults and decisions

- Default hotkey remains `Alt+M` (configurable with conflict detection through
  `validate_hotkey` and the global-shortcut registration error).
- Result ordering: most recent connection first, then display name. Quick-connect
  entries are deduplicated by protocol/address/port/username and capped by
  `recent_limit`.
- Quick connect is RDP-only until the Phase 6 engines land; other protocols are
  visible but disabled with an explicit label.
- The launcher never blocks on IO: catalog/history load in parallel; failures go
  to the app error reporter.

## Deviations / deferred consumers

1. **P5-10** Open SFTP, alternate credentials and runner selection are visible
   actions that announce the Phase 6 engine work; edit/duplicate/delete/shortcut/
   export/copy are functional.
2. **P5-12** reload-on-language/data-change is wired through the shared app
   stores and launcher refresh; selection is clamped rather than restored by id
   after a language-only rebuild.
3. **P5-13** only RDP can actually connect in this phase; the selector prepares
   VNC/SSH/Telnet/Serial/SFTP/FTP/Custom for Phase 6.
4. **P5-16** “save entered info” stores a normal server; credential matching uses
   the first stored credential with the same username (address-scoped matching is
   Phase 7).

## Acceptance evidence

| Requirement | Evidence |
|---|---|
| Toggle/hotkey | `hotkey::on_pressed` → `window::toggle_launcher`; settings tests |
| Borderless/centred/no taskbar | `tauri.conf.json`; `launcher_placement` test |
| Search + highlight | `LauncherPanel.test.ts` filter/highlight test, E2E launcher |
| Recent ordering | `record_connection` + launcher sort; records tests |
| Keyboard nav/Esc/Tab | `LauncherPanel.test.ts`; E2E `Tab` prefill |
| Quick connect + history | `runs a quick connect and keeps history`; E2E |
| Auto-resize/position | `resize_launcher`, `launcher_placement_is_centred_and_clamped` |
| Edit routing | `focus_server_in_catalog` + `catalog:edit-server` |
| History dedupe/limit | `quick_connect_history_deduplicates_and_truncates` |

## Verification snapshot

- Rust: 274 tests (launcher placement test added), clippy/fmt clean.
- UI: 66 Vitest tests (5 launcher tests added).
- E2E: 11 Playwright tests (launcher search/prefill/quick-connect flow).
- Traceability: P0–P5 coverage.

## Next phase

Phase 6 adds the remaining protocol engines (VNC, SSH/Telnet/Serial, SFTP/FTP),
hosts/runners, and turns the launcher's disabled protocol options and actions on.
