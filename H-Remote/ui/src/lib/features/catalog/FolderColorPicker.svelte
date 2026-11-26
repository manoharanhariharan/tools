<script lang="ts">
  import { onMount } from "svelte";
  import { toCssColor } from "../../theme/apply";

  interface Props {
    x: number;
    y: number;
    value: string;
    onpick: (colorHex: string) => void;
    onclose: () => void;
  }
  let { x, y, value, onpick, onclose }: Props = $props();

  const PRESETS = [
    "#FF4FA3D1",
    "#FF3EA66B",
    "#FFE0A23C",
    "#FFD9534F",
    "#FFB785F0",
    "#FF5CC9C4",
    "#FF8A9BA8",
    "#FFE07AB0",
  ];

  const width = 214;
  const height = 132;
  const left = $derived(`${Math.max(8, Math.min(x, window.innerWidth - width - 8))}px`);
  const top = $derived(`${Math.max(8, Math.min(y, window.innerHeight - height - 8))}px`);

  onMount(() => {
    const close = () => onclose();
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onclose();
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

  const current = $derived(value ? toCssColor(value) : "");
</script>

<div
  class="picker"
  style:left
  style:top
  role="dialog"
  tabindex="-1"
  aria-label="Folder colour"
  onpointerdown={(event) => event.stopPropagation()}
>
  <div class="swatches">
    {#each PRESETS as preset (preset)}
      <button
        type="button"
        class="swatch"
        class:selected={current.toUpperCase() === preset.toUpperCase()}
        style:background={toCssColor(preset)}
        title={preset}
        aria-label={`Use colour ${preset}`}
        onclick={() => onpick(preset)}
      ></button>
    {/each}
  </div>
  <div class="actions">
    <button type="button" class="reset" onclick={() => onpick("")}>No colour</button>
    <button type="button" class="close" onclick={onclose}>Close</button>
  </div>
</div>

<style>
  .picker {
    position: fixed;
    z-index: 970;
    width: 214px;
    padding: 8px;
    border-radius: var(--hr-radius);
    border: 1px solid var(--hr-border-strong);
    background: color-mix(in srgb, var(--hr-primary-mid) 94%, var(--hr-background));
    box-shadow: var(--hr-shadow);
  }

  .swatches {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 6px;
  }

  .swatch {
    height: 22px;
    border-radius: var(--hr-radius-small);
    border: 1px solid var(--hr-border-strong);
    cursor: pointer;
  }

  .swatch:hover {
    border-color: var(--hr-accent-light);
  }

  .swatch.selected {
    outline: 2px solid var(--hr-accent-light);
    outline-offset: 1px;
  }

  .actions {
    display: flex;
    justify-content: space-between;
    gap: 6px;
    margin-top: 8px;
  }

  .actions button {
    border: 1px solid var(--hr-border);
    border-radius: var(--hr-radius-small);
    background: transparent;
    color: inherit;
    font-size: var(--hr-font-size-small);
    padding: 3px 8px;
    cursor: pointer;
  }

  .actions button:hover {
    background: color-mix(in srgb, var(--hr-accent-mid) 16%, transparent);
  }
</style>
