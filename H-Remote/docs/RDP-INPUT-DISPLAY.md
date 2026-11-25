# RDP input and display controls

This page documents the embedded IronRDP interaction path and the per-session
display menu added after the initial roadmap.

## Input forwarding (why clicks were ignored)

Earlier phases rendered IronRDP frames but only wired pointer/keyboard events for
VNC, so an embedded RDP session behaved like a static image. The input path is now:

1. `SessionPane.svelte` listens for pointer/key/wheel events only when the event
   target is the stage or frame image (toolbar buttons keep working).
2. Coordinates are mapped through `surfaceCoordinates` (letterbox-aware, same as
   VNC) to the current remote resolution.
3. The events are sent to `sessions.rdpKey` / `rdpPointerMove` /
   `rdpPointerButton` / `rdpWheel`, which call the Tauri commands
   `session_rdp_*`.
4. `hr-session` maps DOM codes/buttons to `ironrdp_input::Operation`s and applies
   them to a per-session `Database` in the IronRDP bridge. Non-empty batches are
   sent as fast-path input events (`RdpInputEvent::FastPath`).
5. Pointer buttons and wheel events carry the current x/y, so a click without a
   prior move still lands under the cursor.

Key mapping (`crates/hr-session/src/rdp_input.rs`):

- Physical `KeyboardEvent.code` → PS/2 set-1 scan codes (letters, digits,
  punctuation, modifiers, function keys, navigation and numpad, including
  extended keys).
- Unmapped printable keys fall back to Unicode keyboard events; dead keys and
  multi-character keys are ignored.
- Mouse: left/middle/right/X1/X2 through `MouseButton::from_web_button`; vertical
  wheel in ±120 units per notch.
- Pointer movement is coalesced to one animation frame in the UI to avoid IPC
  floods.

Tests: `rdp_input` unit tests, `SessionPane.test.ts` (button/key/wheel dispatch,
canvas-rect mapping, cursor style), `manager::tests::connected_is_announced_once_for_many_frames`
(no state flood), `state::tests` (camelCase event fields and cursor payloads),
`sessions.test.ts` (frame-refresh coalescing and retained frames), `backend::tests`
(audio, compression, wide-desktop config and cursor bitmap encoding), and the
Playwright tab/full-screen/frame-containment flow.

## Display menu

RDP sessions have a **Display** button in the workspace bar and a **Display…** tab
context-menu entry:

- **Fit to window** keeps the current remote pixel size and only scales the
  canvas into the pane. Auto-resize stays suspended until a fixed resolution is
  applied, so fitting never changes the server-side resolution.
- **Fixed resolution** presets (1024×768 … 3840×2160) or custom width/height,
  clamped to 640–3840 × 480–2160 per monitor.
- **Virtual monitors** 1–4 multiplies the width (`width × monitors`) and requests
  that larger desktop from the server, e.g. two 1920×1080 monitors become a
  3840×1080 canvas. Requests are capped at the protocol limit of 8192 px wide
  (for example 3840-wide screens allow at most two).
- While an override is active or fit-to-window is selected, automatic resizing is
  suspended; reconnecting reapplies a fixed override.
- Live changes use IronRDP `RdpInputEvent::Resize` (Display Control).

## Rendering, resizing and window controls

- Frames are decoded off-screen and blitted to a stable `<canvas>` instead of
  swapping `<img src>` every poll. The canvas is explicitly sized to the
  contained frame (`containRect`) instead of relying on `object-fit` inside an
  auto grid track, which used to overflow the stage, crop the bottom of the
  desktop and shift pointer coordinates.
- Pointer coordinates are derived from the canvas's actual bounding rect, so the
  host cursor and the remote pointer share one origin and scale. Clicks in the
  letterbox area are ignored rather than sent to the wrong remote position.
- Remote cursor shapes are mirrored. IronRDP advertises pointer support but uses
  the accelerated target (`pointer_software_rendering = false`), so it emits
  `PointerBitmap`/`PointerHidden`/`PointerDefault` instead of baking a lagging
  sprite into the frame. The backend converts the bitmap to a PNG data URL and
  the UI applies it as a CSS cursor with the RDP hotspot. The local OS cursor
  remains the single position source, so click coordinates stay exact.
