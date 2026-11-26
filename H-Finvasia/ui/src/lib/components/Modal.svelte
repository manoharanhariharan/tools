<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    title: string;
    subtitle?: string;
    wide?: boolean;
    onclose: () => void;
    children: Snippet;
  }
  let { title, subtitle = "", wide = false, onclose, children }: Props = $props();
</script>

<svelte:window
  onkeydown={(event) => {
    if (event.key === "Escape") onclose();
  }}
/>

<div
  class="modal-backdrop"
  role="presentation"
  tabindex="-1"
  onclick={onclose}
  onkeydown={(event) => {
    if (event.key === "Escape") onclose();
  }}
>
  <div
    class="modal"
    class:wide
    role="dialog"
    aria-modal="true"
    aria-label={title}
    tabindex="-1"
    onclick={(event) => event.stopPropagation()}
    onkeydown={(event) => event.stopPropagation()}
  >
    <header class="modal-head">
      <div>
        <h2>{title}</h2>
        {#if subtitle}<p>{subtitle}</p>{/if}
      </div>
      <button class="ghost" title="Close" onclick={onclose}>x</button>
    </header>
    <div class="modal-body">
      {@render children()}
    </div>
  </div>
</div>
