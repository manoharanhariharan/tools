<script lang="ts">
  import { onMount } from "svelte";
  import type { Window as TauriWindow } from "@tauri-apps/api/window";
  import { i18n } from "../i18n/index.svelte";
  import { isTauriRuntime } from "../ipc/runtime";
  import Icon from "./Icon.svelte";

  interface Props {
    title: string;
    subtitle?: string;
    version?: string;
  }

  let { title, subtitle, version }: Props = $props();
  let maximized = $state(false);
  let controlsReady = $state(false);

  onMount(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;

    if (isTauriRuntime) {
      void import("@tauri-apps/api/window").then(async ({ getCurrentWindow }) => {
        if (disposed) return;
        const window = getCurrentWindow();
        controlsReady = true;
        try {
          maximized = await window.isMaximized();
          unlisten = await window.onResized(async () => {
            maximized = await window.isMaximized();
          });
          if (disposed) {
            unlisten?.();
            unlisten = undefined;
          }
        } catch (error) {
          console.error("window state unavailable", error);
        }
      });
    }

    return () => {
      disposed = true;
      unlisten?.();
      unlisten = undefined;
    };
  });

  async function withWindow(
    action: (window: TauriWindow) => Promise<unknown>,
  ): Promise<void> {
    if (!isTauriRuntime) return;
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await action(getCurrentWindow());
  }

  function minimize() {
    void withWindow((window) => window.minimize());
  }

  async function toggleMaximize() {
    await withWindow(async (window) => {
      await window.toggleMaximize();
      maximized = await window.isMaximized();
    });
  }

  function close() {
    void withWindow((window) => window.close());
  }

  function onDragDoubleClick() {
    void toggleMaximize();
  }
</script>

<header class="titlebar" data-tauri-drag-region>
  <div class="brand" data-tauri-drag-region>
    <span class="mark" aria-hidden="true">
      <span class="dot"></span>
    </span>
    <div class="names" data-tauri-drag-region>
      <span class="title" data-tauri-drag-region>{title}</span>
      {#if subtitle}<span class="subtitle" data-tauri-drag-region>{subtitle}</span>{/if}
    </div>
    {#if version}<span class="version">v{version}</span>{/if}
  </div>

  <div class="drag-region" data-tauri-drag-region ondblclick={onDragDoubleClick} role="presentation"></div>

  <div class="controls" class:ready={controlsReady}>
    <button
      type="button"
      class="control"
      title={i18n.t("titlebar.minimize")}
      aria-label={i18n.t("titlebar.minimize")}
      onclick={minimize}
    >
      <Icon name="minimize" size={14} />
    </button>
    <button
      type="button"
      class="control"
      title={maximized ? i18n.t("titlebar.restore") : i18n.t("titlebar.maximize")}
      aria-label={maximized ? i18n.t("titlebar.restore") : i18n.t("titlebar.maximize")}
      onclick={() => void toggleMaximize()}
      oncontextmenu={(event) => {
        event.preventDefault();
        void toggleMaximize();
      }}
    >
      <Icon name={maximized ? "restore" : "maximize"} size={13} />
    </button>
    <button
      type="button"
      class="control close"
      title={i18n.t("titlebar.close")}
      aria-label={i18n.t("titlebar.close")}
      onclick={close}
    >
      <Icon name="close" size={14} />
    </button>
  </div>
</header>

<style>
  .titlebar {
    height: var(--hr-title-height);
    flex: none;
    display: flex;
    align-items: center;
    gap: 6px;
    background: color-mix(in srgb, var(--hr-primary-dark) 88%, var(--hr-background));
    border-bottom: 1px solid var(--hr-border);
    user-select: none;
    -webkit-app-region: drag;
    padding-left: 12px;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 9px;
    min-width: 0;
  }

  .mark {
    width: 22px;
    height: 22px;
    border-radius: 6px;
    background: linear-gradient(145deg, var(--hr-accent-light), var(--hr-accent-dark));
    display: grid;
    place-items: center;
    flex: none;
  }

  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--hr-accent-text);
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--hr-accent-text) 25%, transparent);
  }

  .names {
    display: flex;
    align-items: baseline;
    gap: 8px;
    min-width: 0;
  }

  .title {
    font-weight: 600;
    font-size: var(--hr-font-size-small);
    letter-spacing: 0.01em;
  }

  .subtitle {
    color: color-mix(in srgb, var(--hr-primary-text) 58%, transparent);
    font-size: var(--hr-font-size-small);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .version {
    font-size: 10px;
    color: color-mix(in srgb, var(--hr-primary-text) 45%, transparent);
    border: 1px solid var(--hr-border);
    border-radius: 999px;
    padding: 1px 7px;
  }

  .drag-region {
    flex: 1;
    align-self: stretch;
  }

  .controls {
    display: flex;
    align-self: stretch;
    -webkit-app-region: no-drag;
  }

  .control {
    width: 42px;
    border: 0;
    background: transparent;
    color: var(--hr-primary-text);
    display: grid;
    place-items: center;
  }

  .control:hover {
    background: color-mix(in srgb, var(--hr-primary-text) 12%, transparent);
  }

  .control.close:hover {
    background: var(--hr-danger);
    color: #fff;
  }

  .controls:not(.ready) {
    opacity: 0.7;
  }
</style>
