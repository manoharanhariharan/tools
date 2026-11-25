# Architecture decisions

Each decision records the context, the choice, and the consequences. Superseding
a decision requires a new ADR entry, not a silent edit.

## ADR-001 — Tauri 2 + Svelte 5 for the shell

**Context.** 1Remote is WPF/MVVM. A Rust rebuild needs a rich catalog/editor UI,
tab/grid layouts, i18n, accessibility and strong automated testing, while keeping
the core portable.

**Decision.** Tauri 2 (stable) with a Svelte 5 + TypeScript frontend. Tauri 3 is
alpha (checked 2026-11) and is not used.

**Consequences.** The core is UI-agnostic and testable. Rendering is a system
webview, so GPU/video-heavy session content needs the Phase 3 frame-streaming
bridge. The UI is testable in a plain browser via the mock backend.

## ADR-002 — In-webview session frames over shared memory

**Context.** Native HWND embedding is Windows-only, breaks under DWM scaling and
cannot be tested headlessly. Cross-platform parity is required from v1.

**Decision.** A Rust session host owns the protocol client and streams a bounded
ring of BGRA frames through shared memory; the UI displays them on a WebGL
canvas. Input is normalised and sent back over IPC. Native `mstsc` remains a
Windows fallback for edge cases.

**Consequences.** Tabs/grid/snap layouts are pure DOM and identical on all OSes.
Latency is slightly higher than native embedding; the frame path must be measured
in Phase 3. No session infrastructure is created before Phase 3.

## ADR-003 — IronRDP for RDP

**Context.** The reference product depends on `MsTscAx.dll` (ActiveX), which is
Windows-only, unsafe and unmaintainable outside .NET.

**Decision.** IronRDP (pure Rust) for the embedded engine, with an external
`mstsc` mode for compatibility. FreeRDP FFI was rejected for unsafe FFI and
native build complexity.

**Consequences.** Memory-safe, testable, cross-platform. Some MSTSC-specific
features may need fallbacks documented per feature.

## ADR-004 — SQLx for all data sources

**Context.** Phase 1 needs SQLite; Phase 8 adds MySQL/PostgreSQL and read-only
sources.

**Decision.** `sqlx` with SQLite now, adding MySQL/PostgreSQL features in Phase
8. No ORM; explicit SQL with typed query results.

**Consequences.** One query style across sources; read-only detection uses real
write probes. Migrations live in `hr-storage` (Phase 1 expands the list).

## ADR-005 — Fluent with dotted code keys, dashed catalog IDs

**Context.** Fluent identifiers cannot contain dots, but hierarchical keys are
much easier to keep consistent across Rust and TypeScript.

**Decision.** Code uses dotted keys (`general.openLogFolder`); catalogs store
them with dashes (`general-openLogFolder`). Both the Rust and JS loaders normalise
dots to dashes. `scripts/normalize-fluent-ids.mjs` is idempotent.

**Consequences.** One catalog source for both runtimes and a key-existence check
in CI (`scripts/check-i18n-keys.mjs`). Dynamic keys still fall back at runtime.

## ADR-006 — OS keyring + Argon2id file vault instead of DPAPI

**Context.** DPAPI is Windows-only and machine-bound. Cross-platform secrets need
a portable hierarchy with a clear failure mode.

**Decision.** A random 32-byte master key in the OS keyring (Credential
Manager/Keychain/Secret Service). If unavailable, an Argon2id-protected key file
unlocked by `HREMOTE_VAULT_PASSPHRASE`. Values are AES-256-GCM with AAD binding
the installation salt and purpose. A sentinel detects wrong keys.

**Consequences.** Portable profile databases cannot be decrypted on another
machine unless the key is migrated intentionally. Secrets are unavailable (with a
clear UI state) on headless Linux without a passphrase. Windows Hello is replaced
by the OS credential prompt in Phase 7.

## ADR-007 — Link-time protocol registry instead of reflection

**Context.** Reflection-based discovery is idiomatic in .NET but absent in Rust.

**Decision.** `inventory` registrations collected at link time; descriptors carry
display metadata, category, ordering and capabilities. A validation function
detects duplicate/invalid ids.

