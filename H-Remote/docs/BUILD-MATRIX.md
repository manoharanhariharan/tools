# Build matrix

H-Remote is a Rust/Tauri 2 application. The reference product is WPF/.NET, so its
target-framework matrix does not translate one-to-one. This document maps each
reference dimension to what the rebuild produces and how to reproduce it.

## Configurations

| Reference | H-Remote | Notes |
|---|---|---|
| Debug | `npm run tauri:dev` or `cargo build` | Devtools enabled, verbose logs |
| Release | `npm run tauri:build` | LTO thin, single binary |
| StoreDebug | — | Not implemented (no MSIX toolchain) |
| StoreRelease | — | Not implemented; installer/portable only |

## Runtime and packaging

| Dimension | Reference | H-Remote |
|---|---|---|
| TFM | net48/net6/net9/net10/net11 | N/A — one native binary per OS/arch |
| Runtime | win-x64 | `x86_64-pc-windows-msvc`, plus Linux/macOS hosts in CI |
| Packaging | framework-dependent / self-contained / MSIX | Portable exe, NSIS installer, MSI; MSIX deferred |
| Single file | `PublishSingleFile` | Native binary is already one file |
| Trimming | `PublishTrimmed` | Not applicable; no runtime trimming |
| ReadyToRun | `PublishReadyToRun` | Not applicable |
| Native libs | `IncludeNativeLibrariesForSelfExtract` | Rust statically links; WebView2 is a system dependency |

## Publish profiles → commands

| Reference profile | H-Remote command | Output |
|---|---|---|
| `x64-net100` (framework-dependent) | `npm run tauri:build -- --no-bundle` | `target/release/hremote.exe` |
| `x64-net100-self-contained` | Same; Rust binaries have no runtime to bundle | Same |
| `x64-net90/110` | N/A | Version skew is handled by Rust/WebView2, not the TFM |
| `x64-net48` | N/A | Windows 7/.NET Framework target is not supported |
| `x64-net60` | N/A | — |
| Single-file app | Default | One executable (+ installer when bundled) |

Bundle targets:

```powershell
npm run tauri:build -- --bundles nsis        # Windows setup exe
npm run tauri:build -- --bundles msi         # Windows MSI
npm run tauri:build -- --bundles appimage,deb
npm run tauri:build -- --bundles dmg
npm run tauri:build -- --no-bundle           # portable executable only
```

## Store / MSIX

MSIX requires the Windows SDK `MakeAppx`/`signtool` toolchain and a package
identity (publisher/partner center). It is intentionally deferred. The installer
and portable exe cover the same install/startup flows; autostart uses the registry
Run key.

## Dependencies that were submodules in the reference

| Reference | H-Remote |
|---|---|
| Dragablz | Svelte tab workspace |
| Shawn.Utils | Rust utility crates |
| VncSharpCore | `vnc-rs` + RFB framebuffer mapping |
| PuTTY/KiTTY payloads | External runner executables configured by the user |

All Rust dependencies are pinned in `Cargo.lock`; there are no git submodules to
initialise. Native build prerequisites on Windows are the MSVC build tools and
WebView2 (present on Windows 10/11). Linux builds additionally need ALSA headers
for the RDP audio backend (`libasound2-dev` on Debian/Ubuntu).

## Upgrade path

- **Configuration** — `hremote.json` carries `schemaVersion` and is sanitised on
  load; unknown top-level fields are preserved for forward compatibility, newer
  fields get defaults, and ranges are clamped. An older schema is upgraded to the
  current version on the next save (`older_config_schema_is_upgraded_on_sanitize`).
- **SQLite** — schema v3 migrates from any earlier version in one transaction
  chain; a database newer than the build is rejected with a clear error, and a
  wrong vault key fails with `EncryptionKeyError` instead of overwriting data
  (`migrates_an_existing_phase_zero_database`, `newer_schema_is_rejected`).
- **Imports** — JSON (current and legacy 1Remote/PRemoteM), mRemoteNG CSV, `.rdp`
  and legacy SQLite databases map into the current domain model; class versions
  are normalised on load.
- **Sources** — read-only sources with an older/foreign schema report
  `otherError`/`accessDenied` and keep their cached snapshot rather than blocking
  the local catalog.

## Reproducibility checklist

1. `npm ci` (lockfile) and `cargo build --locked`.
2. Clean tree (`BUILD.txt` records `dirty`).
3. Tag/build version exported as `HREMOTE_BUILD_VERSION`.
4. `BUILD.txt` + `SHA256SUMS.txt` written per artifact set.
5. CI runs the same matrix on Windows, Linux and macOS.
