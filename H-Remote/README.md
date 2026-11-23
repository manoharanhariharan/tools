# H-Remote

Cross-platform remote session manager, rebuilt from the 1Remote feature inventory
(`../1Remote/feature-rdm.md`) with a Rust core and a Tauri 2 + Svelte 5 shell.

> **Status: Phases 0–11 complete.** The application boots, resolves a portable or
> AppData profile, persists configuration/local state crash-safely, localises,
> themes, runs in the tray, registers a global hotkey, writes crash reports, and
> ships a modular settings/wizard UI. Protocols, encrypted catalog, sessions,
> tab/grid workspace, launcher, remote files, the credential vault, multiple
> read-only data sources, automation (CLI/scripts/updates/rating), extensibility
> (custom protocols, configurable matching, high-contrast theme) and the
> QA/release tooling (auditable `BUILD.txt`, CI matrix, optional signing) are
> implemented and tested. Embedded RDP sessions now forward mouse/keyboard input
> and expose a per-session display menu (fixed resolution and 1–4 virtual
> monitors); see [`docs/RDP-INPUT-DISPLAY.md`](docs/RDP-INPUT-DISPLAY.md).

## Stack

| Layer | Choice | Why |
|---|---|---|
| Shell | Tauri 2 (stable) | Small binaries, WebView2/WebKit system webview, mature plugin set. Tauri 3 is still alpha and was deliberately avoided. |
| UI | Svelte 5 + TypeScript + Vite | Fine-grained runes reactivity, small runtime, first-class TypeScript and Vitest/Playwright support. |
| Core | Rust 2024 edition, MSRV 1.85 (toolchain pinned to 1.99.1) | Memory safety, deterministic resource ownership, single shared domain core. |
| Persistence | `sqlx` (SQLite + MySQL/PostgreSQL readers) | Typed async queries; one abstraction for the writable local catalog and read-only sources. |
| Domain | `hr-domain` protocol enum, credentials, tags/folders, LocalApp args | Exhaustive protocol fields, validation, deterministic clone/JSON. |
| Config | Atomic JSON + debounced saver thread | Crash-safe, corruption-tolerant, no database required for boot. |
| Secrets | OS keyring + Argon2id/AES-256-GCM vault | Portable DPAPI replacement; no plaintext on disk. |
| i18n | Fluent (`.ftl`) shared by Rust and Svelte | Proper plural/selection support and one catalog per language. |
| RDP | IronRDP + bounded shared-memory frames + `mstsc` fallback | Pure-Rust, embeddable, memory-safe; external fallback for native-only redirections. |

Architecture details: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md). Technology
decisions: [`docs/DECISIONS.md`](docs/DECISIONS.md).

## Prerequisites

- Rust 1.99.1 (installed by `rust-toolchain.toml`; MSRV 1.85)
- Node.js 24+ and npm 11+
- Tauri prerequisites for your OS: WebView2 (Windows), `libwebkit2gtk-4.1-dev`
  and friends (Linux), Xcode command-line tools (macOS)
- A desktop session for the tray/hotkey features

## Run

```bash
npm install
npm run tauri:dev          # full desktop app (Rust + UI)
npm run dev                # UI only, in-memory mock backend
```

The UI-only mode uses the same typed IPC contract with an in-memory backend, so
it is safe for design work and browser tests.

## Test

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check

