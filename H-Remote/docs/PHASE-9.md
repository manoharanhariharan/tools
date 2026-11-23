# Phase 9 — Automation & integrations

**Status: complete (2026-11-22).** Shortcut/CLI activation, startup integration,
pre/post-connect scripts, runner automation, legacy import, update checks, rating
prompts and the tray menu are all wired and covered by tests.

## What shipped

### Shortcuts, CLI and single instance (P9-01/P9-02/P9-03/P9-13)

- `hr_platform::parse_cli_args` accepts `activate`, `start-minimized`,
  install/uninstall, `ULID:<id>`, `#tag` and display-name targets, as separate
  arguments or comma/semicolon lists; duplicates are removed and invalid tokens are
  reported without dropping valid ones.
- The single-instance plugin forwards a second launch's argv to the running app;
  actions are queued until `ui_ready` and then dispatched.
- Shortcuts per selection/tag keep the merged icon, pass `--start-minimized`, and
  activate through the same CLI path.
- Autostart uses the registry Run key (`tauri-plugin-autostart`).
- The tray menu now has explicit **Show** and **Hide** entries next to the recent
  sessions, reset-window, logs, feedback, about and exit items (P9-12).

### Pre/post-connect scripts (P9-05/P9-06)

- `connect_server` generates the session id first, runs the pre-connect script
  synchronously (blocking task), and aborts with a validation error when the exit
  code is non-zero (stderr is included in the message).
- After a session reaches a terminal state, the post-disconnect script runs in the
  background from a watcher task that ends with the session or manager shutdown.
- Both scripts receive `SESSION_ID`, `SERVER_ID`, `SERVER_NAME`, `SERVER_HOST` and
  `SERVER_TAGS` (comma separated). The hide-window option maps to
  `CREATE_NO_WINDOW` on Windows.
- The editor's **Test** buttons now pass the draft server id so the same
  environment is used for a manual run.
- File transfers (SFTP/FTP) are excluded and documented: they have their own
  lifecycle.

### Runner automation (P9-07)

- Runner argument templates already supported `{host}`, `{port}`, `{user}`,
  `{password}`, `{privateKey}`, `{startupPath}`, `{sessionConfig}`, serial fields
  and `%1RM_*` macros; runner `parameters` and `environment` are applied.
- Inline PEM private keys (content beginning with `-----BEGIN … PRIVATE KEY-----`)
  are written to a unique temp file before launch, exposed to the template as
  `{privateKey}`, and deleted on exit, cancellation or spawn failure. On Unix the
  file is created with mode `0600`. Path-based keys pass through untouched.
- `split_arguments` now preserves Windows backslashes unless they escape
  whitespace/quotes/another backslash.

### Imports and exports (P9-08/P9-09)

- JSON, mRemoteNG CSV, PRemoteM/1Remote SQLite and `.rdp` imports were already in
  place; imports always target the writable local source.
- First-run detection now surfaces a **legacy profile** dialog with the detected
  PRemoteM/1Remote path, **Import settings** and **Import servers** actions, and a
  persistent "Not now" opt-out (`skipLegacyMigration`). Server import locates the
  catalog database next to the profile.
- JSON selection export and per-server `.rdp` export remain verification-gated.

### Update checks (P9-10)

- `hr-platform::update` parses dotted numeric versions (pre-release suffix
  supported), compares them numerically with zero padding, filters pre-releases
  unless opted in, and flags a **breaking** upgrade when the build is older than
  the manifest's `minimumVersion`.
- `src-tauri::updates` fetches the configured JSON manifest over HTTPS with an 8 s
  timeout and a 256 KiB cap, and reports errors inside the status.
- Settings → General has an Updates card: pre-release opt-in, check period
  (0–168 h, 0 disables), feed URL, and **Check now** with status/error text.
- A background watcher checks on the configured cadence, records
  `engagement.lastUpdateCheckAt` and emits `update:available`. The dialog shows
  current → available, pre-release/breaking warnings, notes and a Download link;
  nothing is installed silently. `doNotCheckForUpdates` disables automatic checks
  while keeping manual checks available.

### Rating prompt (P9-11)

- Launches are counted once per process and `firstRunAt` is set on first start.
- The prompt appears after 10 launches, at most once per 90 days, and never when
  `doNotShowRating` is set. "Rate now" opens the feedback URL; "Later" records the
  timestamp; "Don't ask again" opts out permanently.

## Defaults and decisions

- Pre-connect scripts are synchronous by design; the post script never delays
  session teardown.
- Private keys are never placed on a command line when supplied as inline PEM.
- Update feeds are opt-in by URL; there is no hard-coded vendor endpoint.
- Rating thresholds: 10 launches / 90-day cooldown.

## Deviations

1. **P9-07** runner params/env were already present; this phase adds inline-key
   temp files and fixes Windows path splitting rather than new UI.
2. **P9-05** scripts are not run for SFTP/FTP file sessions.
3. **P9-10** the app links to the installer instead of self-updating; Store/MSIX
   packaging remains future work.
4. **P9-11** launch counting is per application start, not per window.

## Verification

- Rust: 336 tests (version/manifest rules, rating thresholds, legacy database
  discovery, script environment/abort, private-key temp files, CLI matrix).
- UI: 82 Vitest tests (update/rating/legacy dialogs, updates settings flow).
- E2E: 16 Playwright tests (manual update check + dialog dismissal).
