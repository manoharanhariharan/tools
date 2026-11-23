# Phase 10 — Extensibility, performance & polish

**Status: complete (2026-11-22).** Custom protocols launch through external
runners, matching is configurable with locale-aware defaults, large catalogs get
containment-based virtualisation and a bounded match cache, and the UI surfaces
localisation, capability and empty-state information.

## What shipped

### Custom protocols (P10-01/P10-02)

- `Server::Custom` entries are now launchable: `ProcessPlan::from_server` resolves
  the runner from `customProtocolName`, reuses the argument-template/private-key
  model, and rejects empty or built-in protocol names (the existing validation
  blacklist is enforced before launch as well).
- `extraFields` string values are exposed to the runner as `{field}` macros and
  `CUSTOM_FIELD` environment variables; the editor gained a custom-protocol name
  field and a validated JSON object editor for those fields.
- The protocol registry now registers `custom`, so it appears in the picker.
- Runner resolution stays per launch: requested runner → protocol default →
  first compatible runner → actionable error. `RunnerKind` gained `ExternalSsh`
  (behaves like an external runner, labelled as an SSH client); internal runners
  are still rejected for terminal protocols with a clear message.

### Matching and search performance (P10-03/P10-07/P10-08)

- Settings → General → **Search matching** exposes Initials, Discrete, Pinyin and
  Pinyin-initial providers plus an auto-locale toggle. Direct matching is always
  first.
- `KeywordMatchConfig::effective_matchers(language)` adds pinyin providers for
  `zh*` locales only when the user selected Direct alone (and auto-locale is on);
  the UI mirrors this in `state/matchers.ts` so both sides agree.
- Search stays debounced at 150 ms, and overlapping refreshes are coalesced into a
  single follow-up run instead of piling up IPC calls.
- Catalog searches are cached in a bounded 64-entry, 24-hour cache keyed by
  query + providers + source + catalog revision. Any local server write, import,
  source refresh or database swap bumps the revision and invalidates entries.
- Catalogs with more than 300 servers run matching on `spawn_blocking` when
  parallel matching is enabled, keeping the async worker responsive.

### Performance & accessibility polish (P10-04/P10-06/P10-14)

- Server rows, folder rows and cards use `content-visibility: auto` with intrinsic
  sizes so offscreen entries skip layout/paint; icons are `loading="lazy"` and
  decoded asynchronously. The domain icon cache remains bounded.
- A **High Contrast** theme preset (black background, yellow accents) joins the
  nine reference palettes; custom colours and live preview were already present,
  with the 10–20 px font clamp enforced in both Rust and the editor.
- Global `:focus-visible` outlines, labelled buttons and decorative icons with
  empty `alt` keep keyboard and screen-reader use predictable.

### Diagnostics and empty states (P10-05/P10-09/P10-10/P10-11)

- Settings → Data shows a **Localisation** card from `i18n_report`: active locale,
  per-locale completeness and runtime missing keys (fallbacks to English).
- The capability card explains graceful degradation (Hello → native credential
  prompt, DPAPI export omission, verification confirmation, registry autostart).
- Toolbar actions gained titles, and filtering to an empty/read-only source now
  shows a source-specific empty state with a “Show all sources” action.

### Safety (P10-12/P10-13)

- Closing/exit cancels every session through cancellation tokens and the manager
  shutdown path; `shutdown_cancels_every_session` covers it.
- JSON stores remain temp-write + fsync + rename, SQLite writes run in
  transactions with WAL, and the existing wrong-key/atomicity tests still pass.

## Defaults and decisions

- Custom protocols are external-runner only; internal terminal hosting is not
  implemented.
- Match cache: 64 entries, 24-hour TTL, revision-invalidated.
- Background matching threshold: 300 servers.
- Virtualisation uses CSS containment rather than a windowed list so inline
  editing, folder semantics and keyboard navigation stay intact.

## Deviations

1. **P10-06** list virtualisation is browser-level containment, not JS windowing.
2. **P10-01** custom protocols require a matching external runner configuration.
3. **P10-03** locale defaults are additive only when Direct is the sole explicit
   provider; any explicit multi-provider choice is preserved.
4. **P10-04** was largely delivered in Phase 0; this phase verifies the clamp and
   adds the High Contrast preset.

## Verification

- Rust: 343 tests (custom protocol plans, runner kinds, matcher defaults, match
  cache TTL/eviction, cache-key invalidation, high-contrast preset).
- UI: 86 Vitest tests (matcher defaults, refresh coalescing, dialogs).
- E2E: 16 Playwright tests (unchanged flows, still green).
