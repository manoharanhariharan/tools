<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { SessionCursor, SessionSnapshot } from "../../ipc/contracts";
  import { sessions } from "../../state/sessions.svelte";
  import Button from "../../components/Button.svelte";
  import { containRect, surfaceCoordinates, x11Buttons, x11Keycode } from "./vnc";
  import FileBrowser from "./FileBrowser.svelte";

  interface Props {
    session: SessionSnapshot;
    frame: string | null;
    onfocus: () => void;
    onclose: () => void;
  }
  let { session, frame, onfocus, onclose }: Props = $props();

  let stage = $state<HTMLDivElement | null>(null);
  let canvas = $state<HTMLCanvasElement | null>(null);
  let stageWidth = $state(0);
  let stageHeight = $state(0);
  let frameToken = 0;
  let resizeTimer: ReturnType<typeof setTimeout> | undefined;
  let observer: ResizeObserver | undefined;

  const connected = $derived(session.state === "connected");
  const isVnc = $derived(session.backend === "vnc");
  const isRdp = $derived(session.backend === "ironRdp");
  const interactive = $derived((isVnc || isRdp) && connected);
  // Size the canvas explicitly to the contained frame instead of relying on
  // object-fit inside a grid track: the intrinsic canvas size used to make the
  // track overflow, cropping the bottom of the remote desktop and shifting the
  // pointer mapping origin.
  const fittedFrame = $derived.by(() => {
    const box = containRect(
      stageWidth,
      stageHeight,
      session.width,
      session.height,
    );
    return box
      ? {
          width: Math.max(1, Math.floor(box.width)),
          height: Math.max(1, Math.floor(box.height)),
        }
      : null;
  });
  const isProcess = $derived(
    session.backend === "terminal" ||
      session.backend === "localApp" ||
      session.backend === "remoteApp",
  );
  const isFile = $derived(session.backend === "fileTransfer");
  const terminal = $derived(
    session.state === "disconnected" || session.state === "failed",
  );
  const cursorStyle = $derived.by(() => {
    if (isVnc) {
      return "crosshair";
    }
    if (!isRdp) {
      return undefined;
    }
    return pointerCss(sessions.cursorFor(session.id));
  });

  function pointerCss(cursor: SessionCursor | undefined): string {
    if (!cursor || cursor.kind === "default") {
      return "default";
    }
    if (cursor.kind === "hidden") {
      return "none";
    }
    return `url("${cursor.dataUrl}") ${cursor.hotspotX} ${cursor.hotspotY}, default`;
  }

  let pendingMove: { x: number; y: number } | null = null;
  let moveFrame: number | null = null;

  onMount(() => {
    observer = new ResizeObserver(() => {
      const rect = stage?.getBoundingClientRect();
      if (rect) {
        stageWidth = rect.width;
        stageHeight = rect.height;
      }
      if (resizeTimer) {
        clearTimeout(resizeTimer);
      }
      resizeTimer = setTimeout(() => {
        if (!rect || sessions.hasDisplayOverride(session.id)) {
          return;
        }
        if (connected) {
          sessions.recordFitSize(session.id, rect.width, rect.height);
          void sessions.resize(session.id, rect.width, rect.height);
        } else {
          sessions.reconnectIfLost(session.id);
        }
      }, 300);
    });
    if (stage) {
      observer.observe(stage);
      const rect = stage.getBoundingClientRect();
      stageWidth = rect.width;
      stageHeight = rect.height;
    }
  });

  onDestroy(() => {
    if (resizeTimer) {
      clearTimeout(resizeTimer);
    }
    if (moveFrame !== null) {
      cancelAnimationFrame(moveFrame);
      moveFrame = null;
    }
    observer?.disconnect();
  });

  // Decode off-screen and blit into a stable canvas. Swapping <img> src for every
  // frame (150 ms polling) caused visible flicker while the mouse moved.
  $effect(() => {
    const source = frame;
    const target = canvas;
    const token = ++frameToken;
    if (!target) {
      return;
    }
    const context = target.getContext("2d");
    if (!source) {
      context?.clearRect(0, 0, target.width, target.height);
      return;
    }
    const image = new Image();
    image.decoding = "async";
    image.onload = () => {
      if (token !== frameToken) {
        return;
      }
      if (target.width !== image.naturalWidth && image.naturalWidth > 0) {
        target.width = image.naturalWidth;
      }
      if (target.height !== image.naturalHeight && image.naturalHeight > 0) {
        target.height = image.naturalHeight;
      }
      context?.drawImage(image, 0, 0, target.width, target.height);
    };
    image.src = source;
    return () => {
      image.onload = null;
    };
  });

  function sendMove(x: number, y: number): void {
    pendingMove = { x, y };
    if (moveFrame !== null) {
      return;
    }
    moveFrame = requestAnimationFrame(() => {
      moveFrame = null;
      const next = pendingMove;
      pendingMove = null;
      if (next) {
        void sessions.rdpPointerMove(session.id, next.x, next.y);
      }
    });
  }

  function isStageTarget(event: PointerEvent): boolean {
    const target = event.target as HTMLElement | null;
    return (
      target === event.currentTarget ||
      target?.tagName === "IMG" ||
      target?.tagName === "CANVAS"
    );
  }

  /**
   * The canvas is explicitly sized to the contained frame, so using its rect
   * makes pointer mapping exact and independent of letterbox rounding. The
   * stage is only a fallback for the first frame before the canvas mounts.
   */
  function interactionRect(): DOMRect | null {
    const rect = canvas?.getBoundingClientRect();
    if (rect && rect.width > 0 && rect.height > 0) {
      return rect;
    }
    return stage?.getBoundingClientRect() ?? null;
  }

  /**
   * Remote coordinates are mapped against the server-negotiated desktop size.
   * The canvas bitmap may still be the browser's 300x150 default before the
   * first image decodes, so it must not be used as the coordinate space.
   */
  function remoteFrameSize(): { width: number; height: number } {
    return { width: session.width, height: session.height };
  }

  function pointer(event: PointerEvent): void {
    if (!interactive || !stage || !isStageTarget(event)) {
      return;
    }
    const rect = interactionRect();
    if (!rect) {
      return;
    }
    const size = remoteFrameSize();
    const coordinates = surfaceCoordinates(
      event,
      rect,
      size.width,
      size.height,
    );
    if (!coordinates) {
      return;
    }
    if (isVnc) {
      void sessions.vncPointer(
        session.id,
        coordinates.x,
        coordinates.y,
        x11Buttons(event.buttons),
      );
    } else {
      sendMove(coordinates.x, coordinates.y);
    }
  }

  function pointerButton(event: PointerEvent, down: boolean): void {
    if (!isRdp || !connected || !stage || !isStageTarget(event)) {
      return;
    }
    const rect = interactionRect();
    if (!rect) {
      return;
    }
    const size = remoteFrameSize();
    const coordinates = surfaceCoordinates(
      event,
      rect,
      size.width,
      size.height,
    );
    if (!coordinates) {
      return;
    }
    void sessions.rdpPointerButton(
      session.id,
      coordinates.x,
      coordinates.y,
      event.button,
      down,
    );
  }

  function wheel(event: WheelEvent): void {
    if (!isRdp || !connected || !stage) {
      return;
    }
    event.preventDefault();
    const rect = interactionRect();
    if (!rect) {
      return;
    }
    const size = remoteFrameSize();
    const coordinates = surfaceCoordinates(
      event,
      rect,
      size.width,
      size.height,
    );
    if (!coordinates) {
      return;
    }
    const notches = Math.max(
      1,
      Math.min(10, Math.round(Math.abs(event.deltaY) / 100)),
    );
    const units = (event.deltaY < 0 ? 120 : -120) * notches;
    void sessions.rdpWheel(
      session.id,
      coordinates.x,
      coordinates.y,
      true,
      units,
    );
  }

  function key(event: KeyboardEvent, down: boolean): void {
    if (!interactive) {
      return;
    }
    if (isVnc) {
      const keycode = x11Keycode(event.key);
      if (keycode === null) {
        return;
      }
      event.preventDefault();
      void sessions.vncKey(session.id, keycode, down);
      return;
    }
    if (event.isComposing) {
      return;
    }
    event.preventDefault();
    void sessions.rdpKey(session.id, event.code, event.key, down);
  }

  function label(): string {
    switch (session.state) {
      case "notInitialized":
        return "Preparing";
      case "initializing":
        return "Initialising";
      case "connecting":
        return "Connecting";
      case "reconnecting":
        return `Reconnecting (${session.attempt}/${session.maxAttempts})`;
      case "connected":
        return `Connected · ${session.width}×${session.height}`;
      case "disconnecting":
        return "Disconnecting";
      case "disconnected":
        return session.message ?? "Disconnected";
      case "failed":
        return session.message ?? "Connection failed";
      default:
        return "Session";
    }
  }
