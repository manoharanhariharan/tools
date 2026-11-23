<script lang="ts">
  import { i18n } from "../../i18n/index.svelte";
  import { app } from "../../state/app.svelte";
  import { catalog } from "../../state/catalog.svelte";
  import Button from "../../components/Button.svelte";
  import Icon from "../../components/Icon.svelte";

  interface Props {
    onimport: () => void;
    onnewfolder: () => void;
  }
  let { onimport, onnewfolder }: Props = $props();

  const count = $derived(catalog.selected.size);
  const showNotes = $derived(app.config?.general.showNoteInList ?? true);

  async function shortcut(): Promise<void> {
    await catalog.shortcutSelected(false);
  }
  async function exportCatalog(): Promise<void> {
    if (!(await app.ensureVerified("Export credentials"))) {
      return;
    }
    await catalog.exportSelected();
  }
</script>

<div class="toolbar">
  <div class="search">
    <Icon name="search" size={16} />
    <input
      type="search"
      value={catalog.query}
      placeholder="Search servers, #tags, -#excluded…"
      aria-label={i18n.t("catalog.searchPlaceholder")}
      oninput={(event) => catalog.setQuery(event.currentTarget.value)}
    />
    {#if catalog.query}
      <button type="button" class="clear" aria-label="Clear search" onclick={() => catalog.setQuery("")}>×</button>
    {/if}
  </div>

  <div class="view-switch" role="group" aria-label="View">
    {#each [["list", "☰"], ["card", "▦"], ["tree", "▤"]] as [mode, glyph] (mode)}
      <button
        type="button"
        class:active={catalog.viewMode === mode}
        aria-pressed={catalog.viewMode === mode}
        aria-label={`${mode} view`}
        title={`${mode} view`}
        onclick={() => void catalog.setViewMode(mode as "list" | "card" | "tree")}
      >
        {glyph}
      </button>
    {/each}
  </div>

  <select value={catalog.sort} aria-label="Sort servers" onchange={(event) => void catalog.setSort(event.currentTarget.value)}>
    <option value="insertion">Insertion order</option>
    <option value="protocol">Protocol</option>
    <option value="name">Name</option>
    <option value="address">Address</option>
    <option value="custom">Custom</option>
  </select>

  {#if catalog.hasMultipleSources()}
    <select
      value={catalog.sourceFilter}
      aria-label="Filter by data source"
      onchange={(event) => void catalog.setSourceFilter(event.currentTarget.value)}
    >
      <option value="all">All sources</option>
      {#each catalog.configuredSources() as source (source.id)}
        <option value={source.id}>
          {source.name}{source.state === "ok" ? "" : " — offline"}
        </option>
      {/each}
    </select>
  {/if}

  <label class="notes-toggle" title={i18n.t("general.noteInList")}>
    <input type="checkbox" checked={showNotes} onchange={() => void catalog.toggleNotes()} />
    Notes
  </label>

  <Button size="sm" variant="secondary" title="Create a folder at the selected path" onclick={onnewfolder}>
    <Icon name="folder" size={15} /> Folder
  </Button>
  <Button size="sm" variant="secondary" title="Import JSON, CSV, .rdp or a 1Remote database" onclick={onimport}>
    <Icon name="folder" size={15} /> {i18n.t("catalog.import")}
  </Button>
  <Button size="sm" variant="secondary" title="Export the selection as unencrypted JSON" onclick={() => void exportCatalog()}>
    {i18n.t("catalog.export")}
  </Button>
  <Button size="sm" variant="primary" title="Add a server" onclick={() => void catalog.openAdd()}>
    <Icon name="plus" size={15} /> {i18n.t("catalog.add")}
  </Button>
  <Button size="sm" variant="ghost" title={i18n.t("common.refresh")} onclick={() => void catalog.refreshAllSources()}>
    <Icon name="refresh" size={15} />
  </Button>
</div>

{#if count > 0}
  <div class="bulk">
    <strong>{count} selected</strong>
    <Button size="sm" variant="secondary" onclick={() => void catalog.connect(catalog.selectedServers())}>Connect</Button>
    <Button size="sm" variant="secondary" disabled={count < 2} onclick={() => catalog.openBulk(catalog.selectedServers())}>Edit</Button>
    <Button size="sm" variant="secondary" onclick={() => void catalog.duplicateSelected()}>Duplicate</Button>
    <Button size="sm" variant="secondary" onclick={() => void shortcut()}>Shortcut</Button>
    <Button size="sm" variant="secondary" onclick={onimport}>Export</Button>
    <Button size="sm" variant="danger" onclick={() => void catalog.deleteSelected()}>Delete</Button>
    <Button size="sm" variant="ghost" onclick={() => catalog.clearSelection()}>Cancel</Button>
  </div>
{/if}

{#if catalog.tags().some((tag) => tag.isPinned)}
  <div class="pinned">
    <span>Pinned</span>
    {#each catalog.tags().filter((tag) => tag.isPinned) as tag (tag.name)}
      <button type="button" class:active={catalog.activeTagFilters().includes(tag.name)} onclick={() => catalog.toggleTagFilter(tag.name)}>
        #{tag.name}
      </button>
    {/each}
  </div>
{/if}

<style>
  .toolbar {
    display: flex;
    align-items: center;
    gap: 7px;
    flex-wrap: wrap;
    padding: 8px 12px;
    border-bottom: 1px solid var(--hr-border);
    background: color-mix(in srgb, var(--hr-primary-dark) 24%, transparent);
  }

  .search {
    display: flex;
    align-items: center;
    gap: 7px;
    flex: 1;
    min-width: 220px;
    padding: 0 9px;
    border-radius: var(--hr-radius);
    border: 1px solid var(--hr-border);
    background: color-mix(in srgb, var(--hr-primary-dark) 45%, var(--hr-background));
    height: 32px;
  }

  .search input {
    flex: 1;
    border: 0;
    background: transparent;
    color: inherit;
    outline: none;
    min-width: 0;
    font-size: var(--hr-font-size-small);
  }

  .clear {
    border: 0;
    background: transparent;
    color: inherit;
    opacity: 0.6;
  }

  .view-switch {
    display: flex;
    border: 1px solid var(--hr-border);
    border-radius: var(--hr-radius);
    overflow: hidden;
  }

  .view-switch button {
    border: 0;
    background: transparent;
    color: color-mix(in srgb, var(--hr-background-text) 62%, transparent);
    width: 30px;
    height: 32px;
  }

  .view-switch button.active {
    background: color-mix(in srgb, var(--hr-accent-mid) 20%, transparent);
    color: var(--hr-accent-light);
  }

  select {
    height: 32px;
    border-radius: var(--hr-radius);
    border: 1px solid var(--hr-border);
    background: color-mix(in srgb, var(--hr-primary-dark) 45%, var(--hr-background));
    color: inherit;
    font-size: var(--hr-font-size-small);
    padding: 0 7px;
  }

  .notes-toggle {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: var(--hr-font-size-small);
    color: color-mix(in srgb, var(--hr-background-text) 62%, transparent);
  }

  .bulk {
    display: flex;
    align-items: center;
    gap: 7px;
    flex-wrap: wrap;
    padding: 7px 12px;
    border-bottom: 1px solid var(--hr-border);
    background: color-mix(in srgb, var(--hr-accent-mid) 9%, transparent);
    font-size: var(--hr-font-size-small);
  }

  .bulk strong {
    margin-right: 5px;
    color: var(--hr-accent-light);
  }

  .pinned {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 12px;
    border-bottom: 1px solid var(--hr-border);
    font-size: 9px;
    overflow-x: auto;
  }

  .pinned span {
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: color-mix(in srgb, var(--hr-background-text) 45%, transparent);
  }

  .pinned button {
    border: 1px solid var(--hr-border);
    background: transparent;
    color: var(--hr-accent-light);
    border-radius: 999px;
    padding: 2px 8px;
    font-size: 9px;
  }

  .pinned button.active {
    background: color-mix(in srgb, var(--hr-accent-mid) 18%, transparent);
    border-color: var(--hr-accent-mid);
  }
</style>
