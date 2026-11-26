<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { i18n } from "../../i18n/index.svelte";
  import { app } from "../../state/app.svelte";
  import { catalog } from "../../state/catalog.svelte";
  import { dialogs } from "../../state/dialogs.svelte";
  import Button from "../../components/Button.svelte";
  import Icon from "../../components/Icon.svelte";
  import TriStateCheckbox from "../../components/TriStateCheckbox.svelte";
  import CatalogCards from "./CatalogCards.svelte";
  import CatalogList from "./CatalogList.svelte";
  import CatalogToolbar from "./CatalogToolbar.svelte";
  import CatalogTree from "./CatalogTree.svelte";
  import ImportExportDialog from "./ImportExportDialog.svelte";
  import ServerEditor from "./ServerEditor.svelte";
  import CatalogSidePanel from "./CatalogSidePanel.svelte";
  import {
    LIST_COLUMN_LABELS,
    listTemplate,
    visibleColumns,
  } from "./columns";

  let importOpen = $state(false);
  let dragColumn = $state<string | null>(null);
  let resizing = $state<{
    column: string;
    startX: number;
    startWidth: number;
  } | null>(null);

  const showNotes = $derived(app.config?.general.showNoteInList ?? true);
  const columns = $derived(catalog.columnWidths);
  const template = $derived(
    listTemplate(catalog.columnOrder, columns, showNotes),
  );
  const totalServers = $derived(catalog.snapshot?.servers.length ?? 0);

  onMount(() => {
    void catalog.load();
  });

  onDestroy(() => catalog.dispose());

  async function newFolder(): Promise<void> {
    const value = await dialogs.input({
      title: "New folder",
      label: "Folder path",
      placeholder: "Prod/EU",
      submitLabel: "Create",
    });
    if (!value?.trim()) return;
    const path = value
      .split("/")
      .map((segment) => segment.trim())
      .filter(Boolean);
    if (path.length === 0) return;
    await catalog.createFolder(path);
  }

  function startResize(event: PointerEvent, column: string): void {
    event.preventDefault();
    event.stopPropagation();
    resizing = {
      column,
      startX: event.clientX,
      startWidth: columns[column] ?? 140,
    };
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  }

  function moveResize(event: PointerEvent): void {
    if (!resizing) return;
    const width = Math.max(80, resizing.startWidth + event.clientX - resizing.startX);
    catalog.columnWidths = { ...catalog.columnWidths, [resizing.column]: width };
  }

  async function endResize(): Promise<void> {
    if (!resizing) return;
    const { column } = resizing;
    const width = catalog.columnWidths[column] ?? 140;
    resizing = null;
    await catalog.setColumnWidth(column, width);
  }

  function startColumnDrag(event: DragEvent, key: string): void {
    dragColumn = key;
    event.dataTransfer?.setData("text/plain", `column:${key}`);
    if (event.dataTransfer) {
      event.dataTransfer.effectAllowed = "move";
    }
  }

  function dropColumn(event: DragEvent, target: string): void {
    event.preventDefault();
    const from =
      dragColumn ??
      (event.dataTransfer?.getData("text/plain") ?? "").replace(/^column:/, "");
    dragColumn = null;
    if (from && from !== target) {
      void catalog.moveColumn(from, target);
    }
  }
</script>