**Consequences.** New protocols are explicit, ordered and compile-time checked.
No runtime assembly scanning.

## ADR-008 — Crash-safe JSON with a dedicated saver thread

**Context.** Configuration and locality are small but must never be left
half-written; UI bursts of updates must not cause write amplification.

**Decision.** Per-file worker thread with debounce, atomic rename, corruption
quarantine, and a `watch` channel for persisted snapshots. `flush()` is used on
shutdown and by settings writes (P0-78).

**Consequences.** Deterministic teardown (`Drop` flushes and joins). A failed write
is surfaced via `last_error` and the UI scrolls the error report; it never panics.

## ADR-009 — Local-only observability by default

**Context.** The reference product ships AppCenter/Sentry keys at build time.

**Decision.** `tracing` + rotating local logs + Markdown crash reports. No
telemetry endpoint and no build-time third-party secrets. Reports redact obvious
`key=value` secrets and are never uploaded automatically.

**Consequences.** Privacy by default; support flows require the user to attach a
report. A future opt-in telemetry adapter can implement the same tracing layer.

## ADR-010 — Capability detection, not OS forks

**Context.** Windows Hello, registry autostart, `.lnk`, HWND embedding and
working-set trimming have no direct cross-platform equivalent.

**Decision.** Advertise capabilities at runtime and use portable implementations:
keyring crate, autostart plugin (registry/LaunchAgent/XDG), shortcut writer per
OS, external windows where embedding is unavailable, `malloc_trim`/`EmptyWorkingSet`.

**Consequences.** The UI can explain what is unavailable. Windows keeps native
behaviour where it matters; Linux/macOS get first-class fallbacks.

## ADR-011 — Protocol-typed server enum

**Context.** The reference model uses a base class per protocol with virtual
fields. A single Rust struct with dozens of optional fields would allow invalid
combinations and make validation/rendering error-prone.

**Decision.** `Server` is an enum with one struct per protocol, flattened bases
for common fields, and an internal `protocol` tag for JSON.

**Consequences.** Exhaustive matching and per-protocol validation; Appendix A
fields are typed. Adding a protocol requires a new variant and registry entry.

## ADR-012 — Field-level encrypted JSON envelope

**Context.** Entire JSON columns could be encrypted, but later migrations and
future read-only sources need to inspect non-secret metadata (protocol, tags)
without the vault.

**Decision.** Seal only secret-bearing fields into `{"$enc": "v1:..."}` objects.
A sentinel in `Configs.EncryptionTest` verifies the vault. Legacy plaintext is
readable for migration and resealed on the next write.

**Consequences.** No plaintext secrets in the DB while metadata remains
queryable. A wrong key fails with `ErrorCode::EncryptionKey`; the app shell still
starts and reports the problem instead of writing unreadable data.

## ADR-013 — Table-stamp caches and dialect SQL builders

**Context.** Large catalogs should not be reloaded on every UI action, and
additional databases arrive in Phase 8.

**Decision.** Cache `Arc<Vec<Server>>`/`Arc<Vec<Credential>>` keyed by
`TableMeta.UpdatedAt`; invalidate on local writes. SQL upserts/placeholders are
generated by `SqlDialect` so MySQL/PostgreSQL can share the repository shape.

**Consequences.** Reads are cheap and external bumps force reloads. Phase 1 only
executes SQLite; dialect behavior is tested as SQL text until Phase 8.

## ADR-014 — Folders as a first-class table

**Context.** The reference format derives folders from server `TreeNodes`, so an
empty folder cannot exist, renaming is an O(n) rewrite and drag/drop has no
stable identity.

**Decision.** Schema v3 adds `Folders(Id, Path)` where `Path` is a unique JSON
array. Servers keep `TreeNodes` as their placement. Rename/move/delete rewrite
descendant folder rows and server paths in one transaction; delete moves servers
to the parent instead of deleting them.

**Consequences.** Empty folders persist, tree/drag operations are O(subtree), and
legacy tag-based folders migrate on first load. Two writes must stay in sync;
repository tests assert both sides.

## ADR-015 — Catalog search lives in the Rust domain

**Context.** Search needs tag operators, pinyin/initials matching, highlight
offsets and natural IP ordering; duplicating this in TypeScript would drift.

