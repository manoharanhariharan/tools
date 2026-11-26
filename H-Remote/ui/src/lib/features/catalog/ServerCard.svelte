<script lang="ts">
  import type { Server } from "../../ipc/contracts";
  import { commands } from "../../ipc/client";
  import { app } from "../../state/app.svelte";
  import { catalog } from "../../state/catalog.svelte";
  import ContextMenu, { type MenuItem } from "../../components/ContextMenu.svelte";
  import Icon from "../../components/Icon.svelte";
  import ServerIcon from "../../components/ServerIcon.svelte";
  import TriStateCheckbox from "../../components/TriStateCheckbox.svelte";
  import { i18n } from "../../i18n/index.svelte";

  interface Props {
    server: Server;
  }
  let { server }: Props = $props();
  let menu = $state<{ x: number; y: number } | null>(null);
  const selected = $derived(catalog.selected.has(server.id));

  const items = $derived<MenuItem[]>([
    { label: i18n.t("common.edit"), action: () => catalog.openEdit(server) },
    { label: "Duplicate", action: () => void catalog.openDuplicate(server) },
    { label: "Connect", action: () => void catalog.connect([server]) },
    { separator: true },
    {
      label: "Create shortcut",
      action: async () => {
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
      },
    },
    { separator: true },
    {
      label: i18n.t("common.delete"),
      danger: true,
      action: () => {
        catalog.selected.clear();
        catalog.selected.add(server.id);
        void catalog.deleteSelected();
      },
    },
  ]);
</script>

<div
  class="card"
  class:selected
  role="button"
  tabindex="0"
  title="Double-click to connect · drag to a folder"
  draggable={!catalog.isReadOnly(server)}
  ondragstart={(event) => {
    if (catalog.isReadOnly(server)) {
      event.preventDefault();
      return;
    }
    event.dataTransfer?.setData("text/plain", `server:${server.id}`);
    if (event.dataTransfer) {
      event.dataTransfer.effectAllowed = "move";
    }
  }}
  ondblclick={() => catalog.connect([server])}
  onkeydown={(event) => {
    if (event.key === "Enter") {
      void catalog.connect([server]);
    }
  }}
  oncontextmenu={(event) => {
    event.preventDefault();
    menu = { x: event.clientX, y: event.clientY };
  }}
>
  <div class="top">
    <TriStateCheckbox
      checked={selected}
      label={`Select ${server.displayName}`}
      onchange={() => catalog.toggleSelected(server.id)}
    />
    <ServerIcon {server} size={40} />
    <div class="name">
      <strong>{server.displayName}</strong>
      <span>{server.subTitle}</span>
    </div>
    <span class="protocol">{server.protocol}</span>
    {#if catalog.sort === "custom" && !catalog.isReadOnly(server)}
      <span class="reorder">
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
  </div>
  {#if server.tags.length > 0 || catalog.hasMultipleSources()}
    <div class="tags">
      {#if catalog.hasMultipleSources()}
        <span class="source" class:readonly={catalog.isReadOnly(server)}>
          {catalog.sourceLabel(server.sourceId)}
        </span>
      {/if}
      {#each server.tags.slice(0, 5) as tag (tag)}
        <span>#{tag}</span>
      {/each}
    </div>
  {/if}
  {#if server.note}
    <p class="note">{server.note}</p>
  {/if}
</div>

{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={items} onclose={() => (menu = null)} />
{/if}

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: 9px;
    padding: 13px;
    border-radius: var(--hr-radius);
    border: 1px solid var(--hr-border);
    background: color-mix(in srgb, var(--hr-primary-dark) 32%, transparent);
    cursor: grab;
    min-width: 0;
    /* Defer offscreen card work in large catalogs (P10-06). */
    content-visibility: auto;
    contain-intrinsic-size: auto 140px;
  }

  .card:hover {
    border-color: var(--hr-border-strong);
  }

  .reorder {
    display: inline-flex;
    gap: 2px;
    flex: none;
  }

  .reorder button {
    width: 20px;
    height: 20px;
    border: 1px solid var(--hr-border);
    border-radius: 4px;
    background: transparent;
    color: inherit;
    display: grid;
    place-items: center;
  }

  .reorder button:hover {
    background: color-mix(in srgb, var(--hr-accent-mid) 18%, transparent);
    color: var(--hr-accent-light);
  }

  .card.selected {
    border-color: var(--hr-accent-mid);
    background: color-mix(in srgb, var(--hr-accent-mid) 10%, transparent);
  }

  .top {
    display: flex;
    align-items: flex-start;
    gap: 9px;
    min-width: 0;
  }

  .name {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    flex: 1;
  }

  .name strong,
  .name span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .name span {
    font-size: var(--hr-font-size-small);
    color: color-mix(in srgb, var(--hr-background-text) 62%, transparent);
  }

  .protocol {
    font-size: 9px;
    text-transform: uppercase;
    border: 1px solid var(--hr-border);
    border-radius: 999px;
    padding: 2px 6px;
    color: color-mix(in srgb, var(--hr-background-text) 56%, transparent);
  }

  .tags {
    display: flex;
    gap: 5px;
    flex-wrap: wrap;
    font-size: 9px;
    color: var(--hr-accent-light);
  }

  .tags .source {
    color: color-mix(in srgb, var(--hr-background-text) 46%, transparent);
    border: 1px solid var(--hr-border);
    border-radius: 999px;
    padding: 0 5px;
  }

  .tags .source.readonly {
    color: var(--hr-warning);
    border-color: color-mix(in srgb, var(--hr-warning) 40%, transparent);
  }

  .note {
    font-size: var(--hr-font-size-small);
    color: color-mix(in srgb, var(--hr-background-text) 58%, transparent);
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }
</style>
