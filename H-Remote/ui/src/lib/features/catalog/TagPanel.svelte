<script lang="ts">
  import type { Server, Tag } from "../../ipc/contracts";
  import { app } from "../../state/app.svelte";
  import { catalog } from "../../state/catalog.svelte";
  import { dialogs } from "../../state/dialogs.svelte";
  import { i18n } from "../../i18n/index.svelte";

  interface Props {
    embedded?: boolean;
  }
  let { embedded = false }: Props = $props();

  let filter = $state("");
  let menuOpen = $state<string | null>(null);

  const tags = $derived(
    [...catalog.tags()]
      .filter((tag) => tag.name.includes(filter.trim().toLowerCase()))
      .sort((left, right) => {
        if (left.isPinned !== right.isPinned) return left.isPinned ? -1 : 1;
        return left.customOrder - right.customOrder || left.name.localeCompare(right.name);
      }),
  );

  function serversFor(tag: string): Server[] {
    return (catalog.snapshot?.servers ?? []).filter((server) =>
      server.tags.includes(tag),
    );
  }

  async function rename(tag: Tag): Promise<void> {
    const name = await dialogs.input({
      title: "Rename tag",
      label: "Tag name",
      value: tag.name,
      submitLabel: "Rename",
    });
    if (name?.trim()) {
      await catalog.renameTag(tag.name, name);
    }
  }

  async function remove(tag: Tag): Promise<void> {
    const confirmed = await dialogs.confirm({
      title: `Delete tag #${tag.name}?`,
      message: `The tag is removed from ${tag.itemsCount} server(s).`,
      danger: true,
    });
    if (confirmed) {
      await catalog.deleteTag(tag.name);
    }
  }

  async function move(tag: Tag, delta: number): Promise<void> {
    const order = tags.map((candidate) => candidate.name);
    const index = order.indexOf(tag.name);
    const target = index + delta;
    if (index < 0 || target < 0 || target >= order.length) return;
    [order[index], order[target]] = [order[target], order[index]];
    await catalog.setTagOrder(order);
  }

  async function connect(tag: Tag, newWindow: boolean): Promise<void> {
    const servers = serversFor(tag.name);
    if (newWindow) {
      app.showNotice(
        `New tab window requested for ${servers.length} '${tag.name}' sessions (Phase 4).`,
        "info",
      );
    } else {
      await catalog.connect(servers);
    }
  }

  async function shortcut(tag: Tag): Promise<void> {
    await catalog.createTagShortcut(tag.name, false);
  }
</script>