**Decision.** `hr-domain::search` owns query parsing, providers (direct,
initials, discrete, pinyin, pinyin-initials), character-index highlight ranges,
sorting and the folder tree. The UI sends the raw query + locality and renders
the returned hit ranges.

**Consequences.** One deterministic implementation, testable without a browser.
The mock backend mirrors it approximately for browser/E2E use.

## ADR-016 — DPAPI only for `.rdp` export

**Context.** `mstsc` reads `password 51:b:` as a DPAPI blob, but DPAPI is
Windows-only and machine-bound. The app vault already protects stored secrets.

**Decision.** Use `CryptProtectData` (with `CRYPTPROTECT_LOCAL_MACHINE`) only when
writing `.rdp` files on Windows. Elsewhere the password is omitted and the UI
shows an explicit warning; the app vault remains the portable secret store.

**Consequences.** Exports interoperate with `mstsc` on Windows. Cross-platform
`.rdp` exports require re-entering the password; this is documented in the
export warning and in `docs/SECURITY.md`.

## ADR-017 — Dedicated thread for IronRDP, seqlock frames

**Context.** IronRDP's clipboard/rdpdr futures hold native OS handles and are
deliberately `!Send`, so they cannot run on Tauri's multi-threaded runtime. The
webview also needs frames without unbounded IPC buffering.

**Decision.** Each embedded session runs its `RdpClient` on a dedicated
`hremote-rdp` thread with a current-thread Tokio runtime. Commands and events
cross a bridge channel. Frames go into one bounded latest-frame slot exposed as a
Windows named mapping (`Local\HRemoteFrame-<id>`) guarded by a seqlock; the UI
fetches the latest PNG on demand.

**Consequences.** The async manager stays `Send` and testable, while native
backends stay on their owning thread. Cancellation is cooperative and teardown
is explicit. A single frame slot means the UI can drop intermediate frames (the
right trade-off for a remote desktop, where only the newest frame matters);
Phase 4 can switch the consumer to a direct canvas/custom protocol without
changing the engine API.

## ADR-018 — One shared session window hosts every tab

**Context.** The reference creates a native tab window per session and supports
dragging tabs between windows. Our sessions render in a webview, and OS-level
window dragging between webviews is not portable.

**Decision.** Connections never replace the catalog with a session workspace.
A hidden `sessions` webview window is declared in `tauri.conf.json`
(`index.html?window=sessions`), so it is loaded during startup rather than
created from an IPC command. The first connection shows it; later connections
focus it and appear as tabs. It is borderless and renders a single combined
header (drag handle, tabs, actions, window controls) plus the grid workspace,
polls only its visible panes and stores its geometry in
`locality.session.sessionWindows`. Opening a session emits `session:focus` so
the window selects that tab; duplicate connections to an already-open server
focus the existing tab instead of adding another. Closing it hides the webview.
Closing the main catalog window exits the app because there is no tray icon.

**Consequences.** Tabs/grid always render in a window dedicated to sessions,
and the catalog remains usable while sessions are live. The shared window is a
separate JS context that reads the session list from the manager, so it stays
correct after reloads. The design keeps the door open for a future
custom-protocol/canvas transport without changing the window model.

## ADR-019 — Launcher is a persistent hidden webview

**Context.** A global hotkey must open a search surface quickly from any app,
including when the main window is hidden. Creating/destroying a webview on every
press would add startup latency and leak-adjacent lifecycle complexity.

**Decision.** Ship a `launcher` window in `tauri.conf.json` (borderless,
always-on-top, skip-taskbar, hidden), load `index.html?window=launcher`, and
toggle visibility from Rust. Closing it hides instead of destroying it. Position
comes from `locality.ui.launcherWindow` when `rememberPosition` is on, otherwise
a monitor-aware centred placement; height follows the result count.

**Consequences.** The hotkey path is a show/focus operation with no webview
startup cost. The window stays out of the taskbar and never blocks shutdown. The
launcher reads the catalog through the same commands, so there is no duplicated
catalog cache to invalidate.

## ADR-020 — External runners for terminal protocols

**Context.** Embedding PuTTY/KiTTY-class terminal engines or native window
hosting is Windows-only, fragile and not required to deliver usable SSH/Telnet/
Serial sessions. Users already rely on PuTTY, KiTTY and custom clients.

