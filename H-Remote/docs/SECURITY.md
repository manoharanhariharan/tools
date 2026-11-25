# Security model

## 1. Assets and boundaries

| Asset | Where it lives | Protection |
|---|---|---|
| Server/credential metadata | SQLite profile DB (Phase 1) | OS file permissions; DB encryption for secrets |
| Passwords / private keys | Encrypted values in the DB | AES-256-GCM, purpose-bound AAD |
| Master key | OS keyring, or Argon2id-protected key file | OS access control / passphrase |
| Installation salt | `.locality/.encryption_salt` | file permission 0600 on Unix; integrity binding |
| Configuration | `hremote.json` | no secrets; atomic writes |
| Logs / crash reports | `.logs/` | redaction; no environment dump |
| Session content (Phase 3) | shared-memory ring, in-process | OS process isolation; no disk persistence |

External input boundaries: CLI arguments, JSON files, URLs, imported `.rdp` /
CSV / JSON, remote protocol payloads. Every boundary validates before use.

## 2. Key hierarchy

```
random 32-byte master key
   ├── stored in OS keyring (preferred)
   └── else encrypted with Argon2id(passphrase, random salt)
         → .vault_key.json (AES-256-GCM, AAD="hremote:vault-key-file:v1")

value ciphertext = AES-256-GCM(
  master key,
  nonce=random 12 bytes,
  AAD = "hremote:v1:" + base64(salt) + ":" + purpose
)
format: v1:<nonce b64url>:<payload b64url>
```

- The **purpose** separates fields (`password`, `private-key`, `encryption-test`),
  so a ciphertext copied to another field fails authentication.
- The **salt** binds ciphertext to the installation. A DB copied to another
  installation fails closed.
- `Configs.EncryptionTest` stores a sentinel; failure produces an explicit
  `EncryptKeyError` state in Phase 1 instead of silently writing unreadable data.
- Server/credential JSON stores secrets as field-level `{"$enc": ...}` values;
  protocol metadata, tags and folder paths remain readable for migration.
- Plaintext secrets are wrapped in `Zeroizing` and dropped as soon as the
  connect pipeline no longer needs them.
- No password is ever placed in logs, crash reports or diagnostics.

### Availability fallback

| Environment | Behaviour |
|---|---|
| Normal desktop | OS keyring |
| Keyring locked/unavailable + `HREMOTE_VAULT_PASSPHRASE` | Argon2id key file |
| Headless without passphrase | Secret features disabled; UI shows the reason |
| `HREMOTE_ENCRYPTION_SALT` set | Test/portable deployments; must be 32 base64url bytes |

## 3. Verification and sensitive actions (Phase 7)

Phase 0 ships the settings and UI gates; Phase 7 adds:

- OS credential re-authentication before copying/viewing passwords, exporting
  data, or revealing private keys.
- Windows Hello where available; native credential prompt otherwise; retry until
  success/cancel.
- Clipboard auto-clear for secrets.
- Read-only source enforcement on every write path.

## 4. Input validation

- CLI: every token is parsed; unknown switches and invalid ULIDs are reported,
  not executed. `ULID:` is validated with the ULID parser.
- URLs: only `http`/`https` may be opened from the UI command.
- Crash reports: file name traversal (`..`, separators) is rejected; only
  `crash-*`/`error-report-*` inside the log directory may be read.
- Report size is capped at 1 MiB; log tail reads are capped at 2 MiB.
- Shortcut names are sanitised for cross-platform file systems.
- JSON files are schema-checked; unknown fields are preserved, invalid ranges are
  clamped, corrupt files are quarantined.
- Paths for profile/DB are resolved from the profile root; user-supplied
  relative DB paths are joined to the profile, not used raw.

## 5. Logging and crash reports

- Log level defaults to Warn; Verbose is opt-in.
- Crash details are truncated at 64 KiB and lines containing
  `password|passwd|token|secret|private_key|api_key` are redacted.
- No environment-variable dump is included (avoids leaking tokens from the
  parent environment).
- Reports are local; nothing is uploaded automatically.
- The log directory can be opened from settings for support.

## 6. Platform capabilities

| Capability | Windows | Linux | macOS |
|---|---|---|---|
| OS keyring | Credential Manager | Secret Service | Keychain |
| Credential re-auth | Hello/CredUI (P7) | PAM/desktop prompt (P7) | LocalAuthentication (P7) |
| Autostart | HKCU Run | XDG autostart | LaunchAgent |
| Shortcut | `.lnk` | `.desktop` | `.command` (bundle/alias later) |
| Window embedding | HWND possible (not used for sessions) | no | no |
| Working-set trim | `EmptyWorkingSet` | `malloc_trim` (glibc) | unsupported |

## 7. Known limitations / decisions

1. Portable DBs are machine-bound unless the key is migrated deliberately.
2. A malicious local user with the same OS account can access the keyring entry;
   this is inherent to per-user desktop managers. Full-disk encryption and OS
   account security remain the outer boundary.
3. JSON exports are unencrypted by design; the UI shows an explicit warning and
   an encrypted export format is planned for Phase 9. `.rdp` export protects the
   password with DPAPI on Windows and omits it elsewhere.
4. Session frames live in a bounded in-process/shared-memory ring and are not
   written to disk. Clipboard redirection is opt-in per server; clipboard text
   crosses the session event channel but is never logged.
5. Sensitive actions (revealing/copying passwords, JSON/RDP export) require the
   native Windows credential prompt and are cached for 30 seconds. The prompt
   asks for the **local Windows sign-in account**, not the remote connection's
   username/password. Other
   platforms report verification as unavailable and require explicit
   confirmation; Windows Hello is not wired yet.
6. Additional data sources are read-only by design and their passwords are vault
   ciphertexts in `hremote.dataSources.json`; decrypted values stay inside
   `SourceManager` and are never sent to the UI. A source credential sealed with a
   different vault key falls back to plaintext JSON parsing and is reported as an
   error rather than crashing.
7. Third-party telemetry (AppCenter/Sentry) is intentionally absent.
8. Pre/post-connect scripts execute user-provided shell commands with the user's
   privileges by design; they are never run from imported data unless a server
   actually configures them. Inline PEM private keys are written to a per-launch
   temp file (mode 0600 on Unix) and deleted on exit/cancel/spawn failure.
10. Connection failures, network-loss disconnects and failed reconnects write a
   shareable Markdown diagnostic report containing the session snapshot, a
   secrets-redacted server configuration, whether a password/private key is
   configured (never the values), a TCP probe, proxy/domain environment
   variables, an NLA/CredSSP interpretation and the recent log tail. Passwords,
   private keys and icons are removed before the report is written.

11. A copied/relinked profile can contain rows encrypted with a previous key.
   Listing treats those individual secrets as empty (never plaintext fallback) and
   logs the affected id. Settings → Data → **Repair secrets** explicitly
   re-encrypts readable values with the current key, clears the unreadable ones
   and replaces the sentinel. Repair never runs automatically.

9. Update checks fetch a user-configured HTTPS manifest; the app never downloads
   or installs automatically, only opens the advertised download URL.
10. Custom protocols execute the configured external runner with the user's
    privileges. Runner names cannot shadow built-in protocols; `extraFields`
    strings are passed as environment variables and may contain secrets, so they
    are stored with the catalog like other server fields.

## 8. Reporting a vulnerability

Do not include passwords, private keys or full profile files. Attach the redacted
crash report and the startup trace, and describe the platform and steps to
reproduce.
