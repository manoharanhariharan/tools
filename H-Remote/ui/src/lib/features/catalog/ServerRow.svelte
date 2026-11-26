<script lang="ts">
  import { onDestroy } from "svelte";
  import type { SearchHit, Server } from "../../ipc/contracts";
  import { commands } from "../../ipc/client";
  import { i18n } from "../../i18n/index.svelte";
  import { app } from "../../state/app.svelte";
  import { catalog, cloneJson } from "../../state/catalog.svelte";
  import ContextMenu, { type MenuItem } from "../../components/ContextMenu.svelte";
  import HighlightedText from "../../components/HighlightedText.svelte";
  import Icon from "../../components/Icon.svelte";
  import ServerIcon from "../../components/ServerIcon.svelte";
  import TriStateCheckbox from "../../components/TriStateCheckbox.svelte";

  interface Props {
    server: Server;
    hit?: SearchHit;
    columns: Record<string, number>;
    showNote: boolean;
  }
  let { server, hit, columns, showNote }: Props = $props();

  let editingAddress = $state(false);
  let addressDraft = $state("");
  let addressInput = $state<HTMLInputElement | null>(null);
  let addressEditTimer: ReturnType<typeof setTimeout> | undefined;
  let editingName = $state(false);
  let nameDraft = $state("");
  let nameInput = $state<HTMLInputElement | null>(null);
  let menu = $state<{ x: number; y: number } | null>(null);
  let noteOpen = $state(false);

  $effect(() => {
    if (editingAddress && addressInput) {
      addressInput.focus();
      addressInput.select();
    }
  });

  $effect(() => {
    if (editingName && nameInput) {
      nameInput.focus();
      nameInput.select();
    }
  });

  const selected = $derived(catalog.selected.has(server.id));
  const editing = $derived(
    catalog.editor?.draft.id === server.id ||
      Boolean(catalog.editor?.servers.some((candidate) => candidate.id === server.id)),
  );
  const lastConnected = $derived(
    catalog.snapshot?.connectionRecords.records[server.id]?.lastConnectedAt,
  );
  const template = $derived(
    `46px ${columns.protocol}px minmax(${columns.name}px, 1.2fr) ${columns.ip}px ${showNote ? `${columns.note}px ` : ""}minmax(${columns.target}px, 1fr) 62px`,
  );

  function startAddressEdit(event: MouseEvent) {
    event.stopPropagation();
    // Delay the inline editor so the first click of a double-click does not
    // replace the IP button before the row can handle the connect action.
    clearTimeout(addressEditTimer);
    addressEditTimer = setTimeout(() => {
      addressEditTimer = undefined;
      addressDraft = server.address ?? "";
      editingAddress = true;
    }, 220);
  }

  onDestroy(() => {
    clearTimeout(addressEditTimer);
  });

  async function commitAddress() {
    if (!editingAddress) {
      return;
    }
    editingAddress = false;
    const address = addressDraft.trim();
    if (!address) {
      app.showNotice("Address cannot be empty.", "warning");
      return;
    }
    if (address === server.address) {
      return;
    }
    await catalog.saveInline({ ...cloneJson(server), address });
  }

  function cancelAddress() {
    editingAddress = false;
  }

  function startNameEdit(event?: Event) {
    event?.stopPropagation();
    if (readOnly) {
      return;
    }
    nameDraft = server.displayName;
    editingName = true;
  }

  async function commitName() {
    if (!editingName) {
      return;
    }
    editingName = false;
    const displayName = nameDraft.trim();
    if (!displayName) {
      app.showNotice("Connection name cannot be empty.", "warning");
      return;
    }
    if (displayName === server.displayName) {
      return;
    }
    await catalog.saveInline({ ...cloneJson(server), displayName });
  }

  function cancelName() {
    editingName = false;
  }

  function onRowDragStart(event: DragEvent) {
    if (readOnly || editingAddress || editingName) {
      event.preventDefault();
      return;
    }
    event.dataTransfer?.setData("text/plain", `server:${server.id}`);
    if (event.dataTransfer) {
      event.dataTransfer.effectAllowed = "move";
    }
  }

  function openMenu(event: MouseEvent) {
    event.preventDefault();
    event.stopPropagation();
    menu = { x: event.clientX, y: event.clientY };
  }

  async function copy(value: string, label: string) {
    try {
      await navigator.clipboard.writeText(value);
      app.showNotice(`${label} copied`, "info");
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async function createShortcut() {
    try {
      const result = await commands.createShortcutForServers(
        [server.id],
        server.displayName,
        false,
      );
      app.showNotice(result.path, "info");
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async function exportRdp() {
    if (!(await app.ensureVerified("Export an .rdp file"))) {
      return;
    }
    try {
      const result = await commands.exportRdpServer(server.id);
      const blob = new Blob([result.content], { type: "application/x-rdp" });
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement("a");
      anchor.href = url;
      anchor.download = `${server.displayName.replace(/[\\/:*?"<>|]/g, "_")}.rdp`;
      anchor.click();
      URL.revokeObjectURL(url);
      if (result.warning) {
        app.showNotice(result.warning, "warning");
      }
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  const readOnly = $derived(catalog.isReadOnly(server));

  const menuItems = $derived<MenuItem[]>([
    readOnly
      ? { label: "View", action: () => catalog.openEdit(server) }
      : { label: i18n.t("common.edit"), action: () => catalog.openEdit(server) },
    { label: "Duplicate", action: () => void catalog.openDuplicate(server) },
    { label: "Connect", action: () => void catalog.connect([server]) },
    ...(server.alternateCredentials ?? []).map((alternate) => ({
      label: `Connect via ${alternate.name || alternate.address || "alternate"}`,
      action: () => void catalog.connectVia(server, alternate.name),
    })),
    { separator: true },
    {
      label: `Copy address`,
      action: () =>
        void copy(
          `${server.address ?? ""}${server.port ? `:${server.port}` : ""}`,
          "Address",
        ),
    },
    {
      label: "Copy username",
      action: () => void copy(server.username ?? "", "Username"),
      disabled: !server.username,
    },
    { separator: true },
    {
      label: "Move up",
      action: () => void catalog.moveServerOrder(server.id, -1),
    },
    {
      label: "Move down",
      action: () => void catalog.moveServerOrder(server.id, 1),
    },
    { separator: true },
    {
      label: "Create shortcut",
      action: () => void createShortcut(),
    },
    {
      label: i18n.t("catalog.export") + " .rdp",
      action: () => void exportRdp(),
      disabled: server.protocol !== "rdp",
    },
    { separator: true },
    ...(readOnly
      ? []
      : [
          {
            label: i18n.t("common.delete"),
            danger: true,
            action: () => {
              catalog.selected.clear();
              catalog.selected.add(server.id);
              void catalog.deleteSelected();
            },
          },
        ]),
  ]);
</script>

<div
  class="row"
  class:selected
  class:editing
  style:grid-template-columns={template}
  role="row"
  tabindex="0"
  title="Double-click to connect · F2 to rename · drag the row to a folder"
  draggable={!readOnly && !editingAddress && !editingName}
  ondragstart={onRowDragStart}
  ondblclick={(event) => {
    // Double-clicking a row connects. Only an already-open editable control
    // (for example the inline address input) keeps the event for itself.
    clearTimeout(addressEditTimer);
    addressEditTimer = undefined;
    const target = event.target as HTMLElement | null;
    if (target?.closest("input, textarea, select")) {
      return;
    }
    void catalog.connect([server]);
  }}
  onkeydown={(event) => {
    // Enter belongs to the focused row only. Inputs and buttons handle their
    // own Enter (inline IP editing must save, never connect).
    if (event.target !== event.currentTarget) {
      return;
    }
    if (event.key === "Enter") {
      void catalog.connect([server]);
    } else if (event.key === "F2") {
      event.preventDefault();
      startNameEdit(event);
    }
  }}
  oncontextmenu={openMenu}
>
  <div class="cell select">
    <span class="drag-grip" title="Drag connection" aria-hidden="true">
      <Icon name="grip" size={12} />
    </span>
    <TriStateCheckbox
      checked={selected}
      label={`Select ${server.displayName}`}
      onchange={() => catalog.toggleSelected(server.id)}
    />
  </div>
  <div class="cell protocol">
    <span class="badge">{server.protocol}</span>
  </div>
  <div class="cell name">
    {#if catalog.sort === "custom" && !readOnly}
      <span class="reorder" aria-label={`Reorder ${server.displayName}`}>
        <button
          type="button"
          title="Move up"
          aria-label={`Move ${server.displayName} up`}
          onclick={(event) => {
            event.stopPropagation();
            void catalog.moveServerOrder(server.id, -1);
          }}
        >
          <Icon name="arrow-up" size={11} />
        </button>
        <button
          type="button"
          title="Move down"
          aria-label={`Move ${server.displayName} down`}
          onclick={(event) => {
            event.stopPropagation();
            void catalog.moveServerOrder(server.id, 1);
          }}
        >
          <Icon name="arrow-down" size={11} />
        </button>
      </span>
    {/if}
    {#if editingName}
      <ServerIcon {server} size={26} />
      <input
        bind:this={nameInput}
        class="inline-name"
        bind:value={nameDraft}
        aria-label={`Rename ${server.displayName}`}
        onblur={() => void commitName()}
        onkeydown={(event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            event.stopPropagation();
            void commitName();
          } else if (event.key === "Escape") {
            event.preventDefault();
            event.stopPropagation();
            cancelName();
          }
        }}
      />
    {:else}
      <button
        type="button"
        class="name-button"
        title="Click to select; F2 renames; double-click opens the full editor"
        onclick={(event) => {
          event.stopPropagation();
          catalog.selected.clear();
          catalog.selected.add(server.id);
          (event.currentTarget.closest(".row") as HTMLElement | null)?.focus();
        }}
        onkeydown={(event) => {
          if (event.key === "F2") {
            event.preventDefault();
            startNameEdit(event);
          }
        }}
      >
        <ServerIcon {server} size={26} />
        <span class="name-text">
          <span class="name-line">
            <HighlightedText text={server.displayName} ranges={hit?.nameRanges ?? []} />
          </span>
          <span class="tags">
            {#each server.tags.slice(0, 3) as tag (tag)}
              <span class="tag">#{tag}</span>
            {/each}
          </span>
        </span>
        {#if server.note}
          <span
            class="note-dot"
            title={server.note}
            role="img"
            aria-label="Has note"
            onmouseenter={() => (noteOpen = true)}
            onmouseleave={() => (noteOpen = false)}
          ></span>
        {/if}
      </button>
    {/if}
    {#if noteOpen && server.note}
      <div class="note-pop" role="tooltip">{server.note}</div>
    {/if}
    {#if lastConnected}
      <span class="last-connected" title={new Date(lastConnected * 1000).toLocaleString()}>
        {new Date(lastConnected * 1000).toLocaleDateString()}
      </span>
    {/if}
  </div>
  <div class="cell ip">
    {#if editingAddress}
      <input
        bind:this={addressInput}
        class="inline-edit"
        bind:value={addressDraft}
        onblur={() => void commitAddress()}
        onkeydown={(event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            event.stopPropagation();
            void commitAddress();
          } else if (event.key === "Escape") {
            event.preventDefault();
            event.stopPropagation();
            cancelAddress();
          }
        }}
      />
    {:else}
      <button type="button" class="linkish" onclick={startAddressEdit} title="Click to edit address">
        {server.address || "—"}
      </button>
    {/if}
  </div>
  {#if showNote}
    <div class="cell note" title={server.note}>
      <span>{server.note}</span>
    </div>
  {/if}
  <div class="cell target">
    <HighlightedText text={server.subTitle} ranges={hit?.subtitleRanges ?? []} />
  </div>
  <div class="cell source">
    <span class="source-dot" class:readonly={catalog.isReadOnly(server)} title={catalog.isReadOnly(server) ? "Read-only data source" : "Local catalog"}>
      {catalog.sourceLabel(server.sourceId)}
    </span>
  </div>
</div>

{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={menuItems} onclose={() => (menu = null)} />
{/if}

<style>
  .row {
    display: grid;
    align-items: center;
    gap: 10px;
    min-height: 48px;
    padding: 4px 10px;
    border-bottom: 1px solid var(--hr-border);
    cursor: grab;
    /* Skip layout/paint for offscreen rows in large catalogs (P10-06). */
    content-visibility: auto;
    contain-intrinsic-size: auto 48px;
  }

  .row:hover {
    background: color-mix(in srgb, var(--hr-primary-text) 6%, transparent);
  }

  .row.selected {
    background: color-mix(in srgb, var(--hr-accent-mid) 12%, transparent);
    box-shadow: inset 3px 0 0 var(--hr-accent-mid);
  }

  .row.editing {
    box-shadow: inset 3px 0 0 var(--hr-warning);
  }

  .row:focus-visible {
    outline: 2px solid var(--hr-accent-mid);
    outline-offset: -2px;
  }

  .cell {
    min-width: 0;
    overflow: hidden;
  }

  .select {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
  }

  .drag-grip {
    display: grid;
    place-items: center;
    color: color-mix(in srgb, var(--hr-background-text) 38%, transparent);
    cursor: grab;
  }

  .drag-grip:hover {
    color: var(--hr-accent-light);
  }

  .protocol .badge {
    text-transform: uppercase;
    font-size: 9px;
    letter-spacing: 0.06em;
    padding: 3px 7px;
    border-radius: 999px;
    border: 1px solid var(--hr-border);
    color: color-mix(in srgb, var(--hr-background-text) 72%, transparent);
  }

  .name {
    position: relative;
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .cell.name {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .name-button {
    display: flex;
    align-items: center;
    gap: 9px;
    min-width: 0;
    border: 0;
    background: transparent;
    color: inherit;
    padding: 0;
    text-align: left;
  }

  .reorder {
    display: inline-flex;
    flex-direction: column;
    gap: 1px;
    flex: none;
  }

  .reorder button {
    width: 17px;
    height: 14px;
    padding: 0;
    border: 0;
    border-radius: 3px;
    background: transparent;
    color: color-mix(in srgb, var(--hr-background-text) 58%, transparent);
    display: grid;
    place-items: center;
  }

  .reorder button:hover {
    background: color-mix(in srgb, var(--hr-accent-mid) 22%, transparent);
    color: var(--hr-accent-light);
  }

  .name-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .name-text > :global(*) {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .name-line {
    display: block;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .inline-name {
    flex: 1;
    min-width: 0;
    min-height: 28px;
    border-radius: var(--hr-radius-small);
    border: 1px solid var(--hr-accent-mid);
    background: var(--hr-background);
    color: var(--hr-background-text);
    padding: 3px 7px;
    font: inherit;
    font-size: var(--hr-font-size-small);
  }

  .tags {
    display: flex;
    gap: 4px;
    overflow: hidden;
  }

  .tag {
    font-size: 9px;
    color: var(--hr-accent-light);
  }

  .note-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--hr-warning);
    flex: none;
  }

  .note-pop {
    position: absolute;
    top: calc(100% + 4px);
    left: 34px;
    z-index: 20;
    max-width: 360px;
    padding: 8px 10px;
    border-radius: var(--hr-radius);
    border: 1px solid var(--hr-border-strong);
    background: color-mix(in srgb, var(--hr-primary-mid) 96%, var(--hr-background));
    box-shadow: var(--hr-shadow);
    white-space: pre-wrap;
    font-size: var(--hr-font-size-small);
  }

  .last-connected {
    margin-left: auto;
    font-size: 9px;
    color: color-mix(in srgb, var(--hr-background-text) 45%, transparent);
    flex: none;
  }

  .linkish {
    border: 0;
    background: transparent;
    color: inherit;
    padding: 2px 0;
    text-align: left;
    font-family: var(--hr-font-family);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 100%;
  }

  .linkish:hover {
    color: var(--hr-accent-light);
    text-decoration: underline;
  }

  .inline-edit {
    width: 100%;
    min-height: 28px;
    border-radius: var(--hr-radius-small);
    border: 1px solid var(--hr-accent-mid);
    background: var(--hr-primary-dark);
    color: inherit;
    padding: 0 6px;
  }

  .note span,
  .target {
    color: color-mix(in srgb, var(--hr-background-text) 66%, transparent);
    font-size: var(--hr-font-size-small);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    display: block;
  }

  .source-dot {
    font-size: 9px;
    color: color-mix(in srgb, var(--hr-background-text) 46%, transparent);
  }

  .source-dot.readonly {
    color: var(--hr-warning);
    border: 1px solid color-mix(in srgb, var(--hr-warning) 45%, transparent);
    border-radius: 999px;
    padding: 0 5px;
  }
</style>
