# Phase 1 — Domain model & persistence

**Status: complete (2026-11-22).** The complete protocol model, credentials, tags,
folders, typed LocalApp arguments, schema v2, encrypted JSON storage, repository
CRUD, pagination, caches and change stamps are implemented and tested.

## What shipped

### Domain (`crates/hr-domain`)

- Protocol-typed `Server` enum: RDP, RemoteApp, VNC, SSH, Telnet, Serial, SFTP,
  FTP, LocalApp, Custom, Dummy.
- Shared bases: identity/metadata (`ProtocolBase`), address/port with alternate
  credentials, user/password with inheritance and strict password/private-key
  exclusivity.
- All Appendix A fields for RDP (identity, gateway, display, redirection,
  advanced), RemoteApp, VNC, SSH, Telnet, Serial, SFTP, FTP, LocalApp, Custom.
- Validation with field-level issues; normalisation (tags, colours, folder
  paths, subtitles, display-name fallback, legacy tag→folder migration).
- Credential model with MD5 tuple hash (duplicate detection only), editability,
  alternate credentials, inheritance resolution and address/port normalisation.
- Tags: normalisation, counts, pin/order merging, rename/remove cascades.
- Folder paths: normalisation, validation, legacy migration, descendant checks.
- LocalApp arguments: eight typed kinds, selection maps, macro expansion,
  command-line rendering, Chrome/Edge and NoMachine templates.
- Bounded icon cache: base64/data-URL PNG/JPEG decode, dimension/size limits,
  entry + byte budgets, eviction.
- Link-time registry for all built-ins with categories, ordering, capabilities
  and help URLs; built-in names are blacklisted for custom protocols.
- Deterministic sample data covering every protocol for round-trip tests.

### Persistence (`crates/hr-storage`)

- Schema v2 migration: `Servers`, `Credentials`, `Configs`, `TableMeta` with
  indexes and an upgrade path from the Phase 0 database.
- Dialect-aware SQL builders (`?` vs `$n`, `ON CONFLICT` vs
  `ON DUPLICATE KEY UPDATE`) pre-wired for MySQL/PostgreSQL.
- Encryption-at-rest codec: AES-GCM values for server password/private key/
  gateway password, alternate credentials, argument secrets and credential
  secrets; legacy plaintext is readable and resealed on the next write.
- Encryption sentinel in `Configs.EncryptionTest`; a wrong key returns an
  explicit `AppError::EncryptionKey` and the app keeps running with storage
  disabled instead of corrupting data.
- Repository: single/batch server CRUD, pagination, credential CRUD with unique
  name handling and read-only enforcement, config get/set/delete, table stamps,
  counts, shared cached snapshots and invalidation on every write.
- Cache validation via `TableMeta.UpdatedAt`, so an external writer that bumps
  the stamp triggers a reload; missing/corrupt rows fail with explicit errors.
- Tauri commands for every repository operation, plus `list_protocols`, so the
  Phase 2 UI consumes a stable API.

### UI/visibility

- Settings → Data now shows schema version, encryption verification, server and
  credential counts, registered protocols and storage errors.
- The catalog remains the Phase 2 placeholder; no data entry UI is claimed yet.

## Defaults and decisions

- Passwords and private keys are `SecretString`s whose `Debug` output is
  redacted; JSON export is a separate concern (Phase 2/9).
- `ask_password_when_connect` clears any persisted password on normalise.
- Credentials are MD5-hashed exactly as specified; the hash is for duplicate
  detection, never authentication.
- `private_key`/`password` are mutually exclusive on both servers and
  credentials.
- Missing address/port on alternate credentials inherit from the server.
- A corrupt/unsupported schema version fails startup of the repository with a
  clear error; the app shell still runs and reports it.

## Deviations / deferred consumers

1. **Read-only data sources** are formalised in Phase 8 (MySQL/PostgreSQL). The
   local repository already refuses edits to `is_editable == false` credentials
   and the database layer probes writability.
2. **P1-44 batched UI marshalling** belongs to the Phase 2 view-model layer; the
   repository exposes shared immutable snapshots (`Arc<Vec<_>>`) and pagination
   so the UI can materialise in batches.
3. **P1-45 property-change suppression** has no analogue because Rust models do
   not raise property notifications. Bulk operations validate/encrypt once,
   write in one transaction and invalidate caches once, which is the intended
   observable behaviour.
4. **Bitmap cache** stores decoded RGBA plus dimensions rather than a
   platform-specific bitmap; the UI converts this to its renderer type.
5. **MySQL/PostgreSQL execution** is Phase 8; SQL building is dialect-tested now.

## Acceptance evidence

| Requirement | Evidence |
|---|---|
| Every protocol round-trips | `hr-domain::server::every_protocol_rounds…`, `repository::every_protocol_round_trips_through_storage` |
| No plaintext secret in DB | codec tests + raw `SELECT Json` assertions |
| Wrong encryption key detected | `wrong_vault_is_rejected_explicitly` (EncryptionKey) |
| Bulk insert/update/delete works | `upsert_servers`, `delete_servers`; batch atomicity test |
| Caches invalidate | write invalidation + external stamp test |
| Pagination | `crud_and_pagination_work` |
| Tags/folders/cascades | tag/folder tests |
| LocalApp macros/templates | local_app tests |
| Credential inheritance/duplicates | credential/repository tests |
| Schema upgrade | `migrates_an_existing_phase_zero_database` |

## Verification snapshot

- Rust: 201 tests (domain 41, storage 27, plus Phase 0 suites), clippy `-D warnings`,
  rustfmt.
- UI: 30 Vitest tests, svelte-check 0 errors.
- E2E: 5 Playwright tests (unchanged shell flows).
- Runtime: an existing Phase 0 database was migrated to schema v2 and the
  repository verified the OS-keyring sentinel on launch.

## Next phase

Phase 2 builds the catalog UI (card/list/tree, folders, tags, search, editor,
import/export) on top of the repository commands already exposed here.
