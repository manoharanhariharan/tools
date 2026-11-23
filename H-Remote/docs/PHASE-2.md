# Phase 2 — Catalog UI, search, editor and transfer

**Status: complete (2026-11-22) for the local data source.** Card/List/Tree
catalog, folder hierarchy, tag panel, search providers, side editor, bulk
actions, import/export and shortcuts are implemented and tested. Multi-source
behaviour, the session engine and the icon *set* remain with their owning phases.

## What shipped

### Domain (`crates/hr-domain`)

- `search`: keyword/tag query parser (`#tag`, `+#tag`, `-#tag`, partial tags),
  direct/initials/discrete/pinyin/pinyin-initials providers (direct always kept),
  character-index highlight ranges with overlap merging, natural IP comparison,
  sort modes (insertion, protocol, name, address, custom) and a nested
  folder tree that keeps folders before direct servers (P2-17b).
- `rdp_file`: `.rdp` parse/serialize with unknown-key preservation, mapping to
  and from the complete RDP server model, plus free-form extra settings.
- `transfer`: versioned JSON export/import (new IDs on import), mRemoteNG CSV
  import with protocol mapping/folder/tags/description, `.rdp` import and
  single-server `.rdp` export.
- `folder::Folder` persisted entity, path validation and descendant checks.
- `icon::merge_icons_ico`: up to four icons composited into a square `.ico`.

### Persistence (`crates/hr-storage`)

- Schema v3 adds the `Folders` table (id, JSON path, unique index) and upgrades
  Phase 0/1 databases in one step.
- Repository folder CRUD: create/ensure (ancestors), rename, move (cycle-safe),
  duplicate subtree, delete subtree with servers moved to the parent, namespace
  server path rewrites on every operation, counts and cache invalidation.
- Legacy database import: reads our schema and best-effort PRemoteM/1Remote JSON
  rows (protocol mapping, tags, tree nodes, plaintext-only passwords with a
  warning for encrypted values).

### Tauri commands (`src-tauri`)

`catalog_snapshot`, `catalog_view`, folder CRUD, `move_server_to_folder`,
list/tree view locality, `set_custom_order`, `set_tag_order`, JSON export,
`.rdp` export (DPAPI on Windows; password omitted with a warning elsewhere),
JSON/CSV/RDP/database import, server/tag shortcuts, icon library + URL fetch,
script test, `new_server` and `create_credential`.

### UI (`ui`)

- `CatalogPage` replaces the placeholder: page header, toolbar (search, view
  switch, sort, note toggle, folder, import, export, add, refresh), pinned tag
  bar, bulk action bar, loading/empty/error/no-match states.
- List view with resizable columns, grid header tri-state checkbox, folder rows
  with recursive tri-state selection, row highlight for selection and editing,
  inline address editing (Enter/blur commit, Escape cancel), note hover popup,
  tag chips, last-connected date and context menu.
- Card view with the same folder hierarchy and selection semantics.
- Tree view with drag & drop for servers and folders, context menus, expand/
  collapse persistence and sibling-only reordering.
- Tag panel: counts, filter box, include filtering, pin/unpin, rename, delete,
  ordering, connect-all (session engine arrives in Phase 3/4) and shortcuts.
- Side editor: General/Connection/Display/Advanced/Scripts tabs, protocol
  switch preserving shared fields, folder picker, icon generate/upload/library/
  URL, colour, tag suggestions, Markdown note preview, credential inheritance
  with inline creation, alternate credentials, LocalApp argument rows, RDP
  gateway/redirection/performance fields, per-protocol fields and script tests.
  Bulk mode applies only fields changed in the session ("different values"
  sentinel) and highlights the affected rows.
- Import/export dialog: JSON, mRemoteNG CSV, `.rdp`, database path and export
  tabs, with explicit plaintext warning and result/warning counts.
- Catalog store keeps selection, expansion, sort, view mode, column widths,
  editor state and debounced search; timers are cancelled on `dispose()`.

## Defaults and decisions

