# Phase 3 — RDP session engine

**Status: complete (2026-11-22) for the embedded IronRDP engine and the external
`mstsc` fallback.** Session lifecycle, bounded shared-memory frames, reconnect,
display/credential mapping, gateway, Ctrl+Alt+Del and the session overlay are
implemented and tested. Windows-only ActiveX behaviour is replaced by the
approved cross-platform design; native-only redirections fall back to `mstsc`.

## What shipped

### Engine crate (`crates/hr-session`)

- **Plan (`plan.rs`)** — immutable `RdpPlan` from an RDP server: destination,
  effective credentials, domain, display resolution/scale (100–300 %), full
  screen/multi-monitor flags, resize mode (auto/stretch/fixed), redirection
  flags, audio quality, gateway endpoint/credentials, additional `.rdp` and
  control properties, performance preset (colour depth/codecs), reconnect
  policy and a human-readable warning list for settings the embedded engine
  cannot honour.
- **State machine (`state.rs`)** — NotInitialized → Initializing → Connecting →
  Connected → Disconnecting → Disconnected/Failed with reconnect transitions
  and explicit rejection of impossible moves; disconnect-reason mapping and
  retryability; camelCase events for the UI.
- **Manager (`manager.rs`)** — up to 16 concurrent sessions, session snapshots,
  broadcast events, cancel, reconnect as a new generation, resize,
  Ctrl+Alt+Del, remove, shutdown, and automatic retries (5 by default) with
  exponential backoff and `Reconnecting` events.
- **Frames (`frames.rs`, `shared.rs`)** — bounded latest-frame store (max
  4096×4096 RGBA), and a Windows named shared-memory ring (`Local\HRemoteFrame-*`)
  with a seqlock so readers never observe torn frames. Frames larger than the
  bounded maximum are rejected; readers retry while the producer writes.
- **Backend (`backend.rs`)** — IronRDP client adapter: CredSSP/TLS, clipboard,
  drive/smart-card channels, gateway transport, resize, Ctrl+Alt+Del scancodes,
  RGBA conversion and termination mapping. IronRDP's clipboard/rdpdr futures are
  `!Send`, so the client runs on a dedicated current-thread runtime thread with a
  command/event bridge; the manager stays on the async runtime.
- **`mstsc` fallback (`mstsc.rs`)** — generates a temporary `.rdp` from the same
  plan (DPAPI-protected password on Windows, omitted elsewhere with a warning),
  launches `mstsc.exe`, reports process exit and kills it on cancel.

### Tauri (`src-tauri`)

`connect_server`, `session_list`, `session_snapshot`, `session_cancel`,
`session_reconnect`, `session_remove`, `session_resize`,
`session_send_ctrl_alt_del`, `session_frame_ring` and `session_frame_image`
(PNG data URL for the webview). A `session:event` watcher forwards state/frame/
clipboard events; sessions are shut down with the app.

### UI (`ui`)

- `sessions.svelte.ts` — snapshot list, active session, event subscription via
  `CleanupRegistry`, 120 ms latest-frame polling, clipboard forwarding, cancel/
  reconnect/close/resize, interval teardown on `dispose()`.
- `SessionOverlay.svelte` — full-window session surface with session tabs,
  live status, frame image, Cancel/Reconnect/Close, Ctrl+Alt+Del, fullscreen
  toggle, and a debounced `ResizeObserver` that renegotiates the remote desktop
  size after login/resizes.
- Catalog connect actions now start RDP sessions; non-RDP protocols report that
  their engines arrive in Phase 5.
- Mock backend simulates connect/connected/frame/cancel/reconnect/remove for
  browser development and E2E.

## Defaults and decisions

- The engine always publishes the **latest** frame; there is no unbounded queue.
- Reconnect attempts: 5, 1 s initial delay, capped at 8 s.
- Auto-resize uses the viewport; fixed/stretch-use-stored dimensions clamp to
  640×480…7680×4320.
- Scale follows the system DPI unless a per-server custom value is set.
- Redirection the embedded engine cannot perform (printers, COM, cameras,
  microphone) is reported in `plan.warnings` instead of being dropped silently.
