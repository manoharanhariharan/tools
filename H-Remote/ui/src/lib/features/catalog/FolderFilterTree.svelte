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
    data-testid={`folder-filter-${folder.path.join("-")}`}
    style:padding-left="{6 + depth * 14}px"
    title={folder.path.join(" / ")}
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
