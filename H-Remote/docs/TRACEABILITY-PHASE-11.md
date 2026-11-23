# Feature traceability — Phase 11

Status: **Done**, **Adapted** (different delivery model), **Deferred**.

| ID | Status | Implementation | Verification |
|---|---|---|---|
| P11-01 | Done | domain serialisation/encryption/import mapping suites | workspace tests |
| P11-02 | Done | tag operators, prefix tags, natural IP sort | `hr-domain/src/search.rs` tests |
| P11-03 | Done | grid/snap geometry coverage, bounds, no overlap | `layouts.test.ts` |
| P11-04 | Done | SQLite CRUD + source status transitions | storage/source-manager tests, `classify_source_error` |
| P11-05 | Done | first run, connect/reconnect, tab drag, full screen, launcher, SFTP | Playwright smoke spec |
| P11-06 | Adapted | Debug/Release; no .NET TFMs; OS/arch × bundle matrix | `docs/BUILD-MATRIX.md`, CI |
| P11-07 | Adapted | portable, NSIS, MSI, AppImage, DEB, DMG | `docs/BUILD-MATRIX.md`, `release.yml` |
| P11-08 | Adapted | native single binary; no trim/R2R/single-file switch | build matrix doc |
| P11-09 | Deferred | MSIX/Store packaging | documented limitation |
| P11-10 | Done | CI + tag release workflow, version extraction, artifact upload, optional signing | `.github/workflows/*.yml` |
| P11-11 | Adapted | crates pinned in `Cargo.lock`; runners supplied by user | build matrix doc |
| P11-12 | Done | per-tag folder + `BUILD.txt` + `SHA256SUMS.txt` + flat mirror | `scripts/release.ps1`, `build-info.mjs` |
| P11-13 | Done | opt-in signtool signing, timestamping, SmartScreen guidance | `docs/SIGNING.md`, release script |
| P11-14 | Done | config schema upgrade, SQLite migrations, newer-schema rejection | config/storage tests, build matrix doc |

## Deviations

- .NET target frameworks, publish profiles and MSIX do not apply to the Rust/Tauri
  build; the equivalent matrix is OS/architecture × bundle type.
- No telemetry secrets are injected (AppCenter/Sentry intentionally absent).

## Verification commands

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
npm run check && npm test
npm run test:e2e
npm run check:i18n
npm run check:traceability
node scripts/build-info.mjs --tag v0.0.0 --dir <staged-dir> --signed no
```

CI: `.github/workflows/ci.yml` (every push/PR) and
`.github/workflows/release.yml` (`v*` tags).
