<script lang="ts">
  import type { CatalogFolder, Server } from "../../ipc/contracts";
  import { catalog } from "../../state/catalog.svelte";
  import ServerCard from "./ServerCard.svelte";
  import TriStateCheckbox from "../../components/TriStateCheckbox.svelte";
  import Self from "./CatalogCards.svelte";

  interface Props {
    folders: CatalogFolder[];
    servers: Server[];
  }
  let { folders, servers }: Props = $props();

  function flatten(folder: CatalogFolder): Server[] {
    return [...folder.servers, ...folder.folders.flatMap(flatten)];
  }
</script>

<div class="cards">
  {#each servers as server (server.id)}
    <ServerCard {server} />
  {/each}
</div>

{#each folders as folder (folder.path.join("/"))}
  {@const all = flatten(folder)}
  {@const selected = all.filter((server) => catalog.selected.has(server.id))}
  <section class="folder">
    <header>
      <TriStateCheckbox
        checked={all.length > 0 && selected.length === all.length}
        indeterminate={selected.length > 0 && selected.length < all.length}
        label={`Select folder ${folder.path.join(" / ")}`}
        onchange={() => {
          if (selected.length === all.length) {
            for (const server of all) catalog.selected.delete(server.id);
          } else {
            for (const server of all) catalog.selected.add(server.id);
          }
        }}
      />
      <button
        type="button"
        class="folder-toggle"
        aria-expanded={catalog.isExpanded(folder.path)}
        onclick={() => catalog.toggleFolder(folder.path)}
      >
        <span class="chevron" class:open={catalog.isExpanded(folder.path)}>›</span>
        {folder.name}
        <span class="count">{all.length}</span>
      </button>
    </header>
    {#if catalog.isExpanded(folder.path)}
      <div class="cards">
        {#each folder.servers as server (server.id)}
          <ServerCard {server} />
        {/each}
      </div>
      <Self folders={folder.folders} servers={[]} />
    {/if}
  </section>
{/each}

<style>
  .cards {
    display: grid;
    gap: 10px;
    grid-template-columns: repeat(auto-fill, minmax(245px, 1fr));
  }

  .folder {
    margin-top: 10px;
    border-radius: var(--hr-radius);
    border: 1px solid var(--hr-border);
    padding: 9px;
    background: color-mix(in srgb, var(--hr-primary-dark) 22%, transparent);
  }

  header {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 8px;
  }

  .folder-toggle {
    display: flex;
    align-items: center;
    gap: 7px;
    border: 0;
    background: transparent;
    color: inherit;
    font-weight: 600;
    font-size: var(--hr-font-size-small);
    padding: 2px;
  }

  .chevron {
    transition: transform 120ms ease;
  }

  .chevron.open {
    transform: rotate(90deg);
  }

  .count {
    font-size: 9px;
    border: 1px solid var(--hr-border);
    border-radius: 999px;
    padding: 0 6px;
  }
</style>
