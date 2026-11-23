# Phase 8 — Multiple data sources

**Status: complete (2026-11-22) with additional sources mounted read-only.**
The local SQLite catalog stays writable; extra SQLite/MySQL/PostgreSQL catalogues are
mounted for search, launch and import with health reporting, caching and silent retries.

## What shipped

### Sources

- **Local SQLite (P8-01/P8-08)** — Settings → Data Sources shows the current file, accepts
  a new path, creates the parent folder, opens the new database and verifies the vault
  sentinel **before** swapping it in, then reloads the catalog. A directory path, an
  unwritable folder, or a database encrypted with another key leaves the running catalog
  untouched. An empty path resets to the profile default.
- **MySQL / PostgreSQL (P8-02/P8-03)** — `hr-storage::SourceManager` builds a single-connection
  `sqlx` pool per source with a 5 s acquire timeout. Host, port, database, user and password
  are validated; ports default to 3306/5432 when zero.
- **Extra SQLite (P8-04)** — opens read-only (`read_only(true)`, 5 s busy timeout), so even a
  local file can never be written through the app.
- **Lifecycle (P8-04/P8-14)** — add/edit/remove, “Test connection”, and “Refresh” per source.
  Saving persists first and then attempts a load; an unreachable source still saves and
  reports its error instead of losing the definition.
- **Persistence (P8-12)** — additional sources live in the profile's
  `hremote.dataSources.json` (`schemaVersion` + array). Passwords are encrypted with the
  vault codec under the `datasource-password` purpose and are **never** returned to the UI.

### Read-only behaviour (P8-05/P8-11)

- Every additional source is read-only regardless of database grants; the app never issues
  writes against a secondary connection.
- Server/credential save, bulk save, delete and move-to-folder commands reject foreign
  `sourceId`s with a permission error; import always rewrites `sourceId` to `local`.
- The catalog row/card and launcher show a source badge; read-only sources get a warning
  style. The server editor opens read-only with a banner and a disabled Save, and the row
  menu swaps Edit/Delete for View/Duplicate.
- Import dialogs list every source but only the local one is selectable.

### Health & caching (P8-06/P8-07/P8-09/P8-13)

- States: `ok`, `notConnectedYet`, `lostConnection`, `accessDenied`, `encryptKeyError`,
  `otherError`, each with status text/colour in Settings and a filter option in the catalog.
- A background watcher loads sources after start-up, probes live sources every
  `databaseCheckPeriod` seconds (0–99, 0 disables) with a cheap `SELECT 1`, and retries failed
  sources every `databaseReconnectPeriod` seconds (0–3600). It emits `source:statuses` on
  change and never blocks the UI.
- Per-source caches hold servers/credentials plus `last_checked`; a failing refresh keeps the
  previous catalogue and only downgrades the status, so lost databases keep working from cache.
- Credentials loaded from extra sources are marked `isEditable = false` and tagged with their
  `databaseId` so the vault groups them correctly.

### Catalog integration (P8-10)

- `catalog_snapshot`/`catalog_view` merge local and extra servers/credentials, deduplicated by
  id with local precedence. `catalog_view` accepts a `source` filter that the toolbar exposes
  once multiple sources exist.
- The list view groups visible servers under source headers (local first) and marks extra
  groups “read-only”.

### Credential inheritance fix

While wiring cross-source credentials, connect-time inheritance was found to be resolved only
in the vault UI. `connect_server` now resolves a named credential into username/password/key
before alternate-credential and per-connection overrides, so Phase 7 inheritance actually
reaches the protocol plans.

## Defaults and decisions

- Additional sources are read-only by design (ADR-023); no capability probing of DB grants.
- Timeouts: 5 s connect/acquire for every secondary connection.
- Check/reconnect periods reuse the existing `databaseCheckPeriod` (10 s) and
  `databaseReconnectPeriod` (120 s) settings and their 0–99/0–3600 clamps.
- Source passwords are only decrypted inside `SourceManager`; `configs()` always clears them.
- MySQL/PostgreSQL expect 1Remote-compatible `Servers(Protocol, ClassVersion, Json)` and
  `Credentials(Json)` tables; foreign schemas are reported as `otherError`.

## Deviations

1. **P8-12** the file is `hremote.dataSources.json` inside the profile, not
   `1Remote.dataSources.json`; the profile directory is the app's own namespace.
2. **P8-05** read-only is enforced unconditionally instead of inferring grants. This is
   stricter than the reference and avoids write-probe side effects.
3. **P8-02/P8-03** MySQL/PostgreSQL are covered by unit-level configuration/pool tests and
   compile checks; no live server is available in this environment, so a live handshake smoke
   test remains a manual step.
4. **P8-10** grouping applies to the flat list view; card, tree and launcher use
   source badges instead of separate group headers.
5. **P8-09** invalidation is timestamp-based on the polling cadence rather than per-table
   triggers; the manual Refresh button forces a reload immediately.

## Verification

- Rust: 311 tests (source manager SQLite/MySQL/PostgreSQL loading, cache-on-failure,
  credential tagging, encrypted persistence/reload, source_id defaults, read-only guards).
- UI: 77 Vitest tests (source grouping/filtering, Data Sources settings add/status).
- E2E: 15 Playwright tests (add a read-only source, grouped catalog, read-only editor).
