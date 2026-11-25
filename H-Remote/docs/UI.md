# UI architecture and conventions

## 1. Module map

```
ui/src/
  App.svelte              shell composition: loading/fatal/shell/routes/overlays
  main.ts                 mount + global styles
  app.css                 design tokens, layout primitives, global reset
  lib/
    components/           generic, reusable presentation
    features/             feature containers and their private components
    state/                cross-feature reactive state (runes classes)
    ipc/                  contract, client, events, mock backend, runtime
    i18n/                 catalog map + Fluent store
    theme/                CSS variable application
  tests/                  fixtures + setup
  e2e/                    Playwright specs
```

## 2. Component contracts

- Presentational components:
  - props in, callbacks out;
  - no global store reads, no IPC;
  - unique accessible names, `data-testid` only where a stable E2E hook is
    needed;
  - scoped styles by default. Only cross-feature layout lives in `app.css`.
- Feature containers:
  - read the app store / invoke commands;
  - own loading/empty/error states;
  - call reusable primitives rather than duplicating markup;
  - subscribe through `CleanupRegistry` only.
- `state/app.svelte.ts` is the only place that applies backend config, theme and
  locale. Components call `patchConfig`/`finishGuidance` and react to the
  resulting state.

Current shared primitives: `Button`, `Icon`, `Toggle`, `TextField`,
`SelectField`, `Modal`, `TitleBar`, `Sidebar`, `BusyOverlay`, `DialogHost`,
`NoticeBar`, `ContextMenu`, `TriStateCheckbox`, `HighlightedText`, `ServerIcon`,
`MarkdownView`, `PasswordField`.

### Catalog feature (Phase 2)

```
features/catalog/
  CatalogPage.svelte        shell: header, toolbar, tag panel, views, editor
  CatalogToolbar.svelte     search, view switch, sort, notes, bulk actions
  TagPanel.svelte           counts, filtering, pin/rename/delete/order/shortcuts
  CatalogList.svelte        list rows + nested folder groups (recursive)
  CatalogCards.svelte       card grid + nested folder groups (recursive)
  CatalogTree.svelte        tree view with drag & drop and folder menus
  ServerRow.svelte          list row: inline address edit (Enter saves without
                            connecting), note, context menu
  ServerCard.svelte         card presentation
  ServerEditor.svelte       side panel: protocol tabs, credentials, stock/library
                            icons, bulk; bounded to the visible window width
  stockIcons.ts             built-in Windows/Linux/macOS/server/database/cloud/
                            web/terminal/container/firewall/VM/desktop icons
  ImportExportDialog.svelte JSON/CSV/RDP/DB import and JSON/RDP export
```

`state/catalog.svelte.ts` owns snapshot/view state, debounced search, selection,
expansion, editor drafts/dirty fields, ordering, source statuses/filtering and
import/export triggers. Components never call `structuredClone` on reactive
proxies; use `cloneJson`.

Sources (Phase 8): `ServerRow`/`ServerCard` show a source badge and use warning
styling for read-only sources; `CatalogList` inserts a source header group when
more than one source exists (local first); `CatalogToolbar` adds a source filter.
`ServerEditor` renders a read-only banner and disables Save for foreign servers.
Custom protocols add a protocol-name field and a validated JSON object editor for
runner fields.

Search matching is configured in Settings → General: Direct is always active;
Initials/Discrete/Pinyin/Pinyin-initial toggles persist to `keywordMatch`, and the
auto-locale toggle adds pinyin providers for `zh*` locales. Settings → Data shows a
Localisation card (active locale, completeness, missing keys) and the capability
degradation notes.

### Settings feature

```
features/settings/
  SettingsPage.svelte       section navigation + routing
  GeneralSettings.svelte    language, startup, updates, matching, close behaviour
  AppearanceSettings.svelte themes (incl. High Contrast) and font size
  LauncherSettings.svelte   hotkey and recent limit
  RunnersSettings.svelte    runner definitions (external / externalSsh / internal)
  CredentialsSettings.svelte vault, grouped by source, gated secrets
  DataSourcesSettings.svelte local path, additional read-only sources, health
features/updates/
  UpdateDialog.svelte       version, pre-release/breaking warnings, notes, download
  RatingDialog.svelte       rate/later/permanent opt-out
  LegacyMigrationDialog.svelte detected PRemoteM/1Remote profile import
```

`DataSourcesSettings` polls statuses every 5 s and also listens to
`source:statuses` through a `CleanupRegistry`; the interval and listener are
released on destroy. The update dialog opens for manual checks and for
`update:available` (once per run); the rating and legacy dialogs are driven by the
bootstrap payload.

### Session feature (Phase 3+)

