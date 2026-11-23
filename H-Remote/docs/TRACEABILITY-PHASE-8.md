# Feature traceability — Phase 8

Status: **Done**, **Partial** (platform/environment limitation), **Deferred**.

| ID | Status | Implementation | Verification |
|---|---|---|---|
| P8-01 | Done | `data_source_*` + `change_local_database_path`; path in Settings | `change_local_database_path`; Data Sources UI |
| P8-02 | Done | `SourceKind::MySql` via sqlx pool, 5 s timeout | `saved_sources_persist_encrypted_passwords_and_list_without_them`; compile check |
| P8-03 | Done | `SourceKind::Postgres` via sqlx pool, 5 s timeout | source-manager tests; compile check |
| P8-04 | Done | add/edit/delete/test/refresh commands; validation | `missing_sqlite_file_is_rejected`; UI test |
| P8-05 | Done | extra sources always read-only; write guards + disabled UI | `local_servers_are_writable_but_extra_sources_are_not`, `read_only_credentials_are_rejected` |
| P8-06 | Done | six states with text/colour and catalog filter | `wrong_vault_key_reports_encrypt_key_error_without_losing_the_source`; UI test |
| P8-07 | Done | watcher: check (0–99 s) + reconnect (0–3600 s), config-driven | `spawn_source_watcher`; `SourceManager::check` |
| P8-08 | Done | new DB opened and verified before swap; UI reload | `change_local_database_path`; path field test |
| P8-09 | Done | per-source caches + `last_checked`; manual refresh | `sqlite_source_loads_read_only_servers_and_keeps_the_cache_on_failure` |
| P8-10 | Done | catalog groups by source when >1; local first | `catalog.test.ts` grouping test; E2E `.source-name` |
| P8-11 | Done | source badges in list/card/launcher + import target selector | `ServerRow`/`ServerCard`/`LauncherPanel`; E2E |
| P8-12 | Done (renamed) | `hremote.dataSources.json`, vault-encrypted passwords | persistence/reload test |
| P8-13 | Done | failure keeps cache, watcher retries, UI stays responsive | cache test; watcher loop |
| P8-14 | Done | `data_source_refresh` / `_all`, Settings Refresh, toolbar reload | command + UI |

## Deviations

- Additional sources are read-only unconditionally (no write-probe capability detection).
- MySQL/PostgreSQL are not exercised against live servers in this environment.
- Timestamp invalidation follows the polling cadence rather than per-table triggers.

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
