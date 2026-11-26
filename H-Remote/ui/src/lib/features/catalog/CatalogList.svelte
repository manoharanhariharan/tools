<script lang="ts">
  import type { CatalogFolder, Folder, Server } from "../../ipc/contracts";
  import { catalog } from "../../state/catalog.svelte";
  import ContextMenu, { type MenuItem } from "../../components/ContextMenu.svelte";
  import ServerRow from "./ServerRow.svelte";
  import TriStateCheckbox from "../../components/TriStateCheckbox.svelte";
  import { toCssColor } from "../../theme/apply";
  import FolderColorButton from "./FolderColorButton.svelte";
  import FolderColorPicker from "./FolderColorPicker.svelte";
  import FolderGlyph from "./FolderGlyph.svelte";
  import {
    deleteFolder,
    duplicateFolder,
    promptNewSubfolder,
    promptRenameFolder,
  } from "./folderActions";
  import Self from "./CatalogList.svelte";

  interface Props {
    folders: CatalogFolder[];
    servers: Server[];
    columns: Record<string, number>;
    showNote: boolean;
  }
  let { folders, servers, columns, showNote }: Props = $props();
  let picker = $state<{ x: number; y: number; folder: CatalogFolder } | null>(null);
  let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);

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

  function folderEntity(folder: CatalogFolder): Folder | undefined {
    return catalog.snapshot?.folders.find(
      (candidate) => candidate.path.join("/") === folder.path.join("/"),
    );
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

  function openPicker(event: MouseEvent, folder: CatalogFolder): void {
    event.preventDefault();
    event.stopPropagation();
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    picker = { x: rect.left, y: rect.bottom + 4, folder };
  }

  function openPickerAtPointer(event: MouseEvent, folder: CatalogFolder): void {
    event.preventDefault();
    event.stopPropagation();
    picker = { x: event.clientX, y: event.clientY, folder };
  }

  function folderMenu(event: MouseEvent, folder: CatalogFolder): void {
    event.preventDefault();
    event.stopPropagation();
    menu = {
      x: event.clientX,
      y: event.clientY,
      items: [
        {
          label: "New sub-folder",
          action: () => void promptNewSubfolder(folder.path),
        },
        { label: "Rename", action: () => void promptRenameFolder(folder) },
        { label: "Duplicate", action: () => void duplicateFolder(folder) },
        {
          label: "Folder colour…",
          action: () => openPickerAtPointer(event, folder),
        },
        { separator: true },
        { label: "Delete", danger: true, action: () => void deleteFolder(folder) },
      ],
    };
  }

  async function pickColor(colorHex: string): Promise<void> {
    const entity = picker ? folderEntity(picker.folder) : undefined;
    picker = null;
    if (entity) {
      await catalog.setFolderColor(entity, colorHex);
    }
  }
</script>

{#each folders as folder (folder.path.join("/"))}
  {@const state = folderState(folder)}
  {@const colour = folder.colorHex ? toCssColor(folder.colorHex) : ""}
  <div
    class="folder-group"
    class:colored={Boolean(colour)}
    style:--folder-color={colour || undefined}
  >
    <div
      class="folder-row"
      role="row"
      tabindex="0"
      oncontextmenu={(event) => folderMenu(event, folder)}
      onkeydown={(event) => {
        if (event.target !== event.currentTarget) {
          return;
        }
        if (event.key === "F2") {
          event.preventDefault();
          void promptRenameFolder(folder);
        }
      }}
    >
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
        <span
          class="folder-icon"
          style={colour ? `color: ${colour}` : undefined}
          aria-hidden="true"
        >
          <FolderGlyph size={13} />
        </span>
        <span class="folder-name">{folder.name}</span>
        <span class="count">{flatten(folder).length}</span>
      </button>
      <FolderColorButton
        colorHex={folder.colorHex}
        folderName={folder.name}
        onclick={(event) => openPicker(event, folder)}
      />
    </div>
    {#if catalog.isExpanded(folder.path)}
      <div class="folder-children">
        {#each folder.servers as server (server.id)}
          <ServerRow {server} hit={catalog.hitFor(server.id)} {columns} {showNote} />
        {/each}
        <Self folders={folder.folders} servers={[]} {columns} {showNote} />
      </div>
    {/if}
  </div>
{/each}

{#if catalog.hasMultipleSources() && catalog.sourceFilter === "all"}
  {#each catalog.groupServers(servers) as group (group.id)}
    <div class="source-row" role="row">
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

{#if picker}
  <FolderColorPicker
    x={picker.x}
    y={picker.y}
    value={picker.folder.colorHex}
    onpick={(colorHex) => void pickColor(colorHex)}
    onclose={() => (picker = null)}
  />
{/if}

{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={menu.items} onclose={() => (menu = null)} />
{/if}

<style>
  .folder-group {
    min-width: 0;
  }

  .folder-children {
    margin-left: 18px;
    padding-left: 3px;
    border-left: 1px solid var(--hr-border);
  }

  .folder-group.colored > .folder-children {
    border-left-color: color-mix(in srgb, var(--folder-color) 55%, var(--hr-border));
  }

  .folder-row {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 34px;
    padding: 3px 10px;
    border-bottom: 1px solid var(--hr-border);
    border-left: 3px solid transparent;
    background: color-mix(in srgb, var(--hr-primary-dark) 32%, transparent);
    content-visibility: auto;
    contain-intrinsic-size: auto 34px;
  }

  .folder-group.colored > .folder-row {
    border-left-color: var(--folder-color);
    background: color-mix(
      in srgb,
      var(--folder-color) 9%,
      color-mix(in srgb, var(--hr-primary-dark) 32%, transparent)
    );
  }

  .folder-row:hover {
    background: color-mix(in srgb, var(--hr-accent-mid) 8%, transparent);
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

  .folder-icon {
    display: grid;
    place-items: center;
    flex: none;
    color: var(--hr-accent-light);
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

  .folder-group.colored > .folder-row .folder-name {
    color: color-mix(in srgb, var(--folder-color) 45%, var(--hr-background-text));
  }

  .folder-group.colored > .folder-row .count {
    color: color-mix(in srgb, var(--folder-color) 30%, var(--hr-background-text));
  }

  .count {
    font-size: 9px;
    color: color-mix(in srgb, var(--hr-background-text) 46%, transparent);
    border: 1px solid var(--hr-border);
    border-radius: 999px;
    padding: 0 6px;
  }
</style>
