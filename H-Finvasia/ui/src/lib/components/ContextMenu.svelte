<script lang="ts">
  import type { ContextCreator, ContextItem } from "./context";

  interface Props {
    x: number;
    y: number;
    items: ContextItem[];
    onclose: () => void;
    creator?: ContextCreator | null;
  }
  let { x, y, items, onclose, creator = null }: Props = $props();

  let openSub = $state<number | null>(null);
  let createValue = $state("");

  const left = $derived(Math.max(8, Math.min(x, (globalThis.innerWidth ?? 1200) - 220)));
  const top = $derived(
    Math.max(8, Math.min(y, (globalThis.innerHeight ?? 800) - 16 - items.length * 34)),
  );
  const openRight = $derived(x < 300);

  function hasChild(item: ContextItem): boolean {
    return (item.children?.length ?? 0) > 0 || creator !== null;
  }

  function activate(item: ContextItem) {
    if (item.disabled) return;
    item.action?.();
    onclose();
  }

  function create() {
    const value = createValue.trim();
    if (value === "" || !creator) return;
    creator.action(value);
    onclose();
  }
</script>

<svelte:window
  onclick={onclose}
  oncontextmenu={onclose}
  onkeydown={(event) => {
    if (event.key === "Escape") onclose();
  }}
/>

<div class="ctx-menu" style="left: {left}px; top: {top}px" role="menu" tabindex="-1">
  {#each items as item, index (item.label)}
    <div class="ctx-item-wrap">
      <button
        role="menuitem"
        aria-haspopup={hasChild(item) ? "menu" : undefined}
        disabled={item.disabled}
        onmouseenter={() => (openSub = hasChild(item) ? index : null)}
        onclick={(event) => {
          event.stopPropagation();
          if (hasChild(item)) openSub = index;
          else activate(item);
        }}
      >
        <span>{item.label}</span>
        {#if hasChild(item)}<span class="ctx-caret">{"<"}</span>{/if}
      </button>

      {#if openSub === index && hasChild(item)}
        <div
          class="ctx-submenu"
          class:open-right={openRight}
          role="menu"
          tabindex="-1"
          onclick={(event) => event.stopPropagation()}
          onkeydown={(event) => event.stopPropagation()}
        >
          {#each item.children ?? [] as child (child.label)}
            <button role="menuitem" disabled={child.disabled} onclick={() => activate(child)}>
              {child.label}
            </button>
          {/each}
          {#if creator}
            <div class="ctx-creator">
              <input
                bind:value={createValue}
                placeholder={creator.placeholder}
                aria-label={creator.placeholder}
                onkeydown={(event) => {
                  if (event.key === "Enter") create();
                  if (event.key === "Escape") {
                    createValue = "";
                    openSub = null;
                  }
                }}
              />
              <button disabled={createValue.trim() === ""} onclick={create}>{creator.label}</button>
            </div>
          {/if}
        </div>
      {/if}
    </div>
  {/each}
</div>
