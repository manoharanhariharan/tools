<script lang="ts">
  import type { CatalogFolder, Folder, Server } from "../../ipc/contracts";
  import { app } from "../../state/app.svelte";
  import { catalog } from "../../state/catalog.svelte";
  import { dialogs } from "../../state/dialogs.svelte";
  import ContextMenu, { type MenuItem } from "../../components/ContextMenu.svelte";
  import Icon from "../../components/Icon.svelte";
  import ServerIcon from "../../components/ServerIcon.svelte";
  import TriStateCheckbox from "../../components/TriStateCheckbox.svelte";
  import { i18n } from "../../i18n/index.svelte";
  import Self from "./CatalogTree.svelte";

  interface Props {
    folders: CatalogFolder[];
    servers: Server[];
    depth?: number;
  }
  let { folders, servers, depth = 0 }: Props = $props();
  let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);
  let dropTarget = $state<string | null>(null);

  function flatten(folder: CatalogFolder): Server[] {
    return [...folder.servers, ...folder.folders.flatMap(flatten)];
  }

  function folderEntity(folder: CatalogFolder): Folder | undefined {
    return catalog.snapshot?.folders.find(
      (candidate) => candidate.path.join("/") === folder.path.join("/"),
    );
  }

  async function promptNewFolder(parent: string[]): Promise<void> {
    const name = await dialogs.input({
      title: "New folder",
      label: "Folder name",
      submitLabel: "Create",
    });
    if (name?.trim()) {
      await catalog.createFolder([...parent, name.trim()]);
    }
  }

  async function promptRename(folder: CatalogFolder): Promise<void> {
    const entity = folderEntity(folder);
    const name = await dialogs.input({
      title: "Rename folder",
      label: "Folder name",
      value: folder.name,
      submitLabel: "Rename",
    });
    if (entity && name?.trim()) {
      await catalog.renameFolder(entity, name.trim());
    }
  }

  function folderMenu(event: MouseEvent, folder: CatalogFolder) {
    event.preventDefault();
    event.stopPropagation();
    const entity = folderEntity(folder);
    const items: MenuItem[] = [
      { label: "New sub-folder", action: () => void promptNewFolder(folder.path) },
      { label: "Rename", action: () => void promptRename(folder) },
      { label: "Duplicate", action: () => entity && void catalog.duplicateFolder(entity) },
      {
        label: "Edit all servers",
        action: () => catalog.openBulk(flatten(folder)),
        disabled: flatten(folder).length < 2,
      },
      { label: "Move up", action: () => void catalog.moveFolderOrder(folder.path, -1) },
      { label: "Move down", action: () => void catalog.moveFolderOrder(folder.path, 1) },
      { separator: true },
      {
        label: i18n.t("common.delete"),
        danger: true,
        action: () => entity && void catalog.deleteFolder(entity),
      },
    ];
    menu = { x: event.clientX, y: event.clientY, items };
  }

  function serverMenu(event: MouseEvent, server: Server) {
    event.preventDefault();
    event.stopPropagation();
    menu = {
      x: event.clientX,
      y: event.clientY,
      items: [
        { label: i18n.t("common.edit"), action: () => catalog.openEdit(server) },
        { label: "Duplicate", action: () => void catalog.openDuplicate(server) },
        { label: "Connect", action: () => void catalog.connect([server]) },
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
      ],
    };
  }

  function dragStart(event: DragEvent, payload: string) {
    event.dataTransfer?.setData("text/plain", payload);
    if (event.dataTransfer) event.dataTransfer.effectAllowed = "move";
  }

  async function dropOn(event: DragEvent, path: string[]) {
    event.preventDefault();
    dropTarget = null;
    const payload = event.dataTransfer?.getData("text/plain") ?? "";
    if (!payload) return;
    if (payload.startsWith("server:")) {
      const server = catalog.serverById(payload.slice(7));
      if (server) await catalog.moveServer(server, path);
    } else if (payload.startsWith("folder:")) {
      const entity = catalog.snapshot?.folders.find((folder) => folder.id === payload.slice(7));
      if (entity) await catalog.moveFolder(entity, path);
    }
  }

  function allowDrop(event: DragEvent, key: string) {
    event.preventDefault();
    dropTarget = key;
  }
</script>