**Decision.** Represent each terminal launch as a *runner*: executable, argument
template with macros, environment and session-config path. The process backend
spawns the runner, tracks the child and reports its exit as the session
lifecycle. RemoteApp and LocalApp use the same backend; VNC uses an embedded
RFB client. Native window embedding is deferred.

**Consequences.** Cross-platform terminal support without unsafe HWND work;
runners are configurable per protocol/server and testable without a terminal.
External windows are not painted inside the tab, so the tab shows an explicit
"external window running" state instead of a frame.

## ADR-021 — Pure-Rust SFTP/FTP clients

**Context.** File transfer must work without native libssh2/OpenSSL toolchains.
`ssh2` needs CMake for vendored builds, and the environment has none. The SSH
library also has to coexist with IronRDP's pinned crypto crates.

**Decision.** Use `russh` (0.54, ring + RSA) with `russh-sftp` for SFTP and
`suppaftp` async for FTP. Pin the oldest actively usable russh line compatible
with IronRDP's `picky`/`curve25519-dalek` versions; this is why russh is 0.54
rather than 0.65.

**Consequences.** No native build tools beyond the Rust toolchain; SFTP
password/key auth and FTP work in-process. The pinned russh emits a
future-incompatibility warning that should be revisited when IronRDP updates
its crypto stack. FTPS is deferred.

## ADR-022 — Native credential verification for sensitive actions

**Context.** Passwords can be exported or copied; the reference asks for a
secondary OS verification. Windows Hello via WinRT adds a heavy dependency and
is not available on every machine. Non-Windows has no equivalent account-verifying
prompt.

**Decision.** On Windows, use `CredUIPromptForWindowsCredentialsW` plus
`LogonUserW` to verify the current account before gated actions, with a 30-second
cache. Non-Windows reports `supported: false`, and the UI requires an explicit
confirmation instead of silently allowing the action.

**Consequences.** No plaintext secret leaves the app without a user gesture and
OS verification on Windows. The verification prompt blocks the UI thread only
inside the Tauri command; the password buffer is wiped best-effort. Windows Hello
can replace the prompt later behind the same command contract.

## ADR-023 - Additional data sources are always read-only

**Context.** The reference allows extra SQLite/MySQL/PostgreSQL catalogs and
infers writability from database grants, which requires write probes (or
optimistic writes that can partially fail). Our architecture keeps one
authoritative local catalog with encrypted credentials and table stamps.

**Decision.** Keep the local SQLite catalog writable and mount every additional
source read-only regardless of grants. The UI hides or disables writes, commands
reject foreign sourceIds, imports always target local, and connections/imports
read from remote snapshots. Secondary pools are single-connection with 5 s
timeouts; a source failure keeps its cached snapshot and the watcher retries on
the configured reconnect interval.

**Consequences.** Multi-source editing is not supported, but the app never writes
to a foreign schema and never needs destructive capability probing. Read-only
sources still support search, launch, alternate credentials and export. A future
phase can add explicit writable sources behind the same manager/UI contract.

## ADR-024 - Synchronous pre-connect scripts, asynchronous post scripts

**Context.** Automation needs to abort a connection when a prerequisite fails, but
a post-disconnect hook must never delay session teardown.

**Decision.** Allocate the session id before starting the plan, run the pre-connect
script in a blocking task and abort on a non-zero exit, then start the plan with
that id. Subscribe to session state and run the post-disconnect script in the
background once the session is terminal. Both receive SESSION_ID, SERVER_ID,
SERVER_NAME, SERVER_HOST and SERVER_TAGS.

**Consequences.** The pre script can block connect for its runtime (expected), while
the post script cannot delay shutdown. File-transfer sessions are excluded until
they gain a comparable terminal event. Inline PEM keys are materialised as temp
files so they never appear on a command line.

## ADR-025 - User-configured update feed, no silent self-update

**Context.** The app is distributed as a portable exe and a setup exe; there is no
vendor hosting endpoint yet, and silent updates would bypass the OS installer.

