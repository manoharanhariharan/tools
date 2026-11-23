# Feature traceability — Phase 9

Status: **Done**, **Partial** (platform/environment limitation), **Deferred**.

| ID | Status | Implementation | Verification |
|---|---|---|---|
| P9-01 | Done | `parse_cli_args` switches/ULID/tag/name, lists, dedupe | `hr-platform/src/cli.rs` tests |
| P9-02 | Done | single-instance plugin forwards argv; queued actions | `activation::handle`; manual/E2E launch flow |
| P9-03 | Done | per-selection/tag shortcuts, merged icon, start-minimized | `scripts`/P2 shortcut tests |
| P9-04 | Done | `tauri-plugin-autostart` + settings toggle | startup integration tests |
| P9-05 | Done | sync pre script (abort), async post script, hide option, editor Test | `session_scripts` tests; `test_script` command |
| P9-06 | Done | `SESSION_ID`/`SERVER_*` environment for both scripts and Test | `scripts::tests::environment_variables_reach_the_script`, `session_scripts` env test |
| P9-07 | Done | runner params/env, inline PEM temp file with cleanup | `inline_private_keys_become_temporary_files`, `prepare_writes_the_private_key_and_cleanup_removes_it` |
| P9-08 | Done | JSON/CSV/RDP/DB imports; first-run legacy dialog + server import | `find_legacy_database` test; `LegacyMigrationDialog` |
| P9-09 | Done | JSON selection export, `.rdp` export | existing export tests/E2E |
| P9-10 | Done | manifest version comparison, pre-release opt-in, breaking flag/modal, opt-out | `hr-platform/src/update.rs` tests; `updates.rs`; E2E update dialog |
| P9-11 | Done | 10-launch threshold, 90-day cooldown, permanent opt-out | `rating_prompt_respects_thresholds_and_opt_out`; rating dialog tests |
| P9-12 | Done | tray show/hide, recent, reset, logs, feedback, about, exit | tray unit test + `tray-hide` key |
| P9-13 | Done | single-instance activation from shortcuts/second launch | same path as P9-02 |

## Deviations

- SFTP/FTP sessions do not run pre/post scripts.
- Updates link to an installer; no self-update/MSIX packaging yet.
- Update feeds are user-configured; no default vendor endpoint is baked in.

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
