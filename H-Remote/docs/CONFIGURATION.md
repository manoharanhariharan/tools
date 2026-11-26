# Configuration, profiles and file formats

## 1. Profile modes

| Mode | Base directory | Locality/logs |
|---|---|---|
| Portable | Directory containing the executable | `<base>/.locality`, `<base>/.logs` |
| AppData | User config directory + `H-Remote` (Windows `%APPDATA%\H-Remote`) | user local app data + `H-Remote` |

Resolution order:

1. `HREMOTE_PROFILE_MODE=portable|appdata` (test/portable launcher override).
2. Marker files in the executable directory or AppData base:
   `FORCE_INTO_PORTABLE_MODE`, `FORCE_INTO_APPDATA_MODE`.
   Both kinds present → startup conflict error.
3. Existing AppData profile (`hremote.json`) wins.
4. Otherwise Portable when the executable directory is writable, else AppData.

`HREMOTE_BASE_DIR` overrides the portable base directory.

Changing mode from the wizard writes the marker (restart required); it never
moves existing data automatically.

## 2. Layout

```
<base>/
  hremote.json                  global configuration
  hremote.dataSources.json      additional read-only data sources (Phase 8)
  hremote.db                    SQLite profile DB
  Protocols/<PROTOCOL>.json     per-protocol runner configuration
  .locality/
    .locality.json              window/UI/session state
    .tags.json                  pinned/ordered tags
    .connection_records.json    last-connect times + quick-connect history
    .list_view.json             sort/group/column/custom order
    .tree_view.json             folder expansion/order
    .encryption_salt            installation salt (32 bytes, base64url)
    .vault_key.json             optional Argon2id-protected master key
  .logs/
    hremote.log.<date>.md       rotating logs (14 days)
    crash-*.md                  crash reports
    error-report-*.md           saved UI reports
  .icons/, .icons/library/      generated and reusable icons
  runners/                      extracted/bundled runners (Phase 6)
```

## 3. `hremote.json`

All fields are optional; defaults are applied and ranges clamped on load/save.
Unknown top-level fields are preserved for forward compatibility.

```jsonc
{
  "schemaVersion": 1,
  "general": {
    "language": "system",                 // or en-US, de-DE, ...
    "runAtStartup": false,
    "confirmBeforeClosingSession": false,
    "showSessionIconInTitle": true,
    "tabHeader": {
      "showIcon": true,
      "showCloseButton": true,
      "showReconnectButton": false,
      "closeButtonOnLeft": false
    },
    "showNoteInList": true,
    "logLevel": "warn",                   // off|error|warn|info|debug|trace
    "copyPortWithAddress": true,
    "moveFocusToLocalOnMouseLeave": false,
    "doNotCheckForUpdates": false,
    "includePrereleaseUpdates": false,
    "updateCheckPeriodHours": 24,         // 0-168; 0 disables automatic checks
    "updateFeedUrl": null,                // JSON manifest URL
    "startMinimized": false,
    "guidanceCompleted": false,
    "skipLegacyMigration": false
  },
  "launcher": {
    "enabled": true,
    "hotkey": "Alt+M",
    "showNote": true,
    "showCredentials": false,
    "saveEnteredInfo": false,
    "matchCredential": true,
    "recentLimit": 20,
    "rememberPosition": true
  },
  "runners": {
    "runners": [
      {
        "id": "putty-ssh",
        "name": "PuTTY (SSH)",
        "protocol": "SSH",
        "kind": "external",
        "exePath": "C:\\\\Program Files\\\\PuTTY\\\\putty.exe",
        "arguments": "-ssh {user}@{host} -P {port} -pw {password}",
        "parameters": {},
        "environment": {},
        "runWithHosting": true,
        "sessionConfigPath": null,
        "installState": "missing",
        "macros": ["{host}", "{port}", "{user}", "{password}"]
      }
    ],
    "defaults": { "SSH": "putty-ssh", "Telnet": "putty-telnet", "Serial": "putty-serial" }
  },
  "keywordMatch": {
    "matchers": ["direct"],               // direct is always first
    "debounceMs": 150,                    // clamped 50-1000
    "parallel": true,
    "autoLocale": true                    // pinyin defaults for zh* locales
  },
  "theme": {
    "themeName": "Dark",
    "isCustom": false,
    "colors": { "primaryMid": "#323233", "...": "#..." },
    "fontFamily": "system-ui",
    "fontSize": 12                        // clamped 10-20
  },
  "engagement": {
    "launches": 0,
    "firstRunAt": null,
    "lastUpdateCheckAt": null,
    "lastRatingPromptAt": null,
    "doNotShowRating": false
  },
  "databaseCheckPeriod": 10,              // 0-99 seconds
  "databaseReconnectPeriod": 120,         // 0-3600 seconds
  "sqliteDatabasePath": null,             // relative to profile or absolute
  "pinnedTags": []
}
```

Colours use the reference `#AARRGGBB` notation; the UI converts them to CSS
`#RRGGBBAA`.

`databaseCheckPeriod` drives the additional-source health probe (0 disables it);
`databaseReconnectPeriod` retries failed sources (0 disables it).
`sqliteDatabasePath` is the local catalog file; Settings → Data Sources can change
it at runtime, and an empty value falls back to `<profile>/hremote.db`.