**Decision.** Check a user-configured JSON manifest over HTTPS, compare dotted
versions with pre-release semantics, and offer a download link. Automatic checks
respect doNotCheckForUpdates and a 0-168 h period; manual checks stay available.
Breaking upgrades (running version older than minimumVersion) get an explicit
warning in the dialog.

**Consequences.** No background download or install path exists to secure. Feeds
must be configured before automatic checks do anything, and MSIX/Store update
flows remain future work.

## ADR-026 - Rating prompts are local-only and throttled

**Context.** Engagement prompts are common but must not nag or phone home.

**Decision.** Count launches once per process, prompt after 10 launches, at most
every 90 days, and provide a permanent opt-out. "Rate now" opens the feedback URL
in the browser; no analytics are sent.

**Consequences.** Prompt state lives in hremote.json; no network calls are made
unless the user opens the feedback page.

## ADR-027 - Custom protocols are external-runner descriptors

**Context.** The reference allows protocol extensions beyond the built-in set.
Embedding arbitrary terminal engines would mean a plugin ABI and native hosting
for every protocol.

**Decision.** Represent a custom protocol as a normal CustomServer with a name,
address/credentials and a JSON extraFields object. At launch, resolve an external
runner whose protocol matches the name, expose extra string fields as {field}
macros and CUSTOM_* environment variables, and reuse the existing argument/private
key pipeline. Reject empty names and built-in protocol names in validation and again
at launch.

**Consequences.** Extending a protocol is configuration rather than code, and the
protocol picker lists custom. It requires the matching runner to be installed.
Internal terminal hosting remains unsupported.

## ADR-028 - Containment virtualisation and a bounded match cache

**Context.** Large catalogs can contain thousands of servers; windowed rendering
would complicate editing, folder semantics and keyboard navigation. Repeated
identical searches also waste CPU.

**Decision.** Use CSS content-visibility: auto with intrinsic sizes for rows,
folders and cards, and lazy/async icon decoding. Cache search hits in-process for
24 hours in a 64-entry map keyed by query, providers, source and a catalog
revision; every mutation/import/source refresh bumps the revision. Catalogues above
300 servers match on a blocking task when parallel matching is enabled.

**Consequences.** DOM semantics stay intact and edits keep working; offscreen cost
is deferred by the browser. The cache is bounded and invalidates immediately on
edits; it does not persist across runs.

## ADR-029 - High-contrast preset inside the theme system

**Context.** Accessibility requires a high-contrast option without a separate
rendering path.

**Decision.** Add a tenth preset (HighContrast: black surfaces, white text,
yellow accents) to the same validated palette model, so it participates in live
preview, custom colours and persistence like every other theme.

**Consequences.** No separate contrast mode or media-query path is needed; users
select it like any preset and can still customise colours.

## ADR-030 - Mixed-key catalogs degrade per row and repair explicitly

**Context.** A profile can be copied, relinked or lose its OS keyring entry. The
database sentinel and existing rows may then have been sealed with different
keys. Failing the whole catalog on one row makes the app unusable; silently
writing a new sentinel makes the damage harder to see.

**Decision.** Reads use a lenient unseal that turns only the unreadable secret
fields into empty strings and logs the row id. `Repository::open` refuses to
create a sentinel when rows already exist. Settings → Data → **Repair secrets**
calls `repair_catalog_secrets`, which re-encrypts readable values, clears the
unreadable ones, replaces the sentinel and reopens the repository.

**Consequences.** The catalog and non-secret metadata remain usable, affected
passwords must be re-entered, and no plaintext or foreign ciphertext is copied
back into the database. Repair is explicit, reported to the user, and covered by
storage tests.

## ADR-031 - Connection failures write a diagnostic report

**Context.** A failure that works on LAN but fails over VPN needs more than the
last UI error: the remote target, session state, server configuration, network
probe and recent log lines are all relevant.

**Decision.** When a session reaches `failed`, the UI asks Rust to write a
`crash-session-*.md` report once per session id. Failures before a session is
created use `report_connection_failure`. The report contains the session
snapshot, secrets-redacted server JSON, a 3-second TCP probe, proxy/domain
environment values and the recent log tail. The user gets a notice with the
report path.

**Consequences.** Reports are shareable and safe to attach. They are not uploaded
automatically; the user decides what to share.
