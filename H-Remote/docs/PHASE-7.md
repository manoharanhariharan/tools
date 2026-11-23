# Phase 7 — Credentials & security

**Status: complete (2026-11-22) for the local vault and Windows verification.**
Credential management, inheritance/alternate launch, reachability auto-switching,
ask-password prompts, password-field controls and sensitive-action gating are
implemented. Windows Hello is replaced by the native credential prompt; other
platforms report verification as unavailable and require explicit confirmation.

## What shipped

### Credential vault

- Settings → **Credentials**: grouped by source (Local today), add/edit/delete,
  name/username/password/private-key/address-port overrides.
- Duplicate-name and duplicate-hash detection come from the repository; errors
  are shown inline.
- Deleting a credential now **detaches every server inheriting it** in the same
  transaction, preserving the server's own overrides.
- Updating a credential needs no server rewrite: sessions resolve inheritance at
  connect time and the vault reloads immediately after save.
- `databaseId` is preserved and the source selector is ready for Phase 8’s
  read-only sources.

### Inheritance & alternates

- Server editor keeps named-credential inheritance plus address/port/user/
  password/key overrides.
- Alternate credential rows support name/address/port/user/password/key and are
  normalised against the server.
- “Connect via …” actions in the catalog row, session tab menu and launcher start
  a session with a chosen alternate (address/port/credentials applied before the
  plan is built).

### Reachability & auto-switching

- `hr-platform::tcp_latency` measures TCP connect latency; `tcp_reachable` now
  delegates to it.
- `check_server_availability` probes the main address and every alternate with a
  100 ms quick test, then a 2 s retry.
- `connect_server` auto-switches to the first reachable alternate when
  `isAutoAlternateAddressSwitching` is enabled; RDP gateway connections skip the
  probe.
- `AvailabilityDialog` lists each address with status/latency, 5-second countdown,
  “Connect now”/“Try anyway”/Cancel, and a “Not able to connect” state.

### Ask password

- `askPasswordWhenConnect` prompts for username and password before connecting.
  Leaving the password empty uses the server/private-key flow. The entered values
  are passed only to that connection and never saved.

### Secondary verification and secret display

- `hr-platform::verification` uses `CredUIPromptForWindowsCredentialsW` plus
  `LogonUserW` (native Windows credential prompt; retries invalid credentials).
  Non-Windows returns `supported: false`; the UI asks for explicit confirmation.
- 30-second verification grace cache in `AppState` (`verify_sensitive_action`,
  `verification_status`).
- Gated actions: reveal/copy password, JSON/RDP export. Copy/reveal from the
  credential vault, server editor, launcher and session menus uses the same gate.
- `PasswordField` provides reveal/hide, copy, clear and never renders the stored
  password in lists/cards.
- Password/private-key mutual exclusion is enforced in the credential and server
  editors (setting one clears the other).
- `SecretString` remains the domain representation; password buffers passed to
  verification are wiped best-effort after use.

## Defaults and decisions

- Verification cache: 30 seconds, bypassable per action with `force`.
- Availability probes: 100 ms quick pass then 2 s; sequential to avoid bursting
  many sockets.
- Auto-switch only runs when the setting is enabled and no gateway is configured.
- Deleting a credential detaches rather than deletes servers.
- Non-Windows verification is explicit rather than silently “verified”.

## Deviations

1. **P7-19** Windows Hello is not wired (WinRT consent API); the native credential
   prompt is the supported fallback in this build.
2. **P7-12** the dialog is shown when no address is reachable, not on every
   connect; reachable addresses connect immediately without flashing a dialog.
3. **P7-03/P7-01** only the local source exists until Phase 8; grouping/source
   fields are ready.
4. **P7-27** the domain still uses `SecretString` rather than a zeroizing
   `SecureString`; verification buffers are wiped best-effort.
5. **P7-08/P7-09** alternate launch is wired; “connect via alternate” does not
   yet persist a per-launch choice.

## Verification

- Rust: 302 tests (credential-detach cascade, latency probe, verification result).
- UI: 75 Vitest tests (credential vault create/list).
- E2E: 14 Playwright tests (credentials tab + vault creation).
- clippy/fmt/svelte-check/i18n/traceability clean.
