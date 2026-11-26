<script lang="ts">
  import { catalog } from "../../state/catalog.svelte";
  import { dialogs } from "../../state/dialogs.svelte";
  import Icon from "../../components/Icon.svelte";
  import FolderFilterTree from "./FolderFilterTree.svelte";
  import TagPanel from "./TagPanel.svelte";

  let tab = $state<"folders" | "tags">("folders");
  let rootDrop = $state(false);
  const folders = $derived(catalog.allRootFolders());
  const total = $derived(catalog.snapshot?.servers.length ?? 0);

  async function dropOnRoot(event: DragEvent): Promise<void> {
    event.preventDefault();
    rootDrop = false;
    const payload = event.dataTransfer?.getData("text/plain") ?? "";
    if (payload.startsWith("server:")) {
      const server = catalog.serverById(payload.slice(7));
      if (server) {
        await catalog.moveServer(server, []);
      }
    } else if (payload.startsWith("folder:")) {
      const folder = catalog.snapshot?.folders.find(
        (candidate) => candidate.id === payload.slice(7),
      );
      if (folder && folder.path.length > 1) {
        await catalog.moveFolder(folder, []);
      }
    }
  }

  async function newRootFolder(): Promise<void> {
    const name = await dialogs.input({
      title: "New root folder",
      label: "Folder name",
      submitLabel: "Create",
    });
    if (name?.trim()) {
      await catalog.createFolder([name.trim()]);
    }
  }
</script>

<aside class="side-panel">
  <div class="switch" role="tablist" aria-label="Catalog filters">
    <button
      type="button"
      role="tab"
      aria-selected={tab === "folders"}
      class:active={tab === "folders"}
      onclick={() => (tab = "folders")}
    >
      Folders
    </button>
    <button
      type="button"
      role="tab"
      aria-selected={tab === "tags"}
      class:active={tab === "tags"}
      onclick={() => (tab = "tags")}
    >
      Tags
    </button>
  </div>

  {#if tab === "folders"}
    <div class="folder-head">
      <button
        type="button"
        class="all"
        class:active={!catalog.folderFilter}
        class:drop={rootDrop}
        title="Show all connections; drop here to keep a connection independent"
        ondragover={(event) => {
          event.preventDefault();
          rootDrop = true;
        }}
        ondragleave={() => (rootDrop = false)}
        ondrop={(event) => void dropOnRoot(event)}
        onclick={() => catalog.clearFolderFilter()}
      >
        <Icon name="servers" size={13} />
        <span>All connections</span>
        <span class="count">{total}</span>
      </button>
      <button
        type="button"
        class="new-folder"
        title="New root folder"
        aria-label="New root folder"
        onclick={() => void newRootFolder()}
      >
        <Icon name="plus" size={13} />
      </button>
    </div>
    <div class="folder-list">
      <FolderFilterTree {folders} />
      {#if folders.length === 0}
        <p class="empty">No folders yet. Create one to group and confine connections.</p>
      {/if}
      <p class="hint">Drag a connection onto a folder to file it, or onto “All connections” to keep it independent. Use the palette button (or right-click) to colour a folder.</p>
    </div>
  {:else}
    <TagPanel embedded />
  {/if}
</aside>

<style>
  .side-panel {
    width: 190px;
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 9px;
    border-right: 1px solid var(--hr-border);
    background: color-mix(in srgb, var(--hr-primary-dark) 22%, transparent);
    min-height: 0;
  }

  .switch {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 3px;
    padding: 2px;
    border: 1px solid var(--hr-border);
    border-radius: var(--hr-radius);
    background: color-mix(in srgb, var(--hr-primary-dark) 45%, transparent);
  }

  .switch button {
    min-height: 25px;
    border: 0;
    border-radius: var(--hr-radius-small);
    background: transparent;
    color: color-mix(in srgb, var(--hr-background-text) 62%, transparent);
    font-size: 10px;
  }

  .switch button.active {
    background: color-mix(in srgb, var(--hr-accent-mid) 20%, transparent);
    color: var(--hr-accent-light);
  }

  .folder-head {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .all {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 27px;
    padding: 3px 6px;
    border: 0;
    border-radius: var(--hr-radius-small);
    background: transparent;
    color: color-mix(in srgb, var(--hr-background-text) 78%, transparent);
    font-size: var(--hr-font-size-small);
    text-align: left;
  }

  .all span:nth-child(2) {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .all.active {
    background: color-mix(in srgb, var(--hr-accent-mid) 18%, transparent);
    color: var(--hr-accent-light);
  }

  .all.drop {
    outline: 1px dashed var(--hr-accent-light);
    outline-offset: -2px;
    background: color-mix(in srgb, var(--hr-accent-mid) 22%, transparent);
  }

  .new-folder {
    width: 25px;
    height: 25px;
    border: 1px solid var(--hr-border);
    border-radius: var(--hr-radius-small);
    background: transparent;
    color: inherit;
    display: grid;
    place-items: center;
  }

  .folder-list {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-height: 0;
    overflow: auto;
  }

  .count {
    font-size: 9px;
    color: color-mix(in srgb, var(--hr-background-text) 48%, transparent);
  }

  .empty,
  .hint {
    font-size: 10px;
    line-height: 1.4;
    color: color-mix(in srgb, var(--hr-background-text) 52%, transparent);
    padding: 6px;
  }

  .hint {
    margin-top: auto;
    border-top: 1px solid var(--hr-border);
    padding-top: 7px;
  }
</style>