<div class="tree" role="tree">
  <div
    class="root"
    class:drop={dropTarget === "root"}
    role="treeitem"
    aria-selected="false"
    aria-expanded="true"
    tabindex="0"
    ondragover={(event) => allowDrop(event, "root")}
    ondragleave={() => (dropTarget = null)}
    ondrop={(event) => void dropOn(event, [])}
  >
    <button type="button" class="root-label" onclick={() => void promptNewFolder([])}>
      + New root folder
    </button>
  </div>

  {#each folders as folder (folder.path.join("/"))}
    {@const all = flatten(folder)}
    {@const selected = all.filter((server) => catalog.selected.has(server.id))}
    <div
      class="folder-row"
      class:drop={dropTarget === folder.path.join("/")}
      style:padding-left="{12 + depth * 18}px"
      role="treeitem"
      aria-expanded={catalog.isExpanded(folder.path)}
      aria-selected={selected.length > 0}
      tabindex="0"
      draggable="true"
      ondragstart={(event) =>
        dragStart(event, `folder:${folderEntity(folder)?.id ?? ""}`)}
      ondragover={(event) => allowDrop(event, folder.path.join("/"))}
      ondragleave={() => (dropTarget = null)}
      ondrop={(event) => void dropOn(event, folder.path)}
      oncontextmenu={(event) => folderMenu(event, folder)}
    >
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
        class="toggle"
        onclick={() => catalog.toggleFolder(folder.path)}
        aria-label={catalog.isExpanded(folder.path) ? "Collapse" : "Expand"}
      >
        <span class="chevron" class:open={catalog.isExpanded(folder.path)}>›</span>
      </button>
      <span class="folder-icon" aria-hidden="true">▣</span>
      <span class="folder-name">{folder.name}</span>
      <span class="count">{all.length}</span>
    </div>

    {#if catalog.isExpanded(folder.path)}
      <Self folders={folder.folders} servers={folder.servers} depth={depth + 1} />
    {/if}
  {/each}

  {#each servers as server (server.id)}
    <div
      class="server-row"
      style:padding-left="{34 + depth * 18}px"
      draggable="true"
      role="treeitem"
      aria-selected={catalog.selected.has(server.id) ? "true" : "false"}
      tabindex="0"
      title="Double-click to connect · drag the row to a folder"
      ondragstart={(event) => dragStart(event, `server:${server.id}`)}
      ondblclick={() => catalog.connect([server])}
      oncontextmenu={(event) => serverMenu(event, server)}
    >
      <span class="drag-grip" aria-hidden="true"><Icon name="grip" size={12} /></span>
      <TriStateCheckbox
        checked={catalog.selected.has(server.id)}
        label={`Select ${server.displayName}`}
        onchange={() => catalog.toggleSelected(server.id)}
      />
      <ServerIcon {server} size={22} />
      <span class="server-name">{server.displayName}</span>
      <span class="server-subtitle">{server.subTitle}</span>
    </div>
  {/each}
</div>

{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={menu.items} onclose={() => (menu = null)} />
{/if}

<style>
  .tree {
    border: 1px solid var(--hr-border);
    border-radius: var(--hr-radius);
    overflow: hidden;
    background: color-mix(in srgb, var(--hr-primary-dark) 22%, transparent);
  }

  .root {
    padding: 4px 10px;
    border-bottom: 1px solid var(--hr-border);
    min-height: 32px;
    display: flex;
    align-items: center;
  }

  .root.drop,
  .folder-row.drop {
    outline: 2px dashed var(--hr-accent-mid);
    outline-offset: -2px;
    background: color-mix(in srgb, var(--hr-accent-mid) 10%, transparent);
  }

  .root-label {
    border: 0;
    background: transparent;
    color: color-mix(in srgb, var(--hr-background-text) 62%, transparent);
    font-size: var(--hr-font-size-small);
    padding: 2px 0;
  }

  .folder-row,
  .server-row {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 34px;
    padding: 3px 10px;
    border-bottom: 1px solid var(--hr-border);
  }

  .folder-row {
    background: color-mix(in srgb, var(--hr-primary-dark) 34%, transparent);
  }

  .folder-row:hover,
  .server-row:hover {
    background: color-mix(in srgb, var(--hr-accent-mid) 8%, transparent);
  }

  .server-row {
    cursor: grab;
  }

  .drag-grip {
    display: grid;
    place-items: center;
    color: color-mix(in srgb, var(--hr-background-text) 38%, transparent);
    cursor: grab;
  }

  .toggle {
    border: 0;
    background: transparent;
    color: inherit;
    width: 18px;
    padding: 0;
  }

  .chevron {
    display: inline-block;
    transition: transform 120ms ease;
  }

  .chevron.open {
    transform: rotate(90deg);
  }

  .folder-icon {
    color: var(--hr-accent-light);
    font-size: 12px;
  }

  .folder-name {
    font-weight: 600;
    font-size: var(--hr-font-size-small);
  }

  .count {
    font-size: 9px;
    border: 1px solid var(--hr-border);
    border-radius: 999px;
    padding: 0 6px;
    color: color-mix(in srgb, var(--hr-background-text) 50%, transparent);
  }

  .server-name {
    font-size: var(--hr-font-size-small);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .server-subtitle {
    font-size: 9px;
    color: color-mix(in srgb, var(--hr-background-text) 52%, transparent);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
