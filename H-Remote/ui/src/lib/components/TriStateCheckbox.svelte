<script lang="ts">
  interface Props {
    checked: boolean;
    indeterminate?: boolean;
    label: string;
    onchange: () => void;
    disabled?: boolean;
  }
  let { checked, indeterminate = false, label, onchange, disabled = false }: Props = $props();
  let element = $state<HTMLInputElement | null>(null);

  $effect(() => {
    if (element) {
      element.indeterminate = indeterminate;
    }
  });
</script>

<input
  bind:this={element}
  type="checkbox"
  {checked}
  {disabled}
  aria-label={label}
  onclick={(event) => {
    event.stopPropagation();
    onchange();
  }}
  onchange={(event) => event.stopPropagation()}
/>