<div class="catalog-page">
  <header class="page-head">
    <h1>{i18n.t("catalog.title")}</h1>
    <span>{totalServers} server{totalServers === 1 ? "" : "s"}</span>
    {#if catalog.folderFilter}
      <span class="folder-filter">
        / {catalog.folderFilter.join(" / ")}
        <button type="button" aria-label="Clear folder filter" onclick={() => catalog.clearFolderFilter()}>
          ×
        </button>
      </span>
    {/if}
  </header>
  <CatalogToolbar onimport={() => (importOpen = true)} onnewfolder={newFolder} />

  <div class="body">
    <CatalogSidePanel />

    <section class="main">
      {#if catalog.loading}
        <div class="state"><span class="spinner">◌</span> Loading catalog…</div>
      {:else if catalog.error}
        <div class="state error">
          <p>{catalog.error}</p>
          <Button variant="secondary" onclick={() => void catalog.load()}>Retry</Button>
        </div>
      {:else if totalServers === 0}
        <div class="empty">
          <Icon name="servers" size={34} />
          <h2>{i18n.t("catalog.empty")}</h2>
          <p>Add your first server or import an existing catalog.</p>
          <div class="hr-row">
            <Button variant="primary" onclick={() => void catalog.openAdd()}>
              <Icon name="plus" size={15} /> {i18n.t("catalog.add")}
            </Button>
            <Button variant="secondary" onclick={() => (importOpen = true)}>
              {i18n.t("catalog.import")}
            </Button>
          </div>
        </div>
      {:else if (catalog.view?.total ?? 0) === 0}
        <div class="empty">
          <Icon name="search" size={30} />
          {#if catalog.sourceFilter !== "all"}
            <h2>No servers in this source</h2>
            <p>
              {catalog.sourceLabel(catalog.sourceFilter)} is mounted read-only and does not contain
              matching servers yet.
            </p>
            <Button variant="secondary" onclick={() => void catalog.setSourceFilter("all")}>
              Show all sources
            </Button>
          {:else}
            <h2>No matches</h2>
            <p>Try a different keyword or remove a tag filter.</p>
            <Button variant="secondary" onclick={() => catalog.setQuery("")}>Clear search</Button>
          {/if}
        </div>
      {:else}
        {#if catalog.viewMode === "list"}
          <div class="header" style:grid-template-columns={template} role="row">
            <div class="cell center">
              <TriStateCheckbox
                checked={catalog.allSelected()}
                indeterminate={catalog.someSelected()}
                label="Select all visible servers"
                onchange={() => catalog.toggleSelectAll()}
              />
            </div>
            {#each visibleColumns(catalog.columnOrder, showNotes) as key (key)}
              <div
                class="cell head"
                class:with-resizer={key !== "target"}
                class:drag-over={dragColumn !== null && dragColumn !== key}
                data-testid={`column-head-${key}`}
                role="columnheader"
                tabindex="-1"
                ondragover={(event) => {
                  if (dragColumn) {
                    event.preventDefault();
                  }
                }}
                ondrop={(event) => dropColumn(event, key)}
              >
                <span class="column-grip">
                  <span
                    role="button"
                    tabindex="-1"
                    draggable="true"
                    title="Drag to reorder column"
                    aria-label={`Reorder ${LIST_COLUMN_LABELS[key]}`}
                    ondragstart={(event) => startColumnDrag(event, key)}
                    ondragend={() => (dragColumn = null)}
                  >
                    <Icon name="grip" size={10} />
                  </span>
                </span>
                <span class="head-label">{LIST_COLUMN_LABELS[key]}</span>
                {#if key !== "target"}
                  <span
                    class="resizer"
                    role="separator"
                    aria-label={`Resize ${LIST_COLUMN_LABELS[key]}`}
                    onpointerdown={(event) => startResize(event, key)}
                    onpointermove={moveResize}
                    onpointerup={() => void endResize()}
                    onpointercancel={() => void endResize()}
                  ></span>
                {/if}
              </div>
            {/each}
            <div class="cell head">Source</div>
          </div>
          <div class="scroll">
            <CatalogList
              folders={catalog.rootFolders()}
              servers={catalog.rootServers()}
              {columns}
              showNote={showNotes}
            />
          </div>
        {:else if catalog.viewMode === "card"}
          <div class="scroll card-scroll">
            <CatalogCards
              folders={catalog.rootFolders()}
              servers={catalog.rootServers()}
            />
          </div>
        {:else}
          <div class="scroll">
            <CatalogTree
              folders={catalog.rootFolders()}
              servers={catalog.rootServers()}
            />
          </div>
        {/if}
      {/if}
    </section>

    {#if catalog.editor}
      <ServerEditor />
    {/if}
  </div>
</div>

<ImportExportDialog open={importOpen} onclose={() => (importOpen = false)} />

<style>
  .catalog-page {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    /* The list uses fixed column widths; without an explicit zero minimum the
       page's min-content width pushes the editor off the right edge. */
    min-width: 0;
    width: 100%;
  }

  .page-head {
    display: flex;
    align-items: baseline;
    gap: 10px;
    padding: 5px 10px 3px;
  }

  .page-head h1 {
    font-size: var(--hr-font-size-title);
  }

  .page-head span {
    font-size: var(--hr-font-size-small);
    color: color-mix(in srgb, var(--hr-background-text) 52%, transparent);
  }

  .page-head .folder-filter {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    color: var(--hr-accent-light);
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .folder-filter button {
    border: 0;
    background: transparent;
    color: inherit;
    padding: 0 2px;
  }

  .body {
    display: flex;
    flex: 1;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }

  .main {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }

  .header {
    display: grid;
    align-items: center;
    gap: 10px;
    padding: 6px 10px;
    border-bottom: 1px solid var(--hr-border);
    background: color-mix(in srgb, var(--hr-primary-dark) 34%, transparent);
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: color-mix(in srgb, var(--hr-background-text) 55%, transparent);
    user-select: none;
  }

  .cell {
    min-width: 0;
    position: relative;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .head {
    display: flex;
    align-items: center;
    gap: 4px;
    cursor: grab;
  }

  .drag-over {
    background: color-mix(in srgb, var(--hr-accent-mid) 16%, transparent);
    outline: 1px dashed var(--hr-accent-mid);
    outline-offset: -2px;
  }

  .column-grip {
    display: inline-grid;
    place-items: center;
    flex: none;
    color: color-mix(in srgb, var(--hr-background-text) 30%, transparent);
  }

  .column-grip > span {
    display: inline-grid;
    place-items: center;
    cursor: grab;
  }

  .column-grip > span:hover {
    color: var(--hr-accent-light);
  }

  .head-label {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .center {
    display: flex;
    justify-content: center;
  }

  .resizer {
    position: absolute;
    right: -5px;
    top: 0;
    bottom: 0;
    width: 8px;
    cursor: col-resize;
    z-index: 2;
  }

  .resizer:hover {
    background: color-mix(in srgb, var(--hr-accent-mid) 60%, transparent);
  }

  .scroll {
    flex: 1;
    overflow: auto;
    min-height: 0;
  }

  .card-scroll {
    padding: 12px;
  }

  .state,
  .empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    text-align: center;
    padding: 28px;
  }

  .state.error {
    color: var(--hr-danger);
  }

  .empty h2 {
    font-size: var(--hr-font-size-subtitle);
  }

  .empty p {
    max-width: 46ch;
    color: color-mix(in srgb, var(--hr-background-text) 62%, transparent);
  }

  .spinner {
    animation: spin 900ms linear infinite;
    font-size: 20px;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }
</style>
