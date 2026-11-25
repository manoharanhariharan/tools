<script lang="ts">
  import type { CatalogFolder } from "../../ipc/contracts";
  import { catalog } from "../../state/catalog.svelte";
  import Icon from "../../components/Icon.svelte";
  import Self from "./FolderFilterTree.svelte";

  interface Props {
    folders: CatalogFolder[];
    depth?: number;
  }
  let { folders, depth = 0 }: Props = $props();
  let dropTarget = $state<string | null>(null);

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
  <button
    type="button"
    class="folder"
    class:active={active(folder.path)}
    class:drop={dropTarget === folder.path.join("/")}
    data-testid={`folder-filter-${folder.path.join("-")}`}
    style:padding-left="{6 + depth * 14}px"
    title={`${folder.path.join(" / ")} — drop a connection here`}
    draggable="true"
    ondragstart={(event) => dragStart(event, folder)}
    ondragover={(event) => {
      event.preventDefault();
      event.stopPropagation();
      dropTarget = folder.path.join("/");
    }}
    ondragleave={() => (dropTarget = null)}
    ondrop={(event) => void dropOn(event, folder)}
    onclick={() =>
      active(folder.path)
        ? catalog.clearFolderFilter()
        : catalog.setFolderFilter(folder.path)}
  >
    <Icon name="folder" size={13} />
    <span class="name">{folder.name}</span>
    <span class="count">{count(folder)}</span>
  </button>
  {#if folder.folders.length > 0}
    <Self folders={folder.folders} depth={depth + 1} />
  {/if}
{/each}

<style>
  .folder {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    min-height: 27px;
    padding: 3px 6px;
    border: 0;
    border-radius: var(--hr-radius-small);
    background: transparent;
    color: color-mix(in srgb, var(--hr-background-text) 78%, transparent);
    font-size: var(--hr-font-size-small);
    text-align: left;
  }

  .folder:hover {
    background: color-mix(in srgb, var(--hr-primary-text) 8%, transparent);
  }

  .folder.active {
    background: color-mix(in srgb, var(--hr-accent-mid) 18%, transparent);
    color: var(--hr-accent-light);
  }

  .folder.drop {
    outline: 1px dashed var(--hr-accent-light);
    outline-offset: -2px;
    background: color-mix(in srgb, var(--hr-accent-mid) 22%, transparent);
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
