<script lang="ts">
  import type { CatalogFolder } from "../../ipc/contracts";
  import { catalog } from "../../state/catalog.svelte";
  import ContextMenu, { type MenuItem } from "../../components/ContextMenu.svelte";
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
  import Self from "./FolderFilterTree.svelte";

  interface Props {
    folders: CatalogFolder[];
  }
  let { folders }: Props = $props();
  let dropTarget = $state<string | null>(null);
  let picker = $state<{ x: number; y: number; folder: CatalogFolder } | null>(null);
  let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);

  function folderEntity(folder: CatalogFolder) {
    return catalog.snapshot?.folders.find(
      (candidate) => candidate.path.join("/") === folder.path.join("/"),
    );
  }

  function dragStart(event: DragEvent, folder: CatalogFolder): void {
    const entity = folderEntity(folder);
    if (!entity) {
      return;
    }
    event.stopPropagation();
    event.dataTransfer?.setData("text/plain", `folder:${entity.id}`);
    if (event.dataTransfer) {
      event.dataTransfer.effectAllowed = "move";
    }
  }

  async function dropOn(event: DragEvent, target: CatalogFolder): Promise<void> {
    event.preventDefault();
    event.stopPropagation();
    dropTarget = null;
    const payload = event.dataTransfer?.getData("text/plain") ?? "";
    if (payload.startsWith("server:")) {
      const server = catalog.serverById(payload.slice(7));
      if (server) {
        await catalog.moveServer(server, target.path);
      }
    } else if (payload.startsWith("folder:")) {
      const entity = catalog.snapshot?.folders.find(
        (folder) => folder.id === payload.slice(7),
      );
      if (!entity) {
        return;
      }
      const from = entity.path.join("/");
      const into = target.path.join("/");
      if (from === into || into.startsWith(`${from}/`)) {
        return;
      }
      await catalog.moveFolder(entity, target.path);
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

  function count(folder: CatalogFolder): number {
    return (
      folder.servers.length +
      folder.folders.reduce((total, child) => total + count(child), 0)
    );
  }

  function active(path: string[]): boolean {
    return catalog.folderFilter?.join("/") === path.join("/");
  }
</script>

{#each folders as folder (folder.path.join("/"))}
  {@const colour = folder.colorHex ? toCssColor(folder.colorHex) : ""}
  <div
    class="folder-group"
    class:colored={Boolean(colour)}
    style:--folder-color={colour || undefined}
  >
    <div
      class="folder-row"
      class:active={active(folder.path)}
      class:drop={dropTarget === folder.path.join("/")}
      role="treeitem"
      aria-expanded="true"
      aria-selected={active(folder.path)}
      tabindex="0"
      draggable="true"
      ondragstart={(event) => dragStart(event, folder)}
      ondragover={(event) => {
        event.preventDefault();
        event.stopPropagation();
        dropTarget = folder.path.join("/");
      }}
      ondragleave={() => (dropTarget = null)}
      ondrop={(event) => void dropOn(event, folder)}
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
      <button
        type="button"
        class="folder"
        data-testid={`folder-filter-${folder.path.join("-")}`}
        title={`${folder.path.join(" / ")} — drop a connection here`}
        onclick={() =>
          active(folder.path)
            ? catalog.clearFolderFilter()
            : catalog.setFolderFilter(folder.path)}
      >
        <span
          class="folder-glyph"
          style={colour ? `color: ${colour}` : undefined}
        >
          <FolderGlyph size={13} />
        </span>
        <span class="name">{folder.name}</span>
        <span class="count">{count(folder)}</span>
      </button>
      <FolderColorButton
        colorHex={folder.colorHex}
        folderName={folder.name}
        onclick={(event) => openPicker(event, folder)}
      />
    </div>
    {#if folder.folders.length > 0}
      <div class="folder-children">
        <Self folders={folder.folders} />
      </div>
    {/if}
  </div>
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

{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={menu.items} onclose={() => (menu = null)} />
{/if}

<style>
  .folder-group {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .folder-children {
    margin-left: 9px;
    padding-left: 3px;
    border-left: 1px solid var(--hr-border);
  }

  .folder-group.colored > .folder-children {
    border-left-color: color-mix(in srgb, var(--folder-color) 55%, var(--hr-border));
  }

  .folder-row {
    display: flex;
    align-items: center;
    gap: 2px;
    width: 100%;
    min-width: 0;
    border-radius: var(--hr-radius-small);
    border-left: 2px solid transparent;
  }

  .folder-group.colored > .folder-row {
    border-left-color: var(--folder-color);
    background: color-mix(in srgb, var(--folder-color) 7%, transparent);
  }

  .folder-row:hover {
    background: color-mix(in srgb, var(--hr-primary-text) 8%, transparent);
  }

  .folder-row.active {
    background: color-mix(in srgb, var(--hr-accent-mid) 18%, transparent);
  }

  .folder-row.drop {
    outline: 1px dashed var(--hr-accent-light);
    outline-offset: -2px;
    background: color-mix(in srgb, var(--hr-accent-mid) 22%, transparent);
  }

  .folder {
    display: flex;
    flex: 1;
    align-items: center;
    gap: 6px;
    min-width: 0;
    min-height: 27px;
    padding: 3px 4px;
    border: 0;
    border-radius: var(--hr-radius-small);
    background: transparent;
    color: color-mix(in srgb, var(--hr-background-text) 78%, transparent);
    font-size: var(--hr-font-size-small);
    text-align: left;
    cursor: pointer;
  }

  .folder-row.active .folder {
    color: var(--hr-accent-light);
  }

  .folder-group.colored > .folder-row .name {
    color: color-mix(in srgb, var(--folder-color) 45%, var(--hr-background-text));
    font-weight: 600;
  }

  .folder-group.colored > .folder-row .count {
    color: color-mix(in srgb, var(--folder-color) 30%, var(--hr-background-text));
  }

  .folder-glyph {
    display: grid;
    place-items: center;
    flex: none;
  }

  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .count {
    font-size: 9px;
    color: color-mix(in srgb, var(--hr-background-text) 48%, transparent);
  }
</style>
