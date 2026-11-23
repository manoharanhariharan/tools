<script lang="ts">
  import { onMount } from "svelte";

  export interface MenuItem {
    label?: string;
    action?: () => void;
    danger?: boolean;
    separator?: boolean;
    disabled?: boolean;
  }

  interface Props {
    x: number;
    y: number;
    items: MenuItem[];
    onclose: () => void;
  }
  let { x, y, items, onclose }: Props = $props();

  onMount(() => {
    const close = () => onclose();
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onclose();
      }
    };
    window.addEventListener("pointerdown", close);
    window.addEventListener("blur", close);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("pointerdown", close);
      window.removeEventListener("blur", close);
      window.removeEventListener("keydown", onKey);
    };
  });

  function activate(item: MenuItem) {
    if (item.disabled || !item.action) return;
    item.action();
    onclose();
  }
</script>

<div
  class="menu"
  style:left="{Math.min(x, window.innerWidth - 220)}px"
  style:top="{Math.min(y, window.innerHeight - 260)}px"
  role="menu"
  tabindex="-1"
  onpointerdown={(event) => event.stopPropagation()}
>
  {#each items as item, index (index)}
    {#if item.separator}
      <hr />
    {:else}
      <button
        type="button"
        role="menuitem"
        class:danger={item.danger}
        disabled={item.disabled}
        onclick={() => activate(item)}
      >
        {item.label}
      </button>
    {/if}
  {/each}
</div>

<style>
  .menu {
    position: fixed;
    z-index: 960;
    min-width: 190px;
    padding: 5px;
    border-radius: var(--hr-radius);
    border: 1px solid var(--hr-border-strong);
    background: color-mix(in srgb, var(--hr-primary-mid) 94%, var(--hr-background));
    box-shadow: var(--hr-shadow);
  }

  button {
    display: block;
    width: 100%;
    text-align: left;
    border: 0;
    background: transparent;
    color: inherit;
    padding: 7px 10px;
    border-radius: var(--hr-radius-small);
    font-size: var(--hr-font-size-small);
  }

  button:hover:not(:disabled) {
    background: color-mix(in srgb, var(--hr-accent-mid) 18%, transparent);
  }

  button.danger {
    color: var(--hr-danger);
  }

  button:disabled {
    opacity: 0.45;
  }

  hr {
    border: 0;
    border-top: 1px solid var(--hr-border);
    margin: 4px 6px;
  }
</style>
