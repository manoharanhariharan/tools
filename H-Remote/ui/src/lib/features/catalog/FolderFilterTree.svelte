<script lang="ts">
  import type { CatalogFolder } from "../../ipc/contracts";
  import { catalog } from "../../state/catalog.svelte";
  import Icon from "../../components/Icon.svelte";
  import { toCssColor } from "../../theme/apply";
  import FolderColorPicker from "./FolderColorPicker.svelte";
  import Self from "./FolderFilterTree.svelte";

  interface Props {
    folders: CatalogFolder[];
    depth?: number;
  }
  let { folders, depth = 0 }: Props = $props();
  let dropTarget = $state<string | null>(null);
  let picker = $state<{ x: number; y: number; folder: CatalogFolder } | null>(null);

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
  <div
    class="folder-row"
    class:active={active(folder.path)}
    class:drop={dropTarget === folder.path.join("/")}
    class:colored={Boolean(folder.colorHex)}
    style:padding-left="{6 + depth * 14}px"
    role="listitem"
    draggable="true"
    ondragstart={(event) => dragStart(event, folder)}
    ondragover={(event) => {
      event.preventDefault();
      event.stopPropagation();
      dropTarget = folder.path.join("/");
    }}
    ondragleave={() => (dropTarget = null)}
    ondrop={(event) => void dropOn(event, folder)}
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
        style={folder.colorHex ? `color: ${toCssColor(folder.colorHex)}` : undefined}
      >
        <Icon name="folder" size={13} />
      </span>
      <span class="name">{folder.name}</span>
      <span class="count">{count(folder)}</span>
    </button>
    <button
      type="button"
      class="color-dot"
      aria-label={`Colour for ${folder.name}`}
      title="Folder colour"
      onclick={(event) => openPicker(event, folder)}
    >
      <span
        class="dot"
        class:empty={!folder.colorHex}
        style={folder.colorHex ? `background: ${toCssColor(folder.colorHex)}` : undefined}
      ></span>
    </button>
  </div>
  {#if folder.folders.length > 0}
    <Self folders={folder.folders} depth={depth + 1} />
  {/if}
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
  .folder-row {
    display: flex;
    align-items: center;
    gap: 2px;
    width: 100%;
    border-radius: var(--hr-radius-small);
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

  .folder-glyph {
    display: grid;
    place-items: center;
    flex: none;
  }

  .name {
    flex: 1;    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .count {
    font-size: 9px;
    color: color-mix(in srgb, var(--hr-background-text) 48%, transparent);
  }

  .color-dot {
    display: grid;
    place-items: center;
    flex: none;
    width: 20px;
    height: 22px;
    border: 0;
    border-radius: var(--hr-radius-small);
    background: transparent;
    cursor: pointer;
    opacity: 0.55;
  }

  .folder-row:hover .color-dot,
  .folder-row.active .color-dot,
  .folder-row.colored .color-dot {
    opacity: 1;
  }

  .color-dot:hover {
    background: color-mix(in srgb, var(--hr-accent-mid) 16%, transparent);
  }

  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    border: 1px solid var(--hr-border-strong);
  }

  .dot.empty {
    background: transparent;
    border-style: dashed;
  }
</style>
