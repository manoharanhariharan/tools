# Feature traceability — Phase 10

Status: **Done**, **Partial** (implementation choice/environment), **Deferred**.

| ID | Status | Implementation | Verification |
|---|---|---|---|
| P10-01 | Done | `custom_plan` launches `Server::Custom` via runner + `extraFields` macros/env; built-in blacklist | `custom_protocols_use_a_named_runner_and_extra_fields`; editor JSON validation |
| P10-02 | Done | per-launch resolution (requested → default → first compatible); `RunnerKind::ExternalSsh` | runner tests; `external_ssh_kind_behaves_like_an_external_runner` |
| P10-03 | Done | Search matching settings; `effective_matchers` for `zh*` + auto-locale flag | Rust matcher test; `matchers.test.ts` |
| P10-04 | Done | live theme preview, 10–20 clamp, custom colours (Phase 0) + High Contrast | `hr-theme` preset/clamp tests |
| P10-05 | Done | `i18n_report` command + Localisation card with missing keys/completeness | `i18n` crate missing-key tests; settings card |
| P10-06 | Partial | CSS `content-visibility` containment + lazy/async icon decoding; bounded icon cache | style review; existing icon-cache tests |
| P10-07 | Done | 150 ms debounce + coalesced refresh follow-up | `coalesces overlapping refreshes into one follow-up run` |
| P10-08 | Done | bounded 24 h match cache keyed by revision; `spawn_blocking` >300 servers | `match_cache` tests; `match_cache_keys_change_with_every_input` |
| P10-09 | Done | capability card explains Hello/DPAPI/CredUI/autostart fallbacks | `DataSettings` capability card |
| P10-10 | Done | titles/labels, focus-visible, actionable runner/script errors | UI review; existing focus styles |
| P10-11 | Done | source-aware empty states, add-server tooltip, no-match hint | `CatalogPage`/`CatalogToolbar` |
| P10-12 | Done | shutdown cancels sessions; close-to-tray keeps them; tokens per session | `shutdown_cancels_every_session` |
| P10-13 | Done | atomic JSON writes + SQLite transactions/WAL | existing store/schema tests |
| P10-14 | Done | High Contrast preset, labelled controls, focus rings, decorative `alt` | `ships_exactly_ten_unique_presets`; UI review |

## Deviations

- Virtualisation is CSS containment (not JS windowing) to preserve editing and
  folder semantics.
- Custom protocols require an external runner; internal terminal hosting remains
  out of scope.
- Match caching is in-process only (no cross-run persistence).

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