```
features/session/
  SessionWorkspace.svelte   single combined header (drag handle, tabs, actions,
                            window controls), grid/snap layouts, focus/full-screen,
                            context menus, close-all; standalone in the shared
                            `sessions` window
  SessionPane.svelte        one session surface: RDP/VNC frames, process
                            state, VNC pointer/keyboard/special keys, resize
  FileBrowser.svelte        SFTP/FTP listing, path navigation, transfers
  AvailabilityDialog.svelte reachability status, latency, 5 s countdown
  SessionDisplayDialog.svelte fixed resolution, 1–4 virtual monitors, fit reset
  layouts.ts                snap presets + auto layout + validation
  vnc.ts                    browser → X11 keycode/button/coordinate mapping
state/sessions.svelte.ts    tabs, frames, visibility polling, layout/button
                            settings, activation, shared-window hosting, teardown
```

Detached windows load the same SPA with `?session=<id>` and render the workspace
for that session only.

Error modals are de-duplicated: the same message cannot reopen after dismissal for
the rest of the session, so a recurring input/teardown error cannot create a modal
loop. Errors are still written to the crash/error log.

`SessionPane` forwards VNC desktop input and IronRDP fast-path input (keyboard
scan codes, pointer buttons/moves and wheel). RDP sessions expose a **Display**
button/menu for fixed resolution and 1–4 virtual monitors; the embedded engine
merges virtual monitors into one wide canvas. See `docs/RDP-INPUT-DISPLAY.md`.

### Launcher feature (Phase 5)

```
features/launcher/
  LauncherPanel.svelte      search/recent list, contextual actions, quick
                            connect form + history (rendered only in the
                            borderless `launcher` window)
```

The launcher window loads `index.html?window=launcher`; the shell and workspace
are not rendered there. `launcher:shown`/`launcher:toggle` refocus the search
box, and Escape hides the window through the backend.

## 3. Design tokens

`app.css` defines CSS variables for the full palette, typography scale, radii,
borders, shadows, title/tab heights and sidebar width. The Rust theme resolver and
`lib/theme/apply.ts` overwrite the colour/typography variables at runtime.

Rules:

- Never hard-code a palette colour in a feature; use `var(--hr-*)`.
- Use `color-mix(in srgb, ...)` for derived surfaces.
- Respect `prefers-reduced-motion` (globally handled).
- Keep font sizes derived from `--hr-font-size-*`; the user can change the base
  size (10–20 px).

## 4. State and dialogs

- `busy` is reference counted; `busy.track()` guarantees release in `finally`.
- `dialogs.message/confirm/input/password` return promises. Opening a new dialog
  resolves the previous one with `null`, so callers never hang.
- `app.reportError()` writes a redacted crash report and opens `ErrorReportDialog`;
  it is safe to call from event handlers and rejected promises.
- `app.notice` is a dismissible, non-blocking message bar (no auto-dismiss timer,
  therefore no timer leak).

## 5. Navigation and routes

Routes: `servers` (catalog), `settings`, `about`. `ui:navigate` from the launcher
selects a route. The catalog replaces the Phase 0 placeholder without changing
the shell contracts.

## 6. Accessibility baseline

- All icon-only buttons have `aria-label` and a title.
- Modal has `role="dialog"`, `aria-modal`, labelled heading, focus trap, Escape
  handling and focus restoration.
- Toggles are `role="switch"` with `aria-checked` and labelled descriptions.
- Form controls have explicit labels and error associations (`aria-invalid`,
  `aria-describedby`, `role="alert"`).
- Busy overlay uses `role="status"`/`aria-busy`; notices use `role="alert"`.
- Visible `:focus-visible` outline and reduced-motion support.
- High-contrast support will extend token overrides in Phase 10.

## 7. Adding a feature

1. Create `features/<name>/` with a container and private components.
2. Add any new cross-feature state to `state/` and any command to
   `ipc/contracts.ts` + `ipc/client.ts` + the Rust DTO/command.
3. Use existing primitives; add a new primitive only when it is reused twice.
4. Add a Vitest test and, if it crosses routes/commands, an E2E test.
5. Add translation keys (dotted code keys; dash IDs in `.ftl`).
6. Update this document and the matching `docs/TRACEABILITY-PHASE-N.md` if the
   feature ID is in scope.
7. Ensure every listener/timer is registered with `CleanupRegistry` or cleaned in
   the same effect.

## 8. Testing notes

- Vitest runs with the mock backend automatically (no Tauri internals in jsdom).
- `setup.ts` cleans the DOM, localStorage and mock event bus after each test.
- App-store singleton state is reset explicitly with `app.resetForTests()`.
- Playwright runs against the same Vite app with `VITE_E2E=true`; no native
  window is required.