- Frame delivery is gap-tolerant: the canvas stays mounted for the whole
  connection, a failed poll keeps the last good frame, and unchanged ring
  sequences (`since`/`seq`) skip both the PNG encode and the canvas redraw. This
  removes the periodic/enter-exit flashes caused by clearing the frame.
- Session events are serialized with `rename_all_fields = "camelCase"`. The
  regression test asserts `sessionId` on frame events; without it the UI silently
  ignored frame sizes and kept mapping the pointer against the initial viewport
  (the cause of the resize-dependent mouse drift).
- The per-pane header was removed: the workspace tab strip is the single header,
  and the tab's status dot carries the connection state. The session window,
  workspace bar, panes, tabs and file browser use the same theme CSS variables as
  the main window, including live theme changes from `config:changed`.
- Frame events are coalesced: at most one PNG fetch is in flight per session, and
  bursts collapse into the next fetch. The 250 ms interval is only a safety net
  for dropped events, so mouse-heavy sessions no longer queue dozens of encodes.
- PNG encoding uses the fast compression/filter settings; it is the only
  per-frame backend cost when a frame changes.
- A completed connection is announced once. Frame events update only the
  snapshot width/height (when they actually change) and never re-emit
  `Connected`, so the UI no longer re-renders on every frame.
- Window resizes trigger a debounced auto-resize when the session is connected
  and no fixed display override is active. If a session dropped unexpectedly, a
  resize also retries the connection (up to two attempts).
- Sessions that end with `networkLoss`/`noInfo` auto-reconnect after 1.5 s (two
  attempts, with a notice); explicit Disconnect/Close never auto-reconnects.
- The workspace bar includes a **Minimize** button (works in full-screen mode too)
  next to Grid/Display.

## Audio

- The embedded engine enables IronRDP's `sound` feature and attaches the RDPSND
  channel when the server's **Audio** setting is *Play on this computer*
  (`redirectToLocal`). *Leave on the remote computer* and *Do not play* do not
  advertise audio playback, so the server keeps or drops the sound itself.
- Audio is played through the native `cpal` backend. PCM is decoded by default;
  Opus is not compiled in because the vendored `ironrdp-rdpsnd-native` copy
  disables its bundled-CMake feature to keep builds toolchain-independent.
- The setting is available in the server editor and is applied after imported
  `.rdp` properties, so the plan stays authoritative.

## Handshake and compression notes

- Bulk (MPPC/XCRUSH) compression is deliberately disabled. `ironrdp-client` 0.1
  rebuilds the FastPath processor with `bulk_decompressor: None` during a
  Deactivation-Reactivation (any display resize), which drops compressed updates
  and logs *"Received compressed FastPath data but no decompressor is
  configured"*. Bitmap compression, RemoteFX/QOI and RDP graphics remain
  enabled; only FastPath bulk compression is skipped.
- The client advertises a current Windows build (`22621`) and the proper
  `C:\Windows\System32` client directory so the server negotiates modern
  capability sets.
- The `High` display preset requests `remotefx:on`; IronRDP parses codec names
  case-sensitively, so the display label is never forwarded verbatim.

### Teardown and retry behaviour

Input is only sent while the session state is `connected`. The backend delivery
path (`SessionManager::send_input`) is best-effort: if the session ended or is
between retries, the command is dropped instead of surfacing an
`unsupported operation: the session is no longer accepting input` error. Frame
events also update the snapshot width/height so coordinates follow the
server-negotiated resolution rather than the initial viewport request.

### Limitation

IronRDP's client currently hard-codes a single monitor in its GCC layout, so the
embedded engine exposes virtual monitors as **one wide remote desktop** (Windows
sees a single ultrawide display). This is why 3–4 “monitors” feel unstable:
there is no real per-monitor topology, DPI or taskbar awareness, and a width
above 8192 px is rejected/clamped by the Display Control channel. The UI now
caps the multiplier to 8192 px and labels the options as a wide desktop. For
true per-monitor layout and taskbar/DPI behaviour, enable **Launch with external
mstsc** on the server.
