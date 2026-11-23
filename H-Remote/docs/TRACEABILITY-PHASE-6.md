# Feature traceability — Phase 6

Status: **Done**, **Partial** (documented platform/engine limitation),
**Deferred** (Phase 6b/7/9), **N/A (rebuilt)**.

| ID | Status | Implementation | Verification |
|---|---|---|---|
| P6-01 | Done (rebuilt) | `VncBackend` (vnc-rs) + frame ring | `vnc_backend::tests::*`; manual live VNC |
| P6-02 | Done | address/port/password, lifecycle/reconnect, failure UI | manager + pane state |
| P6-03 | Done | resize mode stored; pane fits/clips; dynamic VNC resize | plan/frames; UI |
| P6-04 | Done | Ctrl+Alt+Del, Ctrl+Esc, Alt+F4 X11 keycodes | `special_events`, workspace menu |
| P6-05 | Partial | ZRLE/TRLE/Raw/CopyRect/DesktopSize; no Hextile/RRE/CoRRE | framebuffer tests; vnc-rs limits |
| P6-06 | Done | SSH runner, version/startup/config path, SFTP-on-connect warning | `ssh_runner_expands_macros_and_password` |
| P6-07 | Done | Telnet runner + startup/config path | telnet plan |
| P6-08 | Done | serial port/baud/bits/parity/flow/startup/config | `serial_runner_uses_serial_settings` |
| P6-09 | Deferred (Phase 9/11) | external windows; native embedding not implemented | `ProcessBackend` |
| P6-10 | Done | SFTP (`russh`/`russh-sftp`) + FTP (`suppaftp`) clients | `hr-files` tests; E2E SFTP tab |
| P6-11 | Done | startup path / CWD + home canonicalisation | `file_list`; E2E `/srv/data` |
| P6-12 | Done | name/size/modified/type, sorting, 6 date formats persisted | `FileBrowser.test.ts`; UI |
| P6-13 | Partial | home/up/path navigation + folder double-click; back/forward history deferred | UI tests; E2E navigation |
| P6-14 | Partial | upload/download with pickers; clipboard upload and recursive folder download deferred | transfer tests |
| P6-15 | Partial | rename dialog + delete confirmation; inline rename deferred | `FileBrowser` |
| P6-16 | Deferred | webview drag/drop path bridge | Phase 9 |
| P6-17 | Partial | transfer panel/progress/cancel; open-destination-folder deferred | `TransferSnapshot`; UI |
| P6-18 | Done | `FileManager` transfer queue with cancellation/progress | `copy_progress_reports_and_cancels` |
| P6-19 | Partial | protocol errors surfaced; reconnect via session reconnect | error paths |
| P6-20 | Done | published app name/program | `remote_app_generates_an_rdp_payload` |
| P6-21 | Done | audio mode/quality + additional `.rdp` | RemoteApp payload |
| P6-22 | Done | RemoteApplicationMode `.rdp` through mstsc | payload test |
| P6-23 | Deferred (Phase 7) | TERMSRV credential lookup | Phase 7 |
| P6-24 | Done | exe path, typed arguments, hosting toggle | `local_app_renders_typed_arguments` |
| P6-25 | Done | macros/environment expansion | `expand_macros`/`render_arguments` |
| P6-26 | Partial | external launch + exit watch; embedding deferred | `ProcessBackend` tests |
| P6-27 | Done | templates exist in `hr-domain` | local_app tests |
| P6-28 | Done | runner model in `hr-config` | `runners::tests::*` |
| P6-29 | Done | built-in PuTTY/KiTTY runner entries | defaults tests |
| P6-30 | Partial | external runner + macros; private-key temp copy deferred | runner tests |
| P6-31 | Done | settings editor + per-server runner selector | `RunnersSettings`, editor |
| P6-32 | Partial | session config path; install/extract/theme JSON deferred | runner editor |
| P6-33 | Done | macro list/help in runner editor | runner editor |
| P6-34 | Partial | executable path validated on launch | `missing_runner_executable_is_reported` |
| P6-35 | Done (current behaviour) | external windows for all runners | process tests |
| P6-36 | Done | `helpUrl` in protocol registry + editor link | registry tests |

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
