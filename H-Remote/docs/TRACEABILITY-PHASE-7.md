# Feature traceability — Phase 7

Status: **Done**, **Partial** (platform/UI limitation), **Deferred**.

| ID | Status | Implementation | Verification |
|---|---|---|---|
| P7-01 | Done | Settings → Credentials grouped by `databaseId` | `CredentialsSettings.test.ts` |
| P7-02 | Done | add/edit/delete; repository duplicate-name/hash validation | repository credential tests |
| P7-03 | Partial | source selector present; only Local exists until Phase 8 | credential editor |
| P7-04 | Done | inheritance resolved at connect; vault reloads after save | credential/inheritance tests |
| P7-05 | Done | delete detaches inheriting servers in one transaction | `deleting_a_credential_detaches_inheriting_servers` |
| P7-06 | Done | named inheritance + overrides | domain inheritance tests; editor |
| P7-07 | Done | alternate credential rows (address/port/user/pass/key) | domain/editor tests |
| P7-08 | Done | “Connect via …” actions in row/tab/launcher | catalog/session actions |
| P7-09 | Done | alternate-specific connect applies address/credentials | `apply_alternate`; UI actions |
| P7-10 | Done | TCP probe before connect (`isPingBeforeConnect`) | `tcp_latency` tests |
| P7-11 | Done | first reachable alternate wins | `auto_switch_address` |
| P7-12 | Done | availability dialog with status/latency/countdown/cancel | `AvailabilityDialog` |
| P7-13 | Done | `check_server_availability` command is UI-free | command |
| P7-14 | Done | 100 ms quick probe then 2 s | `probe` |
| P7-15 | Done | credential normalisation inherits missing address/port | credential tests |
| P7-16 | Done | username/password prompt before connect | `sessions.connect` |
| P7-17 | Partial | overrides passed only to the connection; process argv still holds decrypted values | connect flow |
| P7-18 | Done (Windows) | native credential verification before sensitive actions | `verify_sensitive_action` |
| P7-19 | Partial | native prompt fallback; Windows Hello WinRT deferred | verification module |
| P7-20 | Done | reveal/copy password + JSON/RDP export gated | PasswordField/export gates |
| P7-21 | Done | 30 s cache in AppState | `verification_status` |
| P7-22 | Partial | blocking prompt; no animated verification ring | UI |
| P7-23 | Done | `PasswordField` reveal/hide/copy/clear | component |
| P7-24 | Done | lists/cards never render passwords | catalog UI |
| P7-25 | Done | password/private-key mutual exclusion | editors |
| P7-26 | Done | MD5 tuple hash duplicate detection | repository tests |
| P7-27 | Partial | `SecretString` + best-effort wipe; no zeroizing SecureString | domain/codec |

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
