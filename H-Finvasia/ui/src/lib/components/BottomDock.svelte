<script lang="ts">
  import { onMount } from "svelte";
  import Positions from "../features/positions/Positions.svelte";
  import Orders from "../features/orders/Orders.svelte";

  const STORAGE_KEY = "fv.bottomDock";
  const MIN_HEIGHT = 140;
  const DEFAULT_HEIGHT = 260;

  let open = $state(false);
  let height = $state(DEFAULT_HEIGHT);
  let split = $state(0.5);
  let panelsEl = $state<HTMLDivElement | null>(null);
  let drag = $state<
    | {
        kind: "height" | "split";
        startY: number;
        startX: number;
        startHeight: number;
        startSplit: number;
      }
    | null
  >(null);

  function clamp(value: number, min: number, max: number): number {
    return Math.max(min, Math.min(max, value));
  }

  function maxHeight(): number {
    return Math.max(MIN_HEIGHT, Math.round((globalThis.innerHeight || 900) * 0.7));
  }

  function persist() {
    try {
      localStorage.setItem(
        STORAGE_KEY,
        JSON.stringify({ open, height: Math.round(height), split }),
      );
    } catch {
      // Storage can be unavailable in a hardened webview; resizing still works.
    }
  }

  function toggle() {
    open = !open;
    persist();
  }

  function startHeightDrag(event: PointerEvent) {
    event.preventDefault();
    drag = {
      kind: "height",
      startY: event.clientY,
      startX: event.clientX,
      startHeight: height,
      startSplit: split,
    };
  }

  function startSplitDrag(event: PointerEvent) {
    event.preventDefault();
    drag = {
      kind: "split",
      startY: event.clientY,
      startX: event.clientX,
      startHeight: height,
      startSplit: split,
    };
  }

  function onPointerMove(event: PointerEvent) {
    if (!drag) return;
    if (drag.kind === "height") {
      height = clamp(drag.startHeight - (event.clientY - drag.startY), MIN_HEIGHT, maxHeight());
    } else if (panelsEl) {
      const rect = panelsEl.getBoundingClientRect();
      if (rect.width > 0) {
        split = clamp((event.clientX - rect.left) / rect.width, 0.25, 0.75);
      }
    }
  }

  function onPointerUp() {
    if (!drag) return;
    drag = null;
    persist();
  }

  onMount(() => {
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      if (raw) {
        const saved = JSON.parse(raw) as {
          open?: boolean;
          height?: number;
          split?: number;
        };
        open = saved.open === true;
        if (typeof saved.height === "number") {
          height = clamp(saved.height, MIN_HEIGHT, maxHeight());
        }
        if (typeof saved.split === "number") {
          split = clamp(saved.split, 0.25, 0.75);
        }
      }
    } catch {
      // Ignore corrupt or unavailable storage.
    }

    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
    window.addEventListener("pointercancel", onPointerUp);
    return () => {
      window.removeEventListener("pointermove", onPointerMove);
      window.removeEventListener("pointerup", onPointerUp);
      window.removeEventListener("pointercancel", onPointerUp);
    };
  });
</script>

<section
  class="bottom-dock"
  class:open
  style="height: {open ? `${Math.round(height)}px` : '32px'}"
>
  <div class="dock-bar">
    <button class="dock-toggle ghost" onclick={toggle} aria-expanded={open}>
      <span class="dock-chevron">{open ? "v" : "^"}</span>
      Positions &amp; order book
    </button>
    <span class="muted">positions support square-off</span>
    <span class="spacer"></span>
    {#if open}
      <span class="dock-hint">drag the top edge or the divider to resize</span>
    {/if}
  </div>

  {#if open}
    <div
      class="dock-resize"
      onpointerdown={startHeightDrag}
      role="separator"
      aria-orientation="horizontal"
      aria-label="Resize positions and order book"
      tabindex="-1"
    ></div>

    <div
      class="dock-panels"
      bind:this={panelsEl}
      style="grid-template-columns: {Math.round(split * 1000) / 10}% 6px 1fr"
    >
      <div class="dock-pane">
        <Positions embedded />
      </div>
      <div
        class="dock-split"
        onpointerdown={startSplitDrag}
        role="separator"
        aria-orientation="vertical"
        aria-label="Resize positions and order book columns"
        tabindex="-1"
      ></div>
      <div class="dock-pane">
        <Orders embedded />
      </div>
    </div>
  {/if}
</section>
