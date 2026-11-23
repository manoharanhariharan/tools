<script lang="ts">
  interface SelectOption {
    value: string;
    label: string;
    disabled?: boolean;
  }

  interface Props {
    id: string;
    label: string;
    value: string;
    options: SelectOption[];
    disabled?: boolean;
    hint?: string;
    onchange: (value: string) => void;
  }

  let { id, label, value, options, disabled = false, hint, onchange }: Props = $props();
</script>

<div class="hr-field">
  <label for={id}>{label}</label>
  <select
    {id}
    {value}
    {disabled}
    aria-describedby={hint ? `${id}-hint` : undefined}
    onchange={(event) => onchange(event.currentTarget.value)}
  >
    {#each options as option (option.value)}
      <option value={option.value} disabled={option.disabled}>{option.label}</option>
    {/each}
  </select>
  {#if hint}
    <span id="{id}-hint" class="hr-field-hint">{hint}</span>
  {/if}
</div>

<style>
  .hr-field {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }

  label {
    font-weight: 500;
    font-size: var(--hr-font-size-small);
    color: color-mix(in srgb, var(--hr-background-text) 82%, transparent);
  }

  select {
    min-height: 34px;
    border-radius: var(--hr-radius);
    border: 1px solid var(--hr-border);
    background: color-mix(in srgb, var(--hr-primary-dark) 55%, var(--hr-background));
    padding: 0 9px;
    color: var(--hr-background-text);
    transition: border-color var(--hr-transition);
  }

  select:focus {
    border-color: var(--hr-accent-mid);
    outline: none;
  }

  .hr-field-hint {
    color: color-mix(in srgb, var(--hr-background-text) 58%, transparent);
    font-size: var(--hr-font-size-small);
  }
</style>
