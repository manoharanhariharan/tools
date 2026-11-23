<script lang="ts">
  import type { CatalogFolder, Server } from "../../ipc/contracts";
  import { catalog } from "../../state/catalog.svelte";
  import ServerRow from "./ServerRow.svelte";
  import TriStateCheckbox from "../../components/TriStateCheckbox.svelte";
  import Self from "./CatalogList.svelte";

  interface Props {
    folders: CatalogFolder[];
    servers: Server[];
    columns: Record<string, number>;
    showNote: boolean;
    depth?: number;
  }
  let { folders, servers, columns, showNote, depth = 0 }: Props = $props();

  function flatten(folder: CatalogFolder): Server[] {
    return [
      ...folder.servers,
      ...folder.folders.flatMap((child) => flatten(child)),
    ];
  }

  function folderState(folder: CatalogFolder) {
    const all = flatten(folder);
    const selected = all.filter((server) => catalog.selected.has(server.id));
    return {
      all: all.length > 0 && selected.length === all.length,
      some: selected.length > 0 && selected.length < all.length,
    };
  }

  function toggleFolderSelection(folder: CatalogFolder) {
    const all = flatten(folder);
    const state = folderState(folder);
    if (state.all) {
      for (const server of all) catalog.selected.delete(server.id);
    } else {
      for (const server of all) catalog.selected.add(server.id);
    }
  }
</script>

{#each folders as folder (folder.path.join("/"))}
  {@const state = folderState(folder)}
  <div class="folder-row" style:padding-left="{10 + depth * 18}px" role="row">
    <TriStateCheckbox
      checked={state.all}
      indeterminate={state.some}
      label={`Select folder ${folder.path.join(" / ")}`}
      onchange={() => toggleFolderSelection(folder)}
    />
    <button
      type="button"
      class="folder-toggle"
      aria-expanded={catalog.isExpanded(folder.path)}
      onclick={() => catalog.toggleFolder(folder.path)}
    >
      <span class="chevron" class:open={catalog.isExpanded(folder.path)}>›</span>
      <span class="folder-name">{folder.name}</span>
      <span class="count">{flatten(folder).length}</span>
    </button>
  </div>
  {#if catalog.isExpanded(folder.path)}
    {#each folder.servers as server (server.id)}
      <ServerRow {server} hit={catalog.hitFor(server.id)} {columns} {showNote} />
    {/each}
    <Self
      folders={folder.folders}
      servers={[]}
      {columns}
      {showNote}
      depth={depth + 1}
    />
  {/if}
{/each}

{#if catalog.hasMultipleSources() && catalog.sourceFilter === "all"}
  {#each catalog.groupServers(servers) as group (group.id)}
    <div class="source-row" style:padding-left="{10 + depth * 18}px" role="row">
      <span class="source-name">{group.label}</span>
      <span class="source-count">{group.servers.length}</span>
      {#if group.readOnly}<span class="source-ro">read-only</span>{/if}
    </div>
    {#each group.servers as server (server.id)}
      <ServerRow {server} hit={catalog.hitFor(server.id)} {columns} {showNote} />
    {/each}
  {/each}
{:else}
  {#each servers as server (server.id)}
    <ServerRow {server} hit={catalog.hitFor(server.id)} {columns} {showNote} />
  {/each}
{/if}

<style>
  .folder-row {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 34px;
    padding: 3px 10px;
    border-bottom: 1px solid var(--hr-border);
    background: color-mix(in srgb, var(--hr-primary-dark) 32%, transparent);
    content-visibility: auto;
    contain-intrinsic-size: auto 34px;
  }

  .folder-toggle {
    display: flex;
    align-items: center;
    gap: 7px;
    border: 0;
    background: transparent;
    color: inherit;
    padding: 3px 0;
    text-align: left;
    min-width: 0;
  }

  .chevron {
    display: inline-grid;
    place-items: center;
    width: 16px;
    color: color-mix(in srgb, var(--hr-background-text) 55%, transparent);
    transition: transform 120ms ease;
  }

  .chevron.open {
    transform: rotate(90deg);
  }

  .source-row {
    display: flex;
    align-items: center;
    gap: 7px;
    min-height: 30px;
    padding: 3px 10px;
    border-bottom: 1px solid var(--hr-border);
    background: color-mix(in srgb, var(--hr-accent-mid) 8%, transparent);
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: color-mix(in srgb, var(--hr-background-text) 58%, transparent);
  }

  .source-name {
    font-weight: 600;
  }

  .source-count {
    font-variant-numeric: tabular-nums;
  }

  .source-ro {
    border: 1px solid color-mix(in srgb, var(--hr-warning) 40%, transparent);
    border-radius: 999px;
    padding: 0 6px;
    color: var(--hr-warning);
    letter-spacing: normal;
    text-transform: none;
  }

  .folder-name {
    font-weight: 600;
    font-size: var(--hr-font-size-small);
  }

  .count {
    font-size: 9px;
    color: color-mix(in srgb, var(--hr-background-text) 46%, transparent);
    border: 1px solid var(--hr-border);
    border-radius: 999px;
    padding: 0 6px;
  }
</style>
