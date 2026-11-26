<script lang="ts">
  import type { CatalogFolder, Folder, Server } from "../../ipc/contracts";
  import { catalog } from "../../state/catalog.svelte";
  import Icon from "../../components/Icon.svelte";
  import ServerCard from "./ServerCard.svelte";
  import TriStateCheckbox from "../../components/TriStateCheckbox.svelte";
  import { toCssColor } from "../../theme/apply";
  import FolderColorButton from "./FolderColorButton.svelte";
  import FolderColorPicker from "./FolderColorPicker.svelte";
  import Self from "./CatalogCards.svelte";

  interface Props {
    folders: CatalogFolder[];
    servers: Server[];
  }
  let { folders, servers }: Props = $props();
  let picker = $state<{ x: number; y: number; folder: CatalogFolder } | null>(null);

  function flatten(folder: CatalogFolder): Server[] {
    return [...folder.servers, ...folder.folders.flatMap(flatten)];
  }

  function folderEntity(folder: CatalogFolder): Folder | undefined {
    return catalog.snapshot?.folders.find(
      (candidate) => candidate.path.join("/") === folder.path.join("/"),
    );
  }

  function openPicker(event: MouseEvent, folder: CatalogFolder): void {
    event.preventDefault();
    event.stopPropagation();
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    picker = { x: rect.left, y: rect.bottom + 4, folder };
  }

  async function pickColor(colorHex: string): Promise<void> {
    const entity = picker ? folderEntity(picker.folder) : undefined;
    picker = null;
    if (entity) {
      await catalog.setFolderColor(entity, colorHex);
    }
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
  {@const colour = folder.colorHex ? toCssColor(folder.colorHex) : ""}
  <section
    class="folder"
    class:colored={Boolean(colour)}
    style:--folder-color={colour || undefined}
  >
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
        <span
          class="folder-icon"
          style={colour ? `color: ${colour}` : undefined}
          aria-hidden="true"
        >
          <Icon name="folder" size={13} />
        </span>
        {folder.name}
        <span class="count">{all.length}</span>
      </button>
      <FolderColorButton
        colorHex={folder.colorHex}
        folderName={folder.name}
        onclick={(event) => openPicker(event, folder)}
      />
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

{#if picker}
  <FolderColorPicker
    x={picker.x}
    y={picker.y}
    value={picker.folder.colorHex}
    onpick={(colorHex) => void pickColor(colorHex)}
    onclose={() => (picker = null)}
  />
{/if}

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
    border-left: 3px solid transparent;
    padding: 9px;
    background: color-mix(in srgb, var(--hr-primary-dark) 22%, transparent);
  }

  .folder.colored {
    border-left-color: var(--folder-color);
    background: color-mix(
      in srgb,
      var(--folder-color) 7%,
      color-mix(in srgb, var(--hr-primary-dark) 22%, transparent)
    );
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

  .folder-icon {
    display: grid;
    place-items: center;
    flex: none;
    color: var(--hr-accent-light);
  }

  .count {
    font-size: 9px;
    border: 1px solid var(--hr-border);
    border-radius: 999px;
    padding: 0 6px;
  }
</style>