npm run check              # svelte-check
npm test                   # Vitest component/unit tests
npm run test:e2e           # Playwright against the mock backend
npm run check:i18n         # every literal translation key exists
npm run check:traceability # every Phase 0-11 feature ID is documented
```

Test strategy and counts: [`docs/TESTING.md`](docs/TESTING.md).

## Build

```bash
npm run tauri:build                  # native installer/bundle for the host OS
npm run tauri:build -- --debug       # debug bundle
```

For a traceable local release named `h-remote-v<yy.MM.dd>.<seq>.exe`:

```powershell
.\scripts\release.ps1
```

Tauri produces an installer + app bundle. A literal single-file executable is not
a Tauri distribution format; see the Phase 0 deviation note in
[`docs/PHASE-0.md`](docs/PHASE-0.md). Release naming, hashes and signing caveats:
[`docs/RELEASING.md`](docs/RELEASING.md).

## Configuration & data

Everything lives in a **profile** chosen at startup:

- **Portable** — beside the executable (default when writable).
- **AppData** — user config directory, with logs/locality in local app data.

Files: `hremote.json`, `hremote.dataSources.json`, `hremote.db`,
`Protocols/*.json`, `.locality/`, `.logs/`, `.icons/`. Marker files
`FORCE_INTO_PORTABLE_MODE` / `FORCE_INTO_APPDATA_MODE` pin the mode.

Full reference: [`docs/CONFIGURATION.md`](docs/CONFIGURATION.md).

## Security

Secrets are encrypted with AES-256-GCM; the master key lives in the OS keyring
(Credential Manager/Keychain/Secret Service) or an Argon2id-protected key file.
Crash reports redact obvious secrets, and exported diagnostics never include
credentials. See [`docs/SECURITY.md`](docs/SECURITY.md).

## Resource hygiene

Listeners, timers, saver threads, log workers, session threads and database pools
all have deterministic teardown, with tests that assert it. Session frames use a
bounded latest-frame ring, and the session view stops its frame polling when it
unmounts. Rules and checklist: [`docs/MEMORY.md`](docs/MEMORY.md).

## Repository layout

```
crates/        Rust domain/infrastructure crates (no UI dependencies)
src-tauri/     Tauri composition root, commands, tray, window chrome
ui/            Svelte 5 feature modules, typed IPC, tests
locales/       Fluent catalogs shared by both languages
docs/          Architecture, decisions, phase plan, security, testing
scripts/       i18n/traceability/release helpers
```

## Roadmap

Phase 0 is complete. The dependency-ordered plan continues with:

1. **Phase 1** — domain model, SQLite catalog, credentials, tags/folders. ✅
2. **Phase 2** — card/list/tree catalog, folders, search, editor, import/export,
   shortcuts. ✅
3. **Phase 3** — IronRDP engine, shared-memory frames, reconnect, `mstsc`
   fallback. ✅
4. **Phase 4** — tab workspace, grid/snap layouts, detached windows,
   full-screen. ✅
5. **Phase 5** — global launcher, quick connect, history. ✅
6. **Phase 6** — VNC, terminal runners, RemoteApp, LocalApp, runner settings. ✅
7. **Phase 6b** — SFTP/FTP browser and transfer queue. ✅
8. **Phase 7** — credential vault, alternates, reachability, verification. ✅
9. **Phase 8** — multiple read-only data sources, health and grouping. ✅
10. **Phase 9** — CLI/shortcuts, scripts, imports, updates, rating, tray. ✅
11. **Phase 10** — custom protocols, matcher UI, performance, accessibility. ✅
12. **Phase 11** — QA, build matrix, CI releases, signing, upgrade paths. ✅

See [`docs/TRACEABILITY.md`](docs/TRACEABILITY.md),
[`docs/TRACEABILITY-PHASE-1.md`](docs/TRACEABILITY-PHASE-1.md),
[`docs/TRACEABILITY-PHASE-2.md`](docs/TRACEABILITY-PHASE-2.md),
[`docs/TRACEABILITY-PHASE-3.md`](docs/TRACEABILITY-PHASE-3.md),
[`docs/TRACEABILITY-PHASE-4.md`](docs/TRACEABILITY-PHASE-4.md),
[`docs/TRACEABILITY-PHASE-5.md`](docs/TRACEABILITY-PHASE-5.md) and
[`docs/TRACEABILITY-PHASE-6.md`](docs/TRACEABILITY-PHASE-6.md),
[`docs/TRACEABILITY-PHASE-7.md`](docs/TRACEABILITY-PHASE-7.md),
[`docs/TRACEABILITY-PHASE-8.md`](docs/TRACEABILITY-PHASE-8.md) and
[`docs/TRACEABILITY-PHASE-9.md`](docs/TRACEABILITY-PHASE-9.md) plus
[`docs/TRACEABILITY-PHASE-10.md`](docs/TRACEABILITY-PHASE-10.md) plus
[`docs/TRACEABILITY-PHASE-11.md`](docs/TRACEABILITY-PHASE-11.md) for feature-level
status. Phase notes live in [`docs/PHASE-0.md`](docs/PHASE-0.md),
[`docs/PHASE-1.md`](docs/PHASE-1.md), [`docs/PHASE-2.md`](docs/PHASE-2.md),
[`docs/PHASE-3.md`](docs/PHASE-3.md), [`docs/PHASE-4.md`](docs/PHASE-4.md),
[`docs/PHASE-5.md`](docs/PHASE-5.md) and [`docs/PHASE-6.md`](docs/PHASE-6.md).
The file-transfer stage is documented in
[`docs/PHASE-6B.md`](docs/PHASE-6B.md). Credentials/security are documented in
[`docs/PHASE-7.md`](docs/PHASE-7.md), data sources in
[`docs/PHASE-8.md`](docs/PHASE-8.md), automation in
[`docs/PHASE-9.md`](docs/PHASE-9.md) and extensibility/polish in
[`docs/PHASE-10.md`](docs/PHASE-10.md). QA/release engineering lives in
[`docs/PHASE-11.md`](docs/PHASE-11.md), with the
[build matrix](docs/BUILD-MATRIX.md) and [signing](docs/SIGNING.md) notes.
