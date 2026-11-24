# Feature traceability — Phase 4

Status: **Done**, **Partial** (documented webview/OS limitation), **Deferred**
(later phase), **N/A (rebuilt)** (ActiveX/WPF-specific).

| ID | Status | Implementation | Verification |
|---|---|---|---|
| P4-01 | Done | `SessionWorkspace` tabs; `set_window_title` follows the active tab | E2E tabs; window-title command |
| P4-02 | Done | icon/colour mark/state dot/name/tooltip, optional reconnect/close | E2E tab count; workspace markup |
| P4-03 | Done | middle-click close, close-all confirmation | E2E middle click; `closeAll` dialog |
| P4-04 | Done | full tab context menu incl. display presets and Ctrl+Alt+Del | `menuFor`; store actions |
| P4-05 | Partial | OS min/max/restore; always-on-top deferred | window chrome |
| P4-06 | Partial | F11 full-screen + restore; right-click maximize is native | `fullscreenchange` handling |
| P4-07 | Done | Ctrl+1…9, Ctrl+Tab/Shift | E2E `Control+2`; `cycle/selectIndex` tests |
| P4-08 | Partial | click/pointer focus moves into the pane; tab click selects | pane focus handlers |
| P4-09 | Done | `open_session_window` opens/focuses the shared `sessions` window | `session_window_is_a_single_shared_label` |
| P4-10 | Done (shared) | every connection is hosted as a tab in one window; no cross-window drag | `sessions.connect`, store tests |
| P4-11 | Done (UI) | empty standalone workspace offers Close window; async cleanup | App session-window branch |
| P4-12 | Done | requested placement wins, then stored placement | `open_session_window` order |
| P4-13 | Done | grid toggle requires 2+ tabs | E2E `Grid`; `setGridMode` |
| P4-14 | Done | `autoLayout` balanced grid | `auto layout balances every count` |
| P4-15 | Done | preset picker 2–9 plus N columns/rows | `every offered preset…`, `supports wide/spanning` |
| P4-16 | Done | count mismatch falls back to auto; validation rejects overlap | `falls back to automatic…`, `isLayoutValid` |
| P4-17 | Done | `auto` entry + persisted layout id | `shows every tab…persists the layout` |
| P4-18 | Done | 6 px gap, spanning cells, clamped grid tracks | layout tests + CSS grid |
| P4-19 | Done | grid renders every session tab in the shared window | E2E two panes |
| P4-20 | Done (shared) | all connections open the shared session window with tabs/grid | `sessions.connect`; window tests |
| P4-21 | Done | restore leaves full-screen/focus back to tabs | workspace restore button |
| P4-22 | Done | shared window focus/reuse; no per-connection cache | `open_session_window` |
| P4-23 | Partial | active tab restored from session list; last-token map not yet applied | locality fields |
| P4-24 | Done | main + shared session-window placements saved/restored | `capture_placement`, `session_windows` |
| P4-25 | Deferred (Phase 11) | `prefer_last_display` stored, policy not enforced | locality field |
| P4-26 | Done (layout) | content padding keeps panes inside maximised windows | workspace CSS |
| P4-27 | Partial | pane observer is debounced; explicit minimize suppression deferred | `ResizeObserver` |
| P4-28 | Partial | title/taskbar updates; custom session icon deferred | `set_window_title` |
| P4-29 | Done | shutdown closes sessions, flushes stores, trims working set | `AppState::shutdown` |
| P4-30 | Done | manager registry + shared `sessions` window label | manager/window tests |
| P4-31 | Partial | catalog→plan→manager pipeline; scripts arrive in Phase 9 | connect/activation flows |
| P4-32 | Done | `activation:connect` resolves id/tag/name and starts sessions | `connectActivation` store test path |
| P4-33 | Done (UI) | close removes tabs; empty standalone window can close; async cleanup | E2E close-all/middle click |
| P4-34 | Partial | `mstsc` child watcher; script-after-disconnect in Phase 9 | `mstsc` backend |

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
