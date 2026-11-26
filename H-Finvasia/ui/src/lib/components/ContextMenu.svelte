<script lang="ts">
  export interface ContextItem {
    label: string;
    disabled?: boolean;
    action: () => void;
  }

  interface Props {
    x: number;
    y: number;
    items: ContextItem[];
    onclose: () => void;
  }
  let { x, y, items, onclose }: Props = $props();

  const left = $derived(Math.max(8, Math.min(x, (globalThis.innerWidth ?? 1200) - 220)));
  const top = $derived(Math.max(8, Math.min(y, (globalThis.innerHeight ?? 800) - 16 - items.length * 34)));
</script>

<svelte:window
  onclick={onclose}
  oncontextmenu={onclose}
  onkeydown={(event) => {
    if (event.key === "Escape") onclose();
  }}
/>

<div class="ctx-menu" style="left: {left}px; top: {top}px" role="menu" tabindex="-1">
  {#each items as item (item.label)}
    <button
      role="menuitem"
      disabled={item.disabled}
      onclick={() => {
        if (item.disabled) return;
        item.action();
        onclose();
      }}>{item.label}</button
    >
  {/each}
</div>
