# Phase 11 — QA & release engineering

**Status: complete (2026-11-22) for the Tauri/Rust delivery model.** The reference
matrix is .NET/MSIX-specific; this phase maps it to reproducible Rust builds, adds
auditable release metadata and signing, and closes the remaining test gaps.

## What shipped

### Tests (P11-01…P11-05)

- Domain/serialisation/encryption/import mapping suites already covered every
  protocol variant, legacy JSON/CSV/DB mapping and vault behaviour; Phase 10/11
  added custom-protocol plans and matcher defaults.
- Search tests cover `#tag`, `-#tag`, `+#tag`, prefix/partial tag matching and
  natural IP ordering (`hr-domain/src/search.rs`).
- Grid/snap geometry is validated for 1–12 sessions: every preset keeps all cells,
  stays inside bounds and is layout-valid (no overlap).
- SQLite CRUD and source transitions have integration tests (ok → lostConnection
  with cache retention, encrypted-password reload, new
  `classify_source_error` mapping for denied/encrypted/other).
- Playwright now also drags tabs to reorder and toggles full screen (`F11`) in the
  multi-tab smoke test, alongside first run, connect/reconnect, launcher and SFTP.

### Build matrix (P11-06…P11-11)

- Documented the mapping from reference configurations/TFMs/publish profiles to
  Tauri: debug, portable single binary, NSIS/MSI/AppImage/DEB/DMG bundles. There is
  no .NET runtime to bundle, ready-to-run or trim; WebView2 is the Windows
  prerequisite. MSIX/Store is explicitly deferred.
- Dependencies that were git submodules in the reference are Rust crates pinned in
  `Cargo.lock` (Dragablz → Svelte, VncSharpCore → `vnc-rs`, PuTTY/KiTTY →
  user-configured external runners).
- CI keeps the fmt/clippy/test, UI, i18n, traceability, production-build and
  Playwright jobs; a new `release.yml` builds Windows/Linux/macOS bundles on `v*`
  tags, optionally signs them and uploads one artifact archive per platform.

### Release engineering (P11-12…P11-14)

- `scripts/release.ps1` now reserves the tag **before** building, exports
  `HREMOTE_BUILD_VERSION` (used by `AppInfo` so update checks see release versions),
  keeps a per-tag folder, optionally signs with `signtool`, and writes
  `BUILD.txt` + `SHA256SUMS.txt` via `scripts/build-info.mjs` while mirroring the
  flat layout for convenience.
- `BUILD.txt` records tag, package version, commit, dirty flag, ISO timestamp,
  Node/Rust/Cargo versions, signing status and `sha256 / bytes / file` per artifact.
- `scripts/stage-bundles.mjs` produces the same audit trail for CI, and
  `scripts/update-manifest.mjs` can publish a `latest.json` feed for the in-app
  updater.
- Signing is opt-in locally (`HREMOTE_SIGN_CERT`) and via repository secrets in CI;
  `docs/SIGNING.md` documents SmartScreen/EV/Azure Trusted Signing expectations and
  hash verification.
- Upgrade paths are documented and tested: config schema sanitisation/upgrades,
  SQLite v3 migration chain, newer-schema rejection and legacy imports.

### Post-release fix: rustls provider panic

A release build crashed on the first HTTPS request with
“Could not automatically determine the process-level CryptoProvider”. Both `ring`
(russh) and `aws-lc-rs` (reqwest/sspi) features are enabled in the dependency
graph, so rustls refuses to guess. `src-tauri/src/tls.rs` now installs the ring
provider once at start-up (`tls::install_crypto_provider()`), and a unit test
asserts that repeated installation is safe and leaves a default provider set.

## Deviations

1. **P11-06/P11-07** .NET TFMs and profiles do not apply; the equivalent matrix is
   OS/architecture × bundle type, documented in `docs/BUILD-MATRIX.md`.
2. **P11-08** there is no single-file/trim/ReadyToRun switch; the Rust binary is
   already a single native executable.
3. **P11-09** MSIX/Store packaging is deferred (no Windows SDK/partner identity);
   NSIS/MSI cover installation and autostart.
4. **P11-10** AppCenter/Sentry secret injection is intentionally absent; no
   telemetry is shipped. Update-feed generation uses repository variables.
5. **P11-11** the reference submodules are crates or user-supplied runners.

## Verification

- Rust: 347 tests (including the new source-error classification and config schema
  upgrade tests).
- UI: 86 Vitest tests.
- E2E: 16 Playwright tests (tab drag + full screen added).
- `scripts/build-info.mjs` and `update-manifest.mjs` were exercised against temp
  directories; `release.yml` uses the same scripts.