- Search input is debounced by 150 ms and always includes the direct provider.
- Folder and server reordering is sibling-scoped so drag/move never crosses a
  parent by accident; the persisted order stores the full flattened path list.
- Folder paths are stored as JSON arrays so nesting is explicit and portable.
- Deleting a folder never deletes servers.
- `import_database_file` runs on a blocking Tauri task because `sqlx` futures
  with an owned pool are not `Send` inside the command handler.

## Deviations / deferred consumers

1. **P2-11/P2-12** incremental and parallel matching: filtering is a linear scan
   with debounce; catalogs at the expected scale do not need parallelism yet.
2. **P2-17** data-source grouping: only the local SQLite source exists until
   Phase 8, so every row is labelled `Local`.
3. **P2-21/P2-39/P2-40** connect actions show an explicit notice; the session
   host arrives in Phase 3 and tab windows in Phase 4.
4. **P2-29/P2-51** notes are Markdown in the editor and plain text in hover/list
   popups; full rendered hover arrives with the polish pass.
5. **P2-30** read-only styling depends on multi-source metadata (Phase 8);
   credential editability is already enforced by the repository.
6. **P2-34** reorder is exposed as Move up/down in context menus rather than
   insert-before/after drag hints; ordering itself is persisted per sibling.
7. **P2-48** ships generate/upload/library/URL icon flows; the shipped built-in
   icon *collection* is deferred to the polish phase.
8. **P2-52** scripts support text, hide-window and test run; the file picker is
   deferred until the script runner lands in Phase 9.
9. **P2-56** typed argument rows support add/remove/reorder; the live command
   preview is deferred to the launcher phase (Phase 5).
10. **P2-59** bulk mode copies only dirty fields; tag merging is a replace
    because merging needs an explicit UI affordance (planned for Phase 11).
11. **P2-62** CSV import maps protocol/fields/tags/folders; importing embedded
    icons is not possible from the mRemoteNG export format.
12. **P2-64** `.rdp` import takes the password from the user because Windows
    DPAPI blobs are not portable and `TERMSRV` lookup needs the Windows
    credential APIs planned for Phase 3.
13. **P2-66** import reports counts/warnings after completion; a progress mask
    arrives with large multi-source imports in Phase 8.
14. **Localisation of new strings:** the Phase 2 catalog/editor uses English
    literals while the Fluent infrastructure continues to localise the shell,
    settings and wizard. Extracting the full catalog surface into FTL is part of
    the Phase 10 localisation sweep, so translators work from a stable UI.

## Acceptance evidence

| Requirement | Evidence |
|---|---|
| Search syntax/providers | `search::query_parses_tag_operators_and_partial_tags`, `initials_discrete_and_pinyin_providers_work` |
| Highlight ranges | `highlight_ranges_merge_overlaps` |
| Natural IP sort | `natural_ip_sort_orders_numeric_segments` |
| Folder tree order | `folder_tree_is_nested_and_folders_come_first` |
| Folder CRUD + rewrites | `folder_crud_updates_server_paths_and_prevents_cycles` |
| Schema upgrade | `migrates_an_existing_phase_zero_database` |
| JSON/CSV/RDP transfer | `transfer::tests::*`, `rdp_file::tests::*` |
| Legacy DB import | `import::tests::*` |
| Icon merge | `merged_icons_produce_a_valid_ico` |
| Catalog store behaviour | `ui/src/lib/state/catalog.test.ts` (14 tests) |
| Catalog UI flows | `ui/e2e/smoke.spec.ts` (search/views, inline address, add server) |

## Verification snapshot

- Rust: 229 tests; clippy `--workspace --all-targets -D warnings`; rustfmt.
- UI: 44 Vitest tests (30 Phase 0/1 + 14 catalog store), svelte-check 0 errors.
- E2E: 8 Playwright tests.
- Traceability: `npm run check:traceability` covers P0, P1 and P2 IDs.

## Next phase

Phase 3 builds the IronRDP session host, the in-webview frame transport
(shared memory), the `mstsc` fallback and connection lifecycle events.
