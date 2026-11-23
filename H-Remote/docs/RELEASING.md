# Releasing H-Remote

## Naming and audit convention

Every local release gets a traceable tag derived from the build date and a daily
sequence:

```
release/v<yy.MM.dd>.<seq>/h-remote-v<yy.MM.dd>.<seq>.exe          portable
release/v<yy.MM.dd>.<seq>/h-remote-v<yy.MM.dd>.<seq>-setup.exe    installer
release/v<yy.MM.dd>.<seq>/BUILD.txt                              timestamp, commit, SHA-256
release/v<yy.MM.dd>.<seq>/SHA256SUMS.txt
```

Example: `v26.11.22.1` is the first build on 2026-11-22. The same files are also
copied to `release/` for convenience, while the tag folder is the auditable
artifact set. `release/RELEASE-<tag>.txt` and `release/SHA256SUMS-<tag>.txt` mirror
the metadata at the top level.

The build pipeline sets `HREMOTE_BUILD_VERSION` from the tag, so the running app
reports the release version (used by update checks) rather than `0.1.0`.

`release/` and `release-build.log` are gitignored. The portable exe is copied to
`publish/` and committed (with its `SHA256SUMS.txt` and `BUILD.txt`) so released
builds persist in the repository; the installer stays in the ignored
`release/<tag>/` audit folder.

## One command

```powershell
cd tools\H-Remote
.\scripts\release.ps1
```

What it does:

1. Refuses to run on a dirty tree (override with `-AllowDirty`).
2. Reserves the next `v<date>.<seq>` tag and exports it as the build version.
   The sequence skips both an existing flat artifact and an existing
   `release/v<date>.<seq>/` folder, so an earlier release can never be
   overwritten when the flat copy has been cleaned up.
3. Runs `npm run tauri:build -- --bundles nsis`.
4. Copies the portable exe and installer into `release/<tag>/`.
5. Signs both when `HREMOTE_SIGN_CERT` is set (see `docs/SIGNING.md`).
6. Writes `BUILD.txt` + `SHA256SUMS.txt` for the tag folder.
7. Mirrors the artifacts and hashes into `release/`.
8. (Manual publish) Copy the portable exe to `publish/`, update
   `publish/SHA256SUMS.txt`, then commit/push `publish/`.

Useful variants:

```powershell
.\scripts\release.ps1 -Bundles all       # MSI + NSIS
.\scripts\release.ps1 -Bundles none      # portable exe only, no installer
.\scripts\release.ps1 -NoBuild           # rename/hash an existing build
.\scripts\release.ps1 -AllowDirty        # local experiment build
$env:HREMOTE_SIGN_CERT = "C:\certs\codesign.pfx"
$env:HREMOTE_SIGN_PASSWORD = "..."       # never commit this
.\scripts\release.ps1 -Bundles all
```

`BUILD.txt` is written by `scripts/build-info.mjs` and contains the tag, package
version, commit, dirty flag, ISO timestamp, Node/Rust/Cargo versions, signing
status and one `sha256  bytes  file` line per artifact. CI uses
`scripts/stage-bundles.mjs` to produce the same file.

## Build matrix

See [`BUILD-MATRIX.md`](BUILD-MATRIX.md) for the mapping between the reference
.NET flavors and the Tauri targets, the commands for each bundle and what is
deferred (MSIX/Store). In short:

| Reference flavor | H-Remote equivalent |
|---|---|
| Debug | `npm run tauri:dev` / `cargo build` |
| Release (single file) | `npm run tauri:build -- --no-bundle` |
| Release installer | `--bundles nsis` (Windows) / `msi` / `appimage,deb` / `dmg` |
| Store/MSIX | Deferred; portable/installer only |
| net48/net6/net9/net11 | N/A (Rust binary; WebView2 is the system prerequisite) |

## CI

`.github/workflows/ci.yml` runs fmt/clippy/tests, svelte-check, Vitest, i18n,
traceability, production build and Playwright on every push/PR.

`.github/workflows/release.yml` runs on `v*` tags (or manually) on Windows, Linux
and macOS:

1. resolves `HREMOTE_BUILD_VERSION` from the tag,
2. builds the platform bundles,
3. signs Windows artifacts when `WINDOWS_CERT_PFX_BASE64` /
   `WINDOWS_CERT_PASSWORD` secrets are configured,
4. stages the artifacts, writes `BUILD.txt`, and optionally generates
   `latest.json` for the in-app updater when `UPDATE_DOWNLOAD_BASE_URL` is set,
5. uploads one artifact archive per platform.

## Before releasing

1. Run the verification gate: `npm run verify`.
2. Commit everything so `BUILD.txt` points at a clean commit.
3. Build with the script (or push a tag for CI).
4. Attach the tag-folder contents (or the CI artifact) to the download page.
5. Publish `latest.json` alongside the binaries if automatic update checks are
   enabled.

## Artifact behaviour

- The **portable exe** uses portable mode when its folder is writable
  (`hremote.json`, `.locality/`, `.logs/`, `hremote.db` beside it), otherwise
  `%APPDATA%`.
- The **installer** installs under the user profile and therefore runs in
  AppData mode.
- WebView2 is a system dependency; Windows 10/11 normally has it.
- Code signing and SmartScreen expectations are documented in
  [`SIGNING.md`](SIGNING.md).
- Linux/macOS bundles are produced on their own hosts
  (`target/release/bundle/deb|appimage|dmg|app`); the tag naming stays the same.

## Current known limitations

- MSIX/Store packaging is not implemented; use the NSIS/MSI installer.
- Native terminal window hosting is deferred; runners open external windows.
- RDP uses IronRDP with an `mstsc` fallback; VNC is embedded.
