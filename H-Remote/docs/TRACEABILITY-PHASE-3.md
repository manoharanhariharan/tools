# Feature traceability — Phase 3

Status: **Done**, **Partial** (cross-platform/engine limitation documented),
**N/A (rebuilt)** (ActiveX-specific behaviour replaced by the approved design),
**Deferred** (explicitly assigned to Phase 4+).

| ID | Status | Implementation | Verification |
|---|---|---|---|
| P3-01 | Done (rebuilt) | IronRDP embedded engine + shared-memory frames (ADR-002/003) | `connects_to_a_live_rdp_host` (ignored), engine tests |
| P3-02 | Done | dedicated `hremote-rdp` thread, `CancelToken`, `Drop` teardown | manager/shutdown tests |
| P3-03 | Done | `state.rs` `StateMachine` | `happy_path_transitions_are_allowed`, `impossible_transitions_are_rejected` |
| P3-04 | Done | `SessionManager::reconnect` new generation | `reconnect_starts_a_new_generation` |
| P3-05 | Done | `ReconnectPolicy` retry loop (5 attempts, backoff) | `failed_session_retries_and_then_connects`, `reconnect_backoff_grows_and_is_capped` |
| P3-06 | Done | `SessionOverlay` Cancel/Reconnect/Close | E2E `connects a session, shows frames and reconnects` |
| P3-07 | Done | `DisconnectReason` + IronRDP mapping | `reason_messages_and_retryability`, `termination_reasons_map_to_disconnect_reasons` |
| P3-08 | Done | `ResizeObserver` + debounced `session_resize` | sessions store/UI tests |
| P3-09 | Done | `RdpPlan` credentials/domain/destination | `fixed_mode_keeps_stored_dimensions` |
| P3-10 | Partial | load-balance info stored, not sent | plan tests |
| P3-11 | Partial | CredSSP/TLS on; public-mode/timeout not exposed | `configuration_builds_for_direct_and_gateway_plans` |
| P3-12 | Partial | bounded frames, colour depth/codecs; perf flags not exposed | `performance_maps_to_depth_and_codecs` |
| P3-13 | Partial | plan carries full-screen/multimon; UI overlay fullscreen | plan tests |
| P3-14 | Deferred (Phase 4) | connection bar/pin is tab-chrome work | `docs/PHASE-4` scope |
| P3-15 | Done (auto/fixed/stretch) | resize mode mapping + viewport sizing | `auto_resize_uses_the_viewport_and_clamps`, `fixed_mode_keeps_stored_dimensions` |
| P3-16 | Done | fixed width/height clamped | `fixed_mode_keeps_stored_dimensions` |
| P3-17 | Done | follow-system/custom 100–300 % | `scale_prefers_custom_value_and_clamps` |
| P3-18 | Partial | colour depth/codecs applied; animation/wallpaper flags not exposed | `performance_maps_to_depth_and_codecs` |
| P3-19 | Partial | multimon plan + warning; embedded uses a wide extended canvas (1–4 virtual monitors); mstsc for true per-monitor | plan warnings test; `docs/RDP-INPUT-DISPLAY.md` |
| P3-20 | Partial | primary scale factor used for desktop sizing | scale tests |
| P3-21 | Done | per-session Display dialog: fit/fixed presets/custom, 1–4 virtual monitors | `SessionPane.test.ts`; E2E display flow |
| P3-22 | Done | override suspends fit resizing; reconnect re-applies; reset restores fit size | `sessions.test.ts`/`SessionPane.test.ts` |
| P3-23 | Partial | clipboard channel enabled + clipboard events | sessions clipboard test |
| P3-24 | Partial | rdpdr channel enabled; no drive picker UI | `build_config` |
| P3-25 | Deferred (mstsc) | camera warning; external engine honours it | `unsupported_redirections_are_reported_not_silently_dropped` |
| P3-26 | Deferred (mstsc) | USB redirection not in the embedded engine | plan warnings |
| P3-27 | Partial | Ctrl+Alt+Del scancodes; full policy Phase 4 | `ctrl_alt_del_has_press_and_release_pairs` |
| P3-28 | Done (PCM) | RDPSND channel + `cpal` playback for *Play on this computer*; Opus opt-in | `remote_audio_is_only_advertised_when_redirected_locally`, `external_audio_properties_cannot_override_the_plan` |
| P3-29 | Deferred (mstsc) | microphone warning | plan warnings |
| P3-30 | Deferred (mstsc) | COM warning; fallback honours it | plan warnings |
| P3-31 | Deferred (mstsc) | printer warning; fallback honours it | plan warnings |
| P3-32 | Done | smart-card channel + flag | `build_config` |
| P3-33 | Done | gateway transport in `build_config` | `gateway_requires_a_host_and_normalises_the_port` |
| P3-34 | Partial | gateway user/password; smart-card logon deferred | gateway test |
| P3-35 | Done | `MstscBackend` + generated `.rdp` | `generated_rdp_contains_plan_fields_and_extra_settings`, `file_is_written_to_the_requested_directory` |
| P3-36 | Done | additional `.rdp` merged into property set | `additional_settings_are_parsed_and_protected_keys_rejected` |
| P3-37 | Partial | control properties → property set with denylist (no reflection) | `control_properties_map_typed_values` |
| P3-38 | N/A (rebuilt) | ActiveX device-store paths do not exist | design note |
| P3-39 | Done | Ctrl+Alt+Del command/scancodes | `ctrl_alt_del_has_press_and_release_pairs`, E2E enabled button |
| P3-40 | Partial | overlay focuses the frame surface; explicit click-to-focus Phase 4 | UI tests |
| P3-41 | N/A (rebuilt) | no ActiveX/WinForms message loop | design note |
| P3-42 | N/A (rebuilt) | no ActiveX/WinForms message loop | design note |
| P3-43 | Deferred (Phase 4) | tab/full-screen restore | Phase 4 UI |
| P3-44 | Deferred (Phase 4) | resolution change exits full screen | Phase 4 UI |
| P3-45 | Deferred (Phase 4) | reparenting between tab/full-screen windows | Phase 4 UI |
| P3-46 | Done | `session_reconnect` + retry overlay | `reconnect_starts_a_new_generation`, E2E reconnect |

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

The live-host smoke test is ignored by default:

```bash
HREMOTE_RDP_HOST=10.0.0.5:3389 HREMOTE_RDP_USER=admin HREMOTE_RDP_PASSWORD=... \
  cargo test -p hr-session --test session_flow -- --ignored
```