</script>

<section
  class="pane"
  aria-label={`${session.serverName} session`}
  onfocusin={onfocus}
  onpointerdown={onfocus}
>
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div
    class="stage"
    class:vnc={isVnc}
    class:rdp={isRdp}
    class:files={isFile}
    bind:this={stage}
    role={interactive ? "application" : undefined}
    aria-label={isVnc ? "VNC desktop" : isRdp ? "RDP desktop" : undefined}
    tabindex={interactive ? 0 : undefined}
    style:cursor={cursorStyle}
    onpointermove={(event) => pointer(event)}
    onpointerdown={(event) => {
      if (!isStageTarget(event)) {
        return;
      }
      stage?.focus();
      stage?.setPointerCapture?.(event.pointerId);
      if (isVnc) {
        pointer(event);
      } else {
        pointerButton(event, true);
      }
    }}
    onpointerup={(event) => {
      if (!isStageTarget(event)) {
        return;
      }
      stage?.releasePointerCapture?.(event.pointerId);
      if (isVnc) {
        pointer(event);
      } else {
        pointerButton(event, false);
      }
    }}
    onpointercancel={(event) => {
      if (isRdp && isStageTarget(event)) {
        pointerButton(event, false);
      }
    }}
    onwheel={(event) => wheel(event)}
    onkeydown={(event) => key(event, true)}
    onkeyup={(event) => key(event, false)}
    oncontextmenu={(event) => {
      if (isRdp) {
        event.preventDefault();
      }
    }}
  >
    {#if isFile}
      <FileBrowser sessionId={session.id} serverName={session.serverName} />
    {:else if connected && !isProcess}
      <canvas
        class="frame"
        bind:this={canvas}
        style={fittedFrame
          ? `width:${fittedFrame.width}px;height:${fittedFrame.height}px;`
          : ""}
        aria-hidden="true"
        draggable="false"
      ></canvas>
    {:else if isProcess && connected}
      <div class="placeholder">
        <p class="process-title">{session.serverName}</p>
        <p>External window running.</p>
        <p class="hint">Close the external window to end this session.</p>
      </div>
    {:else}
      <div class="placeholder">
        <div class="pulse" class:busy={!terminal}></div>
        <p>{label()}</p>
        {#if session.state === "reconnecting"}
          <div class="row">
            <Button size="sm" variant="secondary" onclick={() => void sessions.cancel(session.id)}>
              Cancel
            </Button>
          </div>
        {:else if terminal}
          <div class="row">
            <Button size="sm" variant="primary" onclick={() => void sessions.reconnect(session.id)}>
              Reconnect
            </Button>
            <Button size="sm" variant="secondary" onclick={onclose}>Close</Button>
          </div>
        {/if}
      </div>
    {/if}

    {#if session.state === "failed" && !terminal}
      <div class="error" role="alert">{label()}</div>
    {/if}
  </div>
</section>

<style>
  .pane {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 7px;
    overflow: hidden;
    background: #0b1520;
  }

  .pane:focus-within {
    border-color: rgba(83, 158, 226, 0.65);
  }

  .stage {
    position: relative;
    flex: 1;
    min-height: 0;
    min-width: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    background: #05090d;
  }

  .frame {
    display: block;
    flex: none;
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
    pointer-events: none;
    user-select: none;
  }

  .stage.vnc {
    cursor: crosshair;
  }

  .stage.rdp {
    cursor: default;
  }

  .stage.files {
    display: flex;
    place-items: stretch;
    align-items: stretch;
  }

  .stage.vnc:focus-visible,
  .stage.rdp:focus-visible {
    outline: 2px solid var(--hr-accent-mid);
    outline-offset: -2px;
  }

  .process-title {
    color: #e6f0fa;
    font-weight: 600;
  }

  .hint {
    font-size: 10px;
    color: #8aa3b8;
  }

  .placeholder {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 16px;
    text-align: center;
    color: #b7c9d9;
    font-size: 12px;
  }

  .pulse {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    border: 2px solid rgba(143, 208, 255, 0.2);
    border-top-color: #8fd0ff;
  }

  .pulse.busy {
    animation: spin 900ms linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .row {
    display: flex;
    gap: 6px;
  }

  .error {
    position: absolute;
    inset: auto 8px 8px;
    padding: 6px 8px;
    border-radius: 5px;
    background: rgba(208, 90, 90, 0.18);
    color: #f0b4b4;
    font-size: 11px;
  }
</style>