<aside class="tags" class:embedded>
  <header>
    <h2>{i18n.t("common.search")} tags</h2>
    <span>{tags.length}</span>
  </header>
  <input
    type="search"
    placeholder="Filter tags"
    aria-label="Filter tags"
    bind:value={filter}
  />
  <div class="list">
    {#each tags as tag (tag.name)}
      <div class="tag-row" class:active={catalog.activeTagFilters().includes(tag.name)}>
        <button type="button" class="tag-main" onclick={() => catalog.toggleTagFilter(tag.name)}>
          <span class="pin">{tag.isPinned ? "★" : ""}</span>
          <span>#{tag.name}</span>
          <span class="count">{tag.itemsCount}</span>
        </button>
        <button
          type="button"
          class="more"
          aria-label={`Actions for ${tag.name}`}
          onclick={() => (menuOpen = menuOpen === tag.name ? null : tag.name)}
        >
          ⋯
        </button>
        {#if menuOpen === tag.name}
          <div class="tag-menu" role="menu">
            <button type="button" role="menuitem" onclick={() => void catalog.togglePinTag(tag.name)}>
              {tag.isPinned ? "Unpin" : "Pin"}
            </button>
            <button type="button" role="menuitem" onclick={() => void connect(tag, false)}>
              Connect all
            </button>
            <button type="button" role="menuitem" onclick={() => void connect(tag, true)}>
              Connect in new window
            </button>
            <button type="button" role="menuitem" onclick={() => void shortcut(tag)}>
              Create shortcut
            </button>
            <button type="button" role="menuitem" onclick={() => void move(tag, -1)}>
              Move up
            </button>
            <button type="button" role="menuitem" onclick={() => void move(tag, 1)}>
              Move down
            </button>
            <button type="button" role="menuitem" onclick={() => void rename(tag)}>
              Rename
            </button>
            <button type="button" role="menuitem" class="danger" onclick={() => void remove(tag)}>
              Delete
            </button>
          </div>
        {/if}
      </div>
    {/each}
    {#if tags.length === 0}
      <p class="empty">No tags</p>
    {/if}
  </div>
</aside>

<style>
  .tags {
    width: 196px;
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 9px;
    padding: 10px;
    border-right: 1px solid var(--hr-border);
    background: color-mix(in srgb, var(--hr-primary-dark) 22%, transparent);
    min-height: 0;
  }

  .tags.embedded {
    width: auto;
    flex: 1;
    padding: 0;
    border-right: 0;
    background: transparent;
  }

  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
  }

  h2 {
    font-size: var(--hr-font-size-small);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: color-mix(in srgb, var(--hr-background-text) 58%, transparent);
  }

  header span {
    font-size: 9px;
    color: color-mix(in srgb, var(--hr-background-text) 45%, transparent);
  }

  input {
    min-height: 30px;
    border-radius: var(--hr-radius);
    border: 1px solid var(--hr-border);
    background: color-mix(in srgb, var(--hr-primary-dark) 50%, var(--hr-background));
    color: inherit;
    padding: 0 8px;
    font-size: var(--hr-font-size-small);
  }

  .list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    overflow: auto;
    min-height: 0;
  }

  .tag-row {
    display: flex;
    align-items: center;
    gap: 2px;
    border-radius: var(--hr-radius-small);
    position: relative;
  }

  .tag-row:hover,
  .tag-row.active {
    background: color-mix(in srgb, var(--hr-accent-mid) 12%, transparent);
  }

  .tag-main {
    display: flex;
    align-items: center;
    gap: 5px;
    flex: 1;
    min-width: 0;
    border: 0;
    background: transparent;
    color: inherit;
    padding: 5px 4px;
    text-align: left;
    font-size: var(--hr-font-size-small);
  }

  .tag-main > span:nth-child(2) {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .pin {
    width: 10px;
    color: var(--hr-warning);
    font-size: 9px;
  }

  .count {
    margin-left: auto;
    font-size: 9px;
    color: color-mix(in srgb, var(--hr-background-text) 45%, transparent);
  }

  .more {
    border: 0;
    background: transparent;
    color: inherit;
    padding: 3px 5px;
    border-radius: 3px;
    opacity: 0.5;
  }

  .more:hover {
    opacity: 1;
    background: color-mix(in srgb, var(--hr-primary-text) 10%, transparent);
  }

  .tag-menu {
    position: absolute;
    right: 0;
    top: 100%;
    z-index: 30;
    min-width: 175px;
    padding: 4px;
    border-radius: var(--hr-radius);
    border: 1px solid var(--hr-border-strong);
    background: color-mix(in srgb, var(--hr-primary-mid) 96%, var(--hr-background));
    box-shadow: var(--hr-shadow);
  }

  .tag-menu button {
    display: block;
    width: 100%;
    text-align: left;
    border: 0;
    background: transparent;
    color: inherit;
    padding: 6px 8px;
    border-radius: var(--hr-radius-small);
    font-size: var(--hr-font-size-small);
  }

  .tag-menu button:hover {
    background: color-mix(in srgb, var(--hr-accent-mid) 16%, transparent);
  }

  .tag-menu .danger {
    color: var(--hr-danger);
  }

  .empty {
    font-size: var(--hr-font-size-small);
    color: color-mix(in srgb, var(--hr-background-text) 45%, transparent);
    padding: 6px 4px;
  }
</style>
