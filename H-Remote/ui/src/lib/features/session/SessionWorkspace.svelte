<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { MenuItem } from "../../components/ContextMenu.svelte";
  import ContextMenu from "../../components/ContextMenu.svelte";
  import ServerIcon from "../../components/ServerIcon.svelte";
  import Button from "../../components/Button.svelte";
  import Icon from "../../components/Icon.svelte";
  import { commands } from "../../ipc/client";
  import { isTauriRuntime } from "../../ipc/runtime";
  import { minimizeCurrentWindow } from "../../ipc/window";
  import { i18n } from "../../i18n/index.svelte";
  import { app } from "../../state/app.svelte";
  import { catalog } from "../../state/catalog.svelte";
  import { dialogs } from "../../state/dialogs.svelte";
  import { sessions } from "../../state/sessions.svelte";
  import SessionPane from "./SessionPane.svelte";
  import { availableLayouts, isLayoutValid, resolveLayout } from "./layouts";

  interface Props {
    /** True when this component owns a dedicated session window. */
    standalone?: boolean;
  }
  let { standalone = false }: Props = $props();

  let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);
  let layoutOpen = $state(false);
  let fullscreen = $state(false);
  let dragId = $state<string | null>(null);
  let resizingWindow: ReturnType<typeof setTimeout> | undefined;
  let onKeydown: ((event: KeyboardEvent) => void) | undefined;
  let onFullscreen: (() => void) | undefined;
  let maximized = $state(false);
  let windowControlsReady = $state(false);
  let windowResizeUnlisten: (() => void) | undefined;
  let windowDisposed = false;

  const sessionsInWorkspace = $derived(sessions.sessions);
  const active = $derived(
    sessionsInWorkspace.find((session) => session.id === sessions.activeId) ??
      sessionsInWorkspace[0] ??
      null,
  );
  const focused = $derived(
    sessions.focusedId
      ? (sessionsInWorkspace.find((session) => session.id === sessions.focusedId) ?? null)
      : null,
  );
  const gridActive = $derived(
    !focused && sessions.gridMode && sessionsInWorkspace.length >= 2,
  );
  const activeVnc = $derived(active?.backend === "vnc");
  const activeRdp = $derived(active?.backend === "ironRdp");
  const layoutOptions = $derived(availableLayouts(sessionsInWorkspace.length));
  const layout = $derived(
    resolveLayout(sessions.layout, sessionsInWorkspace.length),
  );
  const gridStyle = $derived(
    `display:grid;grid-template-columns:repeat(${layout.columns},minmax(0,1fr));grid-template-rows:repeat(${layout.rows},minmax(0,1fr));gap:6px;height:100%;`,
  );

  onMount(() => {
    sessions.startEvents();
    if (!catalog.snapshot) {
      void catalog.load();
    }
    onKeydown = (event: KeyboardEvent) => {
      if (event.key === "F11") {
        event.preventDefault();
        void toggleFullscreen();
        return;
      }
      if (!event.ctrlKey || event.altKey || event.metaKey) {
        return;
      }
      if (event.key === "Tab") {
        event.preventDefault();
        sessions.cycle(event.shiftKey ? -1 : 1);
        return;
      }
      if (/^[1-9]$/.test(event.key)) {
        event.preventDefault();
        sessions.selectIndex(Number(event.key) - 1);
      }
    };
    onFullscreen = () => {
      fullscreen = Boolean(document.fullscreenElement);
    };
    window.addEventListener("keydown", onKeydown);
    document.addEventListener("fullscreenchange", onFullscreen);
    if (standalone && isTauriRuntime) {
      void import("@tauri-apps/api/window").then(async ({ getCurrentWindow }) => {
        if (windowDisposed) {
          return;
        }
        const window = getCurrentWindow();
        windowControlsReady = true;
        try {
          maximized = await window.isMaximized();
          windowResizeUnlisten = await window.onResized(async () => {
            maximized = await window.isMaximized();
          });
          if (windowDisposed) {
            windowResizeUnlisten?.();
            windowResizeUnlisten = undefined;
          }
        } catch (reason) {
          console.debug("session window state unavailable", reason);
        }
      });
    }
  });

  onDestroy(() => {
    windowDisposed = true;
    if (onKeydown) {
      window.removeEventListener("keydown", onKeydown);
    }
    if (onFullscreen) {
      document.removeEventListener("fullscreenchange", onFullscreen);
    }
    windowResizeUnlisten?.();
    windowResizeUnlisten = undefined;
    if (resizingWindow) {
      clearTimeout(resizingWindow);
    }
  });

  async function toggleMaximizeWindow(): Promise<void> {
    if (!isTauriRuntime) {
      return;
    }
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    const window = getCurrentWindow();
    await window.toggleMaximize();
    maximized = await window.isMaximized();
  }

  async function closeWindow(): Promise<void> {
    if (!isTauriRuntime) {
      return;
    }
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().close();
  }

  const serverFor = (serverId: string) =>
    catalog.snapshot?.servers.find((server) => server.id === serverId);

  async function toggleFullscreen(): Promise<void> {
    try {
      if (document.fullscreenElement) {
        await document.exitFullscreen();
      } else {
        await document.documentElement.requestFullscreen();
      }
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async function copy(value: string, label: string): Promise<void> {
    try {
      await navigator.clipboard.writeText(value);
      app.showNotice(`${label} copied`, "info");
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  function menuFor(session: (typeof sessions.sessions)[number], event: MouseEvent): void {
    const server = serverFor(session.serverId);
    event.preventDefault();
    event.stopPropagation();
    const items: MenuItem[] = [
      { label: "Focus", action: () => sessions.focus(session.id) },
      {
        label: "Reconnect",
        action: () => void sessions.reconnect(session.id),
      },
      ...(session.backend === "ironRdp"
        ? [{ label: "Display…", action: () => sessions.openDisplay(session.id) }]
        : []),
      ...(server?.alternateCredentials ?? []).map((alternate) => ({
        label: `Connect via ${alternate.name || alternate.address || "alternate"}`,
        action: () => void sessions.connectVia(server!, alternate.name),
      })),
      {
        label: "Close tab",
        action: () => void sessions.close(session.id),
      },
      {
        label: "Close other tabs",
        action: () => void closeOthers(session.id),
      },
      { separator: true },
      ...(server
        ? [
            {
              label: i18n.t("common.edit"),
              action: () => catalog.openEdit(server),
            },
            {
              label: "Duplicate",
              action: () => void catalog.openDuplicate(server),
            },
            { separator: true },
          ]
        : []),
      {
        label: "Copy hostname",
        disabled: !server?.address,
        action: () => void copy(server?.address ?? "", "Hostname"),
      },
      {
        label: "Copy username",
        disabled: !server?.username,
        action: () => void copy(server?.username ?? "", "Username"),
      },
      {
        label: "Copy password",
        disabled: !server?.password,
        action: async () => {
          if (
            server?.password &&
            (await app.ensureVerified("Copy a stored password"))
          ) {
            await copy(server.password, "Password");
          }
        },
      },
      { separator: true },
      {
        label: "Create shortcut",
        action: () => void createShortcut(session),
      },
      {
        label: "Export .rdp",
        disabled: server?.protocol !== "rdp",
        action: () => void exportRdp(session),
      },
      { separator: true },
      { label: "Ctrl+Alt+Del", action: () => void sessions.sendCtrlAltDel(session.id) },
      ...resolutionItems(session),
      { separator: true },
      {
        label: "Customise tab buttons",
        action: () => void customiseButtons(),
      },
      { separator: true },
      {
        label: i18n.t("common.close") + " all tabs",
        danger: true,
        action: () => void closeAll(),
      },
    ];
    menu = { x: event.clientX, y: event.clientY, items };
  }

  function resolutionItems(session: (typeof sessions.sessions)[number]): MenuItem[] {
    const presets: Array<[string, number, number]> = [
      ["1024 × 768", 1024, 768],
      ["1280 × 720", 1280, 720],
      ["1920 × 1080", 1920, 1080],
    ];
    return [
      ...presets.map(([label, width, height]) => ({
        label: `Display: ${label}`,
        action: () => void sessions.resize(session.id, width, height),
      })),
      {
        label: "Display: fit pane",
        action: () => {
          const pane = document.querySelector<HTMLElement>(
            `[aria-label="${CSS.escape(session.serverName)} session"] .stage`,
          );
          if (pane) {
            const rect = pane.getBoundingClientRect();
            void sessions.resize(session.id, rect.width, rect.height);
          }
        },
      },
    ];
  }

  async function createShortcut(session: (typeof sessions.sessions)[number]): Promise<void> {
    try {
      const result = await commands.createShortcutForServers(
        [session.id],
        session.serverName,
        false,
      );
      app.showNotice(result.path, "info");
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async function exportRdp(session: (typeof sessions.sessions)[number]): Promise<void> {
    if (!(await app.ensureVerified("Export an .rdp file"))) {
      return;
    }
    try {
      const result = await commands.exportRdpServer(session.serverId);
      const blob = new Blob([result.content], { type: "application/x-rdp" });
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement("a");
      anchor.href = url;
      anchor.download = `${session.serverName.replace(/[\\/:*?"<>|]/g, "_")}.rdp`;
      anchor.click();
      URL.revokeObjectURL(url);
      if (result.warning) {
        app.showNotice(result.warning, "warning");
      }
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async function customiseButtons(): Promise<void> {
    const choice = await dialogs.message({
      title: "Tab buttons",
      message: "Toggle tab header buttons. Changes persist for every window.",
      buttons: [
        { value: "icons", label: `Icons: ${sessions.showIcons ? "on" : "off"}` },
        { value: "close", label: `Close: ${sessions.showClose ? "on" : "off"}` },
        {
          value: "reconnect",
          label: `Reconnect: ${sessions.showReconnect ? "on" : "off"}`,
        },
        { value: "cancel", label: i18n.t("common.close") },
      ],
    });
    if (choice === "icons") {
      await sessions.setTabButtons({ showIcons: !sessions.showIcons });
    } else if (choice === "close") {
      await sessions.setTabButtons({ showClose: !sessions.showClose });
    } else if (choice === "reconnect") {
      await sessions.setTabButtons({ showReconnect: !sessions.showReconnect });
    }
  }

  async function closeAll(): Promise<void> {
    const confirmed = await dialogs.confirm({
      title: "Close all session tabs?",
      message: `${sessionsInWorkspace.length} session(s) will be disconnected.`,
      danger: true,
    });
    if (!confirmed) {
      return;
    }
    for (const session of [...sessionsInWorkspace]) {
      await sessions.close(session.id);
    }
    if (standalone) {
      await closeDetachedWindow();
    }
  }

  async function closeOthers(keep: string): Promise<void> {
    for (const session of [...sessionsInWorkspace]) {
      if (session.id !== keep) {
        await sessions.close(session.id);
      }
    }
  }

  async function closeDetachedWindow(): Promise<void> {
    if (!isTauriRuntime) {
      return;
    }
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().close();
  }

  function onTabDragStart(event: DragEvent, id: string): void {
    dragId = id;
    event.dataTransfer?.setData("text/plain", id);
    if (event.dataTransfer) {
      event.dataTransfer.effectAllowed = "move";
    }
  }

  function onTabDrop(event: DragEvent, index: number): void {
    event.preventDefault();
    const id = dragId ?? event.dataTransfer?.getData("text/plain") ?? "";
    dragId = null;
    if (id) {
      sessions.reorder(id, index);
    }
  }

  function tabStyle(index: number): string {
    const cell = layout.cells[index];
    if (!gridActive || !cell || !isLayoutValid(layout)) {
      return "";
    }
    return `grid-column:${cell.column + 1} / span ${cell.spanX};grid-row:${cell.row + 1} / span ${cell.spanY};`;
  }
</script>

<section
  class="workspace"
  class:fullscreen
  aria-label="Session workspace"
>
  <header class="workspace-bar">
    {#if standalone}
      <div
        class="drag-handle"
        data-tauri-drag-region
        title="Drag window"
        role="presentation"
        ondblclick={() => void toggleMaximizeWindow()}
      ></div>
    {/if}
    <div class="tabs" role="tablist" aria-label="Sessions">
      {#each sessionsInWorkspace as session, index (session.id)}
        {@const server = serverFor(session.serverId)}
        <button
          type="button"
          role="tab"
          aria-selected={session.id === sessions.activeId}
          class="tab"
          class:active={session.id === sessions.activeId}
          draggable="true"
          title={`${session.serverName} — ${session.state}`}
          onclick={() => sessions.select(session.id)}
          onauxclick={(event) => {
            if (event.button === 1) {
              event.preventDefault();
              void sessions.close(session.id);
            }
          }}
          oncontextmenu={(event) => menuFor(session, event)}
          ondragstart={(event) => onTabDragStart(event, session.id)}
          ondragover={(event) => event.preventDefault()}
          ondrop={(event) => onTabDrop(event, index)}
        >
          {#if sessions.showIcons && server}
            <ServerIcon {server} size={16} />
          {/if}
          {#if server?.colorHex}
            <span class="colour" style:background={server.colorHex} aria-hidden="true"></span>
          {/if}
          <span class="dot" class:connected={session.state === "connected"} class:failed={session.state === "failed"}></span>
          <span class="label">{session.serverName}</span>
          {#if sessions.showReconnect && (session.state === "failed" || session.state === "disconnected")}
            <span
              class="tab-action"
              role="button"
              tabindex="0"
              title="Reconnect"
              onclick={(event) => {
                event.stopPropagation();
                void sessions.reconnect(session.id);
              }}
              onkeydown={(event) => {
                if (event.key === "Enter") {
                  event.stopPropagation();
                  void sessions.reconnect(session.id);
                }
              }}
            >
              ↻
            </span>
          {/if}
          {#if sessions.showClose}
            <span
              class="tab-action"
              role="button"
              tabindex="0"
              title="Close tab"
              onclick={(event) => {
                event.stopPropagation();
                void sessions.close(session.id);
              }}
              onkeydown={(event) => {
                if (event.key === "Enter") {
                  event.stopPropagation();
                  void sessions.close(session.id);
                }
              }}
            >
              ×
            </span>
          {/if}
        </button>
      {/each}
    </div>

    <div class="workspace-actions">
      <Button
        size="sm"
        variant={gridActive ? "primary" : "secondary"}
        disabled={sessionsInWorkspace.length < 2}
        title="Toggle grid mode"
        onclick={() => void sessions.setGridMode(!sessions.gridMode)}
      >
        <Icon name="servers" size={14} /> Grid
      </Button>
      {#if gridActive}
        <div class="layout-wrap">
          <Button size="sm" variant="secondary" title="Snap layout" onclick={() => (layoutOpen = !layoutOpen)}>
            Layout
          </Button>
          {#if layoutOpen}
            <div class="layout-pop" role="menu">
              {#each layoutOptions as option (option.id)}
                <button
                  type="button"
                  role="menuitem"
                  class:active={sessions.layout === option.id}
                  onclick={() => {
                    layoutOpen = false;
                    void sessions.setLayout(option.id);
                  }}
                >
                  <span
                    class="thumb"
                    style={`grid-template-columns:repeat(${resolveLayout(option.id, sessionsInWorkspace.length).columns},1fr);grid-template-rows:repeat(${resolveLayout(option.id, sessionsInWorkspace.length).rows},1fr);`}
                  >
                    {#each resolveLayout(option.id, sessionsInWorkspace.length).cells as cell, index (index)}
                      <i
                        style={`grid-column:${cell.column + 1} / span ${cell.spanX};grid-row:${cell.row + 1} / span ${cell.spanY};`}
                      ></i>
                    {/each}
                  </span>
                  {option.label}
                </button>
              {/each}
            </div>
          {/if}
        </div>
      {/if}
      <Button
        size="sm"
        variant="secondary"
        title="Session display: resolution and virtual monitors"
        disabled={!activeRdp}
        onclick={() => sessions.openDisplay(active?.id ?? null)}
      >
        <Icon name="maximize" size={14} /> Display
      </Button>
      <Button
        size="sm"
        variant="secondary"
        title="Minimize this window"
        onclick={() => void minimizeCurrentWindow().catch((error) => app.reportError(error))}
      >
        <Icon name="minimize" size={14} /> Minimize
      </Button>
      {#if focused}
        <Button size="sm" variant="secondary" onclick={() => sessions.leaveFocus()}>
          All tabs
        </Button>
      {/if}
      {#if active && active.state === "connected"}
        <Button size="sm" variant="secondary" onclick={() => void sessions.cancel(active.id)}>
          Disconnect
        </Button>
      {/if}
      <Button size="sm" variant="secondary" title="Send Ctrl+Alt+Del" onclick={() => void sessions.sendCtrlAltDel()} disabled={!active || active.state !== "connected"}>
        Ctrl+Alt+Del
      </Button>
      {#if activeVnc && active}
        <select
          class="special"
          aria-label="Special keys"
          value=""
          disabled={active.state !== "connected"}
          onchange={(event) => {
            const value = event.currentTarget.value;
            if (value) {
              void sessions.sendSpecial(
                active.id,
                value as "ctrlAltDel" | "ctrlEsc" | "altF4",
              );
            }
            event.currentTarget.value = "";
          }}
        >
          <option value="">Keys…</option>
          <option value="ctrlAltDel">Ctrl+Alt+Del</option>
          <option value="ctrlEsc">Ctrl+Esc</option>
          <option value="altF4">Alt+F4</option>
        </select>
      {/if}
      <Button size="sm" variant="secondary" title="Toggle full screen (F11)" onclick={() => void toggleFullscreen()}>
        <Icon name={fullscreen ? "restore" : "maximize"} size={14} />
      </Button>
      <Button size="sm" variant="ghost" title={i18n.t("common.close")} onclick={() => void closeAll()}>
        <Icon name="close" size={14} />
      </Button>
    </div>

    {#if standalone}
      <div class="window-controls" class:ready={windowControlsReady}>
        <button
          type="button"
          class="window-control"
          title={i18n.t("titlebar.minimize")}
          aria-label={i18n.t("titlebar.minimize")}
          onclick={() => void minimizeCurrentWindow()}
        >
          <Icon name="minimize" size={14} />
        </button>
        <button
          type="button"
          class="window-control"
          title={maximized ? i18n.t("titlebar.restore") : i18n.t("titlebar.maximize")}
          aria-label={maximized ? i18n.t("titlebar.restore") : i18n.t("titlebar.maximize")}
          onclick={() => void toggleMaximizeWindow()}
          oncontextmenu={(event) => {
            event.preventDefault();
            void toggleMaximizeWindow();
          }}
        >
          <Icon name={maximized ? "restore" : "maximize"} size={13} />
        </button>
        <button
          type="button"
          class="window-control close"
          title={i18n.t("titlebar.close")}
          aria-label={i18n.t("titlebar.close")}
          onclick={() => void closeWindow()}
        >
          <Icon name="close" size={14} />
        </button>
      </div>
    {/if}
  </header>

  <div class="content">
    {#if sessionsInWorkspace.length === 0}
      <div class="empty">
        <h2>No sessions in this window</h2>
        {#if standalone}
          <Button variant="secondary" onclick={() => void closeDetachedWindow()}>Close window</Button>
        {/if}
      </div>
    {:else}
      <div
        class="panes"
        style={gridActive ? gridStyle : "display:grid;grid-template-columns:1fr;grid-template-rows:1fr;height:100%;"}
      >
        {#if gridActive}
          {#each sessionsInWorkspace as session, index (session.id)}
            <div class="cell" style={tabStyle(index)}>
              <SessionPane
                {session}
                frame={sessions.frames[session.id] ?? null}
                onfocus={() => sessions.select(session.id)}
                onclose={() => void sessions.close(session.id)}
              />
            </div>
          {/each}
        {:else}
          {@const shown = focused ?? active}
          {#if shown}
            <SessionPane
              session={shown}
              frame={sessions.frames[shown.id] ?? null}
              onfocus={() => sessions.select(shown.id)}
              onclose={() => void sessions.close(shown.id)}
            />
          {/if}
        {/if}
      </div>
    {/if}
  </div>
</section>

{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={menu.items} onclose={() => (menu = null)} />
{/if}

<style>
  .workspace {
    position: fixed;
    inset: 0;
    z-index: 800;
    display: flex;
    flex-direction: column;
    background: #0b1520;
    color: #e6f0fa;
  }

  .workspace-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 42px;
    padding: 5px 8px;
    background: #101f2e;
    border-bottom: 1px solid rgba(255, 255, 255, 0.12);
    user-select: none;
  }

  .drag-handle {
    width: 16px;
    align-self: stretch;
    flex: none;
    cursor: move;
  }

  .window-controls {
    display: flex;
    align-self: stretch;
    flex: none;
    margin: -5px -8px -5px 0;
  }

  .window-control {
    width: 40px;
    border: 0;
    background: transparent;
    color: #a9bfd2;
    display: grid;
    place-items: center;
  }

  .window-control:hover {
    background: rgba(255, 255, 255, 0.12);
    color: #eaf4ff;
  }

  .window-control.close:hover {
    background: #c0392b;
    color: #fff;
  }

  .window-controls:not(.ready) {
    opacity: 0.7;
  }

  .tabs {
    display: flex;
    gap: 3px;
    overflow-x: auto;
    flex: 1;
    min-width: 0;
    scrollbar-width: thin;
  }

  .tab {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    min-width: 110px;
    max-width: 230px;
    padding: 4px 7px;
    border: 1px solid transparent;
    border-radius: 6px;
    background: transparent;
    color: #a9bfd2;
    font-size: 12px;
    white-space: nowrap;
  }

  .tab.active {
    background: rgba(83, 158, 226, 0.18);
    border-color: rgba(83, 158, 226, 0.5);
    color: #eaf4ff;
  }

  .tab .label {
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }

  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #cf9b3c;
    flex: none;
  }

  .colour {
    width: 6px;
    height: 16px;
    border-radius: 2px;
    flex: none;
  }

  .dot.connected {
    background: #49b675;
  }

  .dot.failed {
    background: #d05a5a;
  }

  .tab-action {
    border: 0;
    background: transparent;
    color: inherit;
    padding: 0 2px;
    border-radius: 3px;
    opacity: 0.65;
    font-size: 12px;
  }

  .tab-action:hover {
    opacity: 1;
    background: rgba(255, 255, 255, 0.12);
  }

  .workspace-actions {
    display: flex;
    align-items: center;
    gap: 5px;
    flex: none;
  }

  .special {
    height: 26px;
    border-radius: 5px;
    border: 1px solid rgba(255, 255, 255, 0.16);
    background: #132436;
    color: inherit;
    font-size: 11px;
  }

  .layout-wrap {
    position: relative;
  }

  .layout-pop {
    position: absolute;
    top: calc(100% + 4px);
    right: 0;
    z-index: 900;
    width: 220px;
    max-height: 320px;
    overflow: auto;
    padding: 5px;
    border-radius: 7px;
    border: 1px solid rgba(255, 255, 255, 0.16);
    background: #16293b;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.45);
  }

  .layout-pop button {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 5px;
    border: 0;
    border-radius: 5px;
    background: transparent;
    color: inherit;
    font-size: 11px;
    text-align: left;
  }

  .layout-pop button:hover,
  .layout-pop button.active {
    background: rgba(83, 158, 226, 0.2);
  }

  .thumb {
    display: grid;
    gap: 1px;
    width: 34px;
    height: 24px;
    flex: none;
    padding: 2px;
    border: 1px solid rgba(255, 255, 255, 0.25);
    border-radius: 3px;
  }

  .thumb i {
    background: rgba(143, 208, 255, 0.55);
    border-radius: 1px;
  }

  .content {
    flex: 1;
    min-height: 0;
    padding: 7px;
    display: flex;
  }

  .panes {
    flex: 1;
    min-height: 0;
  }

  .cell {
    min-width: 0;
    min-height: 0;
    display: flex;
  }

  .cell > :global(.pane) {
    flex: 1;
  }

  .empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    color: #b7c9d9;
  }

  .empty h2 {
    font-size: 18px;
    color: #e6f0fa;
  }
</style>
