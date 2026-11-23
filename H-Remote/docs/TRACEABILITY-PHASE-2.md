# Feature traceability — Phase 2

Status: **Done**, **Partial** (local source/session host pending), **Infra**
(consumer lands in a later phase).

| ID | Status | Implementation | Verification |
|---|---|---|---|
| P2-01 | Done | `CatalogToolbar` view switch; `set_server_view` | E2E `searches, filters tags and switches views` |
| P2-02 | Done | `search::SearchQuery::parse`; catalog toolbar input | search tests; catalog store test |
| P2-03 | Done (include chips) | `TagPanel`, pinned bar, `-#tag` typing | `tag_filters_are_prefix_based_and_exclusive` |
| P2-04 | Done | `App.svelte` navigation + catalog routes | App/E2E navigation tests |
| P2-05 | Done | toolbar actions (add/import/export/sort/refresh/settings/about) | E2E + catalog store tests |
| P2-06 | Done | responsive flex layout, note toggle, column widths persisted | `persists sort, view mode and column widths` |
| P2-07 | Done | `get/update_list_view`, tree view locality | catalog store test; mock locality |
| P2-08 | Done | `provider_matches` name/subtitle | `direct_search_matches_name_and_subtitle_with_highlights` |
| P2-09 | Done | `SearchQuery::parse` tag operators, prefix tags | tag query test |
| P2-10 | Done | `highlight_ranges` + `HighlightedText` | highlight tests; E2E search |
| P2-11 | Done (linear) | debounced full recompute | catalog store filter test |
| P2-12 | Partial | 150 ms debounce; sequential matchers | catalog store test |
| P2-13 | Done | `MatchProvider` + config matchers | `initials_discrete_and_pinyin_providers_work`, `direct_provider_is_always_kept` |
| P2-14 | Done | empty / no-match / error states in `CatalogPage` | catalog store + App tests |
| P2-15 | Done | `CatalogSort` + toolbar select | `custom_sort_uses_persisted_order` |
| P2-16 | Done | `natural_ip_cmp` | natural IP sort test |
| P2-17 | Partial (single source) | rows expose source column; grouping needs Phase 8 | catalog store test |
| P2-17a | Done | `CatalogList`/`CatalogCards` recursive folder groups | `folder_tree_is_nested_and_folders_come_first`; catalog store |
| P2-17b | Done | `build_catalog_tree` folder-first ordering | folder tree test |
| P2-18 | Done | `set_custom_order` + `moveServerOrder` | catalog store reorder test |
| P2-19 | Done | list `groupExpanded` + tree `expanded` persisted | `toggleFolder`; catalog store |
| P2-20 | Done | `TriStateCheckbox`, visibility-scoped select-all | catalog store selection tests |
| P2-21 | Partial | connect notice until Phase 3 | catalog store connect path |
| P2-22 | Done | `openBulk` + dirty-field merge | bulk edit store test |
| P2-23 | Done | bulk delete with confirmation | `deleteSelected`; E2E flows |
| P2-24 | Done | `export_servers_json` + plaintext warning | catalog store export test |
| P2-25 | Done | `create_shortcut_for_servers` + merged ICO | `merged_icons_produce_a_valid_ico` |
| P2-26 | Done | bulk bar cancel + `clearSelection` | catalog store selection test |
| P2-27 | Done | `ServerRow`/`ServerCard` render all affordances | E2E catalog assertions |
| P2-27a | Done | list header/grid: protocol, name, IP, note, target, source | E2E row snapshot |
| P2-28 | Done | dblclick/Enter connect, context menu, checkbox | E2E inline-edit/row tests |
| P2-29 | Done (plain hover) | note dot/popup + Markdown editor preview | note editor store path |
| P2-30 | Partial | source label + repository read-only guards; styling in Phase 8 | repository read-only credential test |
| P2-30a | Done | inline address edit in `ServerRow` | E2E `edits an address inline` |
| P2-30b | Done | `editing` row class for editor and bulk rows | catalog store/editor tests |
| P2-30c | Done | recursive tri-state through folder groups | catalog store selection test |
| P2-31 | Done | `Folders` schema v3 + `CatalogTree` | `folder_crud_updates_server_paths_and_prevents_cycles` |
| P2-32 | Done | folder create/rename/move/duplicate/delete | repository + catalog store tests |
| P2-33 | Done | `CatalogTree` drag & drop + `move_server_to_folder` | catalog store move test |
| P2-34 | Done (menu move) | sibling-only Move up/down for servers/folders | catalog store reorder test |
| P2-35 | Done | folder context `Edit all servers` → bulk editor | bulk edit store test |
| P2-36 | Done | expansion persistence + tree filtering via `catalog_view` | catalog store filter test |
| P2-37 | Done | `is_descendant_path` + repository move guard | folder cycle test |
| P2-37a | Done | List/Card folder groups expose no folder mutation | `CatalogList`/`CatalogCards` |
| P2-38 | Done | `TagPanel` counts + filter box | catalog store tag tests |
| P2-39 | Partial | connect-all notice until Phase 3 | `TagPanel` connect action |
| P2-40 | Infra (Phase 4) | new-tab-window notice | `TagPanel` connect action |
| P2-41 | Done | pin/unpin/rename/delete + persisted order | `rename_and_remove_cascade_across_servers`; catalog store tag test |
| P2-42 | Done | `create_shortcut_for_tag` | tag shortcut command |
| P2-43 | Done | `set_tag_order`, pinned/order locality merge | `collected_tags_report_counts_and_order`; catalog store |
| P2-44 | Done | add/edit/duplicate/bulk editor with title/footer | E2E add server; catalog store tests |
| P2-45 | Done | `changeEditorProtocol` preserves shared fields | `switches protocol on a new draft but keeps shared fields` |
| P2-46 | Done | per-protocol tabs/fields + validation | Rust validation tests; editor |
| P2-47 | Done | display-name datalist of existing names | editor markup; catalog store add test |
| P2-48 | Partial | generate/upload/library/URL icons; built-in set later | icon cache/merge tests |
| P2-49 | Done | colour input + hex value | editor markup |
| P2-50 | Done | tag input + suggestion chips | editor markup; tag tests |
| P2-51 | Done | Markdown textarea + preview | `MarkdownView`; editor |
| P2-52 | Partial | script text, hide-window, test run | `scripts::tests::*` |
| P2-53 | Done | runner name field (follows global when blank) | editor markup |
| P2-54 | Done | inherited credential select + inline `create_credential` | catalog store; command tests |
| P2-55 | Done | alternate credential add/edit/delete rows | editor markup; domain tests |
| P2-56 | Partial | typed argument rows add/remove/reorder | `local_app::tests::*` |
| P2-57 | Done | folder/tree placement selector | catalog store move test |
| P2-58 | Done | per-protocol `helpUrl` link | registry tests |
| P2-59 | Partial | dirty-field copy + "different values" sentinel; merge tags later | bulk edit store test |
| P2-59a | Done | side-by-side `<aside>` editor panel | E2E add server |
| P2-60 | Done | export JSON with warning | catalog store export test |
| P2-61 | Done | import JSON with fresh IDs | `json_export_and_import_assign_new_ids` |
| P2-62 | Partial | mRemoteNG CSV fields/folders/tags; icons not in format | `mremoteng_csv_maps_protocols_and_fields` |
| P2-63 | Done | legacy DB import | `import::tests::*` |
| P2-64 | Partial | `.rdp` import with manual password; `TERMSRV` lookup in Phase 3 | `rdp_import_and_export_round_trip` |
| P2-65 | Done | `.rdp` export + DPAPI blob (omit elsewhere) | `dpapi::tests::*`; `builds_rdp_file_with_optional_password_blob` |
| P2-66 | Partial | local target check + result counts; progress mask later | `ensure_local_target`; catalog store import test |
| P2-67 | Done | shortcut for selected servers or tag | `create_shortcut_for_*` |
| P2-68 | Done | `.lnk` target/working dir/`ULID:`/`#tag` args | `hr-platform::shortcuts` tests |
| P2-69 | Done | `merge_icons_ico` (up to 4, 2×2) | `merged_icons_produce_a_valid_ico` |

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

`check:traceability` validates Phase 0, 1 and 2 IDs.
