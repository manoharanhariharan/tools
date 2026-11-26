<script lang="ts">
  import Icon from "../../components/Icon.svelte";
  import { toCssColor } from "../../theme/apply";

  interface Props {
    colorHex: string;
    folderName: string;
    onclick: (event: MouseEvent) => void;
  }
  let { colorHex, folderName, onclick }: Props = $props();

  const colour = $derived(colorHex ? toCssColor(colorHex) : "");
</script>

<button
  type="button"
  class="color-button"
  title="Change folder colour"
  aria-label={`Change colour for ${folderName}`}
  {onclick}
>
  <Icon name="palette" size={12} />
  <span
    class="chip"
    class:empty={!colour}
    style={colour ? `background: ${colour}` : undefined}
  ></span>
</button>

<style>
  .color-button {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    flex: none;
    height: 22px;
    padding: 0 4px;
    border: 0;
    border-radius: var(--hr-radius-small);
    background: transparent;
    color: color-mix(in srgb, var(--hr-background-text) 55%, transparent);
    cursor: pointer;
  }

  .color-button:hover,
  .color-button:focus-visible {
    color: var(--hr-accent-light);
    background: color-mix(in srgb, var(--hr-accent-mid) 16%, transparent);
  }

  .chip {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    border: 1px solid var(--hr-border-strong);
  }

  .chip.empty {
    background: transparent;
    border-style: dashed;
  }
</style>