### Update manifest

`updateFeedUrl` points at a small JSON document (HTTPS, 8 s timeout, 256 KiB cap):

```json
{
  "version": "26.12.1.1",
  "notes": "Fixes and polish",
  "url": "https://example.invalid/h-remote-26.12.1.1.exe",
  "publishedAt": 1766000000,
  "prerelease": false,
  "minimumVersion": "26.10.1.1"
}
```

Versions are dotted numbers (pre-release suffixes sort before the stable release).
`prerelease` manifests are hidden unless `includePrereleaseUpdates` is set.
`minimumVersion` older than the running build marks the upgrade as breaking in the
dialog. The app only opens the download URL; it never installs silently.

## 4. Auxiliary assets

- `hremote.dataSources.json` - additional read-only sources (Phase 8):
  `{"schemaVersion": 1, "sources": [{id, name, kind, path, host, port, database,
  username, password, readOnly, enabled}]}`. `kind` is `sqlite`, `mysql` or
  `postgres`; `password` is a vault ciphertext (`datasource-password` purpose) and
  is never returned to the UI. Additional sources are always read-only; the local
  SQLite catalog is configured by `sqliteDatabasePath` instead.
- `Protocols/*.json` — one file per protocol, parsed and listed in
  Settings → Data. Schema is owned by Phase 6.
- Corrupt files are renamed to `<name>.corrupt-<millis>` and reported in the
  bootstrap payload / Data settings.

## 5. Locality files

| File | Contents |
|---|---|
| `.locality.json` | `ui.serverView`, `ui.serverSort`, column widths, main window placement (`x/y/width/height/maximized/monitorName/scaleFactor/layoutSignature`), `preferLastDisplay`, session fullscreen monitors, last tab tokens, tab layout/grid mode/tab-button flags, shared session-window placement, launcher window placement |
| `.tags.json` | `pinned[]`, `order[]` (normalised lowercase, no `#`) |
| `.connection_records.json` | `records{id: {lastConnectedAt, connectCount}}`, newest-first `quickConnect[]` (max 200) |
| `.list_view.json` | sort, group expansion, custom server order, column widths and left-to-right column order |
| `.tree_view.json` | expanded folder ids, node order |

## 5a. SQLite schema (v4)

| Table | Columns | Purpose |
|---|---|---|
| `Servers` | `Id` (ULID PK), `Protocol`, `ClassVersion`, `Json`, `UpdatedAt` | protocol-typed servers |
| `Credentials` | `Id` (ULID PK), `Name` (unique, case-insensitive), `Hash`, `Json`, `UpdatedAt` | reusable credentials |
| `Configs` | `Key` PK, `Value`, `UpdatedAt` | source config + encryption sentinel |
| `Folders` | `Id` (ULID PK), `Path` (unique JSON array), `ColorHex`, `UpdatedAt` | folder hierarchy with an optional per-folder tint |
| `TableMeta` | `TableName` PK, `UpdatedAt`, `RowCount` | cache invalidation stamps |
| `_hremote_meta` | `key` PK, `value`, `updated_at` | schema version ledger |

Secrets inside `Servers.Json` / `Credentials.Json` are replaced with
`{"$enc":"v1:<nonce>:<ciphertext>"}`; the rest of the JSON stays readable.
`Configs.EncryptionTest` verifies the active vault. A wrong key reports
`encryption_key` and disables catalog storage without modifying data.

Schema versions newer than the build are rejected. Phase 0 (v1) and Phase 1 (v2)
databases migrate to v4 automatically on first open. Folder paths are ordered
JSON arrays; renaming/moving a folder rewrites every descendant folder and server
path in the same transaction.

## 6. Atomicity and recovery

- Writes: temp file in the same directory → `write_all` → `flush` → `fsync` →
  rename; up to 5 retries (25/50/75/100 ms) for Windows sharing violations.
- Reads: missing files return defaults; parse errors quarantine the file and
  continue with defaults. The quarantine path is shown in the UI.
- Debounce: 300 ms; settings changes flush immediately (P0-78); shutdown flushes
  and joins every saver thread.

## 7. Environment variables

| Variable | Purpose |
|---|---|
| `HREMOTE_PROFILE_MODE` | `portable` or `appdata` override |
| `HREMOTE_BASE_DIR` | Portable base directory override |
| `HREMOTE_ENCRYPTION_SALT` | 32-byte base64url salt override (tests/automation) |
| `HREMOTE_VAULT_PASSPHRASE` | Enables the Argon2id key-file fallback |
| `HREMOTE_BUILD_VERSION` | Compile-time release version override used by update checks |
| `HREMOTE_SIGN_CERT` / `HREMOTE_SIGN_PASSWORD` | Release-script signing certificate (never commit) |
| `RUST_LOG` | Reserved; the configured log level is authoritative |

Environment overrides are intentionally limited to bootstrap/packaging concerns;
application settings live in `hremote.json`.

## 8. i18n keys

Code uses dotted keys (`general.openLogFolder`); Fluent catalogs use dashes
(`general-openLogFolder`). Both loaders normalise. `npm run check:i18n` fails if a
literal key used in Rust/Svelte is missing from English.
