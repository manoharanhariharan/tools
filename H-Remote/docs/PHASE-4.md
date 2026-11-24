# Phase 4 — Session & tab workspace

**Status: complete (2026-11-22) for the in-window tab workspace, grid/snap
layouts, detached session windows and full-screen mode.** Native ActiveX tab
windows are replaced by the approved webview workspace; cross-window tab drag is
represented by detach/dock commands.

## What shipped

### Backend

- Locality v1 additions (backward compatible): `tabLayout`, `gridMode`,
  `showTabIcons`, `showTabClose`, `showTabReconnect`, and
  `session.sessionWindows` placements keyed by window label.
- `update_tab_workspace` command validates and persists the layout/button
  settings.
- `open_session_window` creates (or focuses) the shared `sessions` webview
  window, falling back to the stored placement, with a centred 980×700 default.
  Every connection lands there as a tab. Closing the window saves geometry
  automatically.
- `set_window_title` reflects the selected session in the OS title/taskbar.
- Session windows are included in the default capability set.
- CLI/shortcut activation (`ULID:`, `#tag`, display name) now resolves servers
  and starts every supported session type in the shared window instead of only
  showing a notice.

### UI

- `SessionWorkspace` replaces the single-session overlay:
  - tab strip with icon, colour mark, state dot, name, tooltip, optional
    reconnect/close buttons, middle-click close and drag reordering;
  - per-tab context menu: focus, reconnect, close, close others/all, edit,
    duplicate, copy hostname/username/password, shortcut, `.rdp` export,
    Ctrl+Alt+Del, display presets, tab-button customisation;
  - Ctrl+1…9 tab selection, Ctrl+Tab cycling, F11 full-screen with a connection
    bar/header that restores the window;
  - grid mode for 2+ tabs and a snap-layout picker with thumbnails;
  - automatic balanced layout; explicit layouts only apply when the cell count
    matches, otherwise the workspace falls back to automatic (no session is
    hidden);
  - per-pane status/error overlays with cancel/reconnect and a debounced
    `ResizeObserver` that renegotiates the remote resolution.
- `SessionPane` is a reusable pane (single and grid modes) with focus, disconnect,
  close and per-session resize handling.
- `layouts.ts` defines the snap presets (2–9 panes plus N columns/rows) and
  validates cells for overlap/bounds; auto layout covers any tab count.
- Sessions store now owns tab order, frames per session, visible polling,
  layout/button settings, focus mode, detach/dock state and clipboard forwarding;
  polling stops when no session is connected and is released on `dispose()`.

## Defaults and decisions

- Grid mode requires at least two tabs; toggling it on with fewer is ignored.
- The layout id is persisted; `auto` balances `ceil(sqrt(n))` columns.
- Detached windows are keyed by session id (`session-<id>`), so reopening focuses
  the existing window instead of duplicating it.
- A browser session (mock backend) cannot create windows, so "detach" focuses the
  session instead of hiding it.
- Frame polling is limited to visible sessions (one in focus mode, all non-detached
  tabs in grid mode) at 150 ms.

## Deviations / deferred consumers

1. **P4-05/P4-06** always-on-top and right-click-maximize are delegated to the
   native title bar/OS; F11 full-screen is implemented in-app.
2. **P4-10** cross-window tab dragging is detach/dock via the context menu and
   window placement, not a native drag between HWNDs.
3. **P4-20** full-screen is a maximised in-app workspace/window rather than a
   dedicated OS window per connection; detached windows cover multi-monitor
   placement. Per-connection full-screen window caching is deferred.
4. **P4-23/P4-25/P4-27** last-tab-token restore, "prefer last opened display" and
   resize suppression while minimised are partially wired through locality and
   the pane observer; a full multi-monitor/DPI pass is Phase 11.
5. **P4-31/P4-34** the open pipeline starts sessions and resolves credentials;
   pre/post-connect scripts and the unhosted-process watcher arrive with Phase 9
   automation (the `mstsc` child process is already watched).

## Acceptance evidence

| Requirement | Evidence |
|---|---|
| N tabs, title follows selection | E2E `hosts multiple tabs…`, `set_window_title` command |
| Tab close/reorder/middle click | E2E, `keeps multiple tabs…` store test |
| Grid + snap layouts | `layouts.test.ts` (every preset 2–9), E2E grid/layout step |
| No hidden sessions | `never hides a session for any tab count` |
| Keyboard switching | E2E `Control+2`, `selectIndex` store test |
| Persisted layout/buttons | `shows every tab in grid mode and persists the layout` |
| Detached window/dock | `open_session_window`/`dock_session_window`, window-label tests |
| Full-screen round trip | F11 + `fullscreenchange` + restore button (manual; browser API) |
| Clean shutdown | `AppState::shutdown` → `SessionManager::shutdown` |

## Verification snapshot

- Rust: 273 tests (2 window-label tests added), clippy/fmt clean.
- UI: 61 Vitest tests (7 layout + 4 tab/workspace store tests added).
- E2E: 10 Playwright tests.
- Traceability: P0/P1/P2/P3/P4 coverage.

## Next phase

Phase 5 adds the launcher and quick-connect surface: global search palette,
recent sessions, tag shortcuts and protocol-specific launch paths.
