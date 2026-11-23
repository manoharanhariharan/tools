# Phase 6 — Other protocols, hosts & runners

**Status: complete (2026-11-22), including the SFTP/FTP file-transfer stage
(Phase 6b).** VNC, external terminal runners, RemoteApp, LocalApp, the runner
system and remote file browsing/transfers are implemented and tested.

## What shipped

### VNC (embedded RFB)

- `VncPlan` derives address/port/password/resize mode from `VncServer`.
- `VncBackend` uses `vnc-rs`: password auth, RFB 3.3–3.8, ZRLE/TRLE/CopyRect/
  Raw/CopyRect/DesktopSize encodings, shared sessions, dynamic desktop resize.
- Rect updates are blitted into a bounded RGBA framebuffer (≤ 4096×4096) and
  published through the existing frame ring; `Copy` updates, clipboard text and
  bell events are handled.
- Pointer/key input and special keys (`Ctrl+Alt+Del`, `Ctrl+Esc`, `Alt+F4`) are
  forwarded as X11 keycodes. Keyboard mapping covers letters, digits, named
  keys, modifiers and F1–F12; unknown keys are ignored rather than guessed.
- The session pane maps pointer coordinates through `object-fit: contain` and
  focuses the desktop on click.

### External terminal runners (SSH / Telnet / Serial)

- Runner model in `hr-config`: id, name, protocol, kind, executable path,
  argument template, parameters, environment, hosting flag, session-config path,
  install state and macro list; defaults for PuTTY SSH/Telnet/Serial and KiTTY
  SSH.
- Template expansion supports `{host}`, `{port}`, `{user}`, `{password}`,
  `{privateKey}`, `{startupPath}`, `{sessionConfig}`, serial fields and
  `%1RM_*` macros; passwords expand last so braces in a password are safe.
- `ProcessPlan` resolves the requested runner (name or id), falls back to the
  per-protocol default, validates the executable path and rejects internal
  hosting with a clear message.
- Session config paths become `-load <path>` for PuTTY/KiTTY; startup commands
  are written to a temporary file and passed with `-m`.
- `ProcessBackend` launches the runner, marks the session connected, watches the
  process, kills it on cancel, reports the exit code and removes temporary
  files. Native window embedding remains Phase 9/11 (P6-09).
- Settings → Runners: list/add/edit/delete, executable path, argument template,
  session-config path, default per protocol, hosting toggle and macro help.
  Server editor exposes a per-server runner selector.

### RemoteApp

- `ProcessPlan` generates an `.rdp` payload with `remoteapplicationmode:i:1`,
  application name/program, `rdpinit.exe`, clipboard/ports/smart-card,
  auto-reconnect and audio settings, then launches `mstsc.exe` and cleans the
  temp file on exit.

### Local application

- Executable and typed argument list are expanded through the existing macro
  engine; legacy untyped arguments are split with quote awareness; environment
  variables are passed through. Hosted mode launches externally with a warning
  until native hosting lands.

### Session/UI

- `SessionSnapshot.backend` now distinguishes `vnc`, `terminal`, `localApp`,
  `remoteApp` (and the upcoming `fileTransfer`).
- The session pane renders frames for VNC, shows an explicit external-window
  state for process backends, and exposes VNC special keys.
- Quick connect now supports RDP, VNC, SSH, Telnet and Serial; SFTP/FTP/Custom
  remain disabled with a Phase 6b label.
- Session manager plans are backend-neutral (`SessionPlan`) with VNC/process
  registration; reconnect, cancel, retry and teardown semantics are shared.

## Defaults and decisions

- One runner per launch; the requested runner wins over the protocol default.
- External process sessions do not retry automatically (max attempts = 1) to
  avoid duplicate windows; the user reconnects explicitly.
- Passwords reach external runners through their documented argument templates;
  `ProcessPlan`'s `Debug` output never includes arguments or credentials.
- VNC input uses fixed X11 keycodes; layouts beyond the covered keys are a known
  limitation and are documented.

## Deviations / deferred consumers

1. **P6-09** native window embedding (`IntegrateHost`) is deferred; runners open
   external windows. The session workspace still tracks them as tabs.
2. **P6-05** vnc-rs does not implement Hextile/RRE/CoRRE; ZRLE/TRLE/Raw/CopyRect
   and DesktopSize are supported. VNC keyboard mapping is QWERTY-oriented.
3. **P6-10…P6-19** are complete in Phase 6b (`docs/PHASE-6B.md`); drag/drop and
   inline rename remain partial as documented there.
4. **P6-23** `TERMSRV` credential lookup on `.rdp` import remains Phase 7 (OS
   credential APIs).
5. **P6-32/P6-34** PuTTY/KiTTY install/extract and theme JSON are not automated;
   the runner editor validates the executable path only on launch.
6. **P6-35** run-without-hosting is the current behaviour for all external
   runners; hosted mode is future native work.

## Acceptance evidence

| Requirement | Evidence |
|---|---|
| VNC plan/validation | `vnc_plan_requires_address_and_port`, `non_vnc_servers_are_rejected` |
| VNC framebuffer/copy/bounds | `vnc_backend::tests::*` |
| VNC input mapping | `ui/.../vnc.test.ts` |
| Runner defaults/templates | `runners::tests::*`, `ssh_runner_expands_macros_and_password`, `split_arguments_handles_quotes_and_escapes` |
| Serial/Telnet mapping | `serial_runner_uses_serial_settings` |
| Missing executable | `missing_runner_executable_is_reported`, `missing_executable_fails_clearly` |
| Process lifecycle | `process_exits_and_reports_completion` |
| RemoteApp `.rdp` | `remote_app_generates_an_rdp_payload` |
| LocalApp arguments | `local_app_renders_typed_arguments` |
| Settings runner editor | `RunnersSettings.svelte` |

## Verification snapshot

- Rust: 300 tests.
- UI: 73 Vitest tests.
- E2E: 13 Playwright tests.
- Traceability: P0–P6 (P6-10…19 deferred to Phase 6b).

## Next

Phase 6b: SFTP/FTP remote browser and transfer queue. Then Phase 7 credentials
and security.