- `.rdp` export and the `mstsc` fallback share `RdpFile` serialisation and DPAPI.

## Deviations / deferred consumers

1. **P3-01** uses IronRDP + shared memory instead of `AxMsRdpClient9`; the
   approved ADR-002/ADR-003 design replaces ActiveX.
2. **P3-10/P3-11** load-balance info is persisted but not sent; public-mode,
   Windows-key handling, focus-on-connect and an explicit connection timeout are
   not exposed by `ironrdp-client 0.1`. CredSSP/TLS are always on.
3. **P3-12/P3-18** performance presets map colour depth and codecs; wallpaper/
   animation/desktop-composition flags are not exposed by the IronRDP client
   configuration, so only the observable depth/codec subset is applied.
4. **P3-13/P3-14/P3-19/P3-20/P3-21/P3-22** full-screen policy, connection bar,
   per-session display menu and multi-monitor canvas layout are Phase 4 UI work.
   The engine accepts the resolution and multi-monitor plan today.
5. **P3-24/P3-25/P3-26/P3-29/P3-30/P3-31** drive/rdpdr and smart-card channels are
   enabled; camera, USB, microphone, COM and printer redirection require native
   backends or the `mstsc` fallback and are surfaced as warnings.
6. **P3-27** key-combination policy is limited to Ctrl+Alt+Del; full keyboard
   redirection policy is Phase 4.
7. **P3-37** control properties are applied as a generic property set with a
   denylist for identity/secret keys; there is no ActiveX reflection.
8. **P3-38/P3-41/P3-42/P3-43/P3-44/P3-45** `DeviceStoreDirect`, WM_GETOBJECT,
   WM_MOUSEACTIVATE, reparenting and connection-bar restore are ActiveX/WPF
   concerns with no direct equivalent; tab/full-screen reparenting is Phase 4.
9. **Audio** is embedded: `ironrdp-client/sound` is enabled and a vendored
   `ironrdp-rdpsnd-native` defaults to PCM-only to avoid the bundled libopus
   CMake toolchain. Audio is advertised only for *Play on this computer*; the
   external engine still honours all audio modes.

## Acceptance evidence

| Requirement | Evidence |
|---|---|
| Lifecycle and cancel | `manager::tests::session_runs_and_can_be_cancelled`, integration `lifecycle_publishes_frames_and_cancels_cleanly` |
| Auto-reconnect | `failed_session_retries_and_then_connects` |
| Reconnect command | `reconnect_starts_a_new_generation` |
| Bounded frames | `frames::tests::*`, `readers_never_observe_torn_frames` |
| Shared-memory ring | `shared_ring_round_trips_between_handles`, `transport_reports_the_ring_name_on_windows` |
| Display/scale mapping | `plan::tests::*` |
| Gateway/settings | `gateway_requires_a_host_and_normalises_the_port`, `additional_settings_are_parsed_and_protected_keys_rejected` |
| Ctrl+Alt+Del | `ctrl_alt_del_has_press_and_release_pairs` |
| `mstsc` fallback | `generated_rdp_contains_plan_fields_and_extra_settings`, `password_is_dpapi_protected` |
| UI session flow | `ui/src/lib/state/sessions.test.ts`, E2E `connects a session, shows frames and reconnects` |
| Live host (manual) | `connects_to_a_live_rdp_host` (ignored; needs `HREMOTE_RDP_*`) |

## Verification snapshot

- Rust: 271 tests (39 engine unit + 3 engine integration, plus Phases 0–2),
  clippy `--workspace --all-targets -D warnings`, rustfmt.
- UI: 51 Vitest tests, svelte-check 0 errors/0 warnings.
- E2E: 9 Playwright tests.
- Traceability: P0/P1/P2/P3 ID coverage.

## Next phase

Phase 4 builds the tab/grid workspace on top of the session manager: tabs,
split/grid layouts, drag-out windows, per-session display menu, and real
full-screen/reparent behaviour.
