<script lang="ts">
  interface Props {
    id: string;
    label: string;
    value: string;
    type?: "text" | "password" | "url" | "number";
    placeholder?: string;
    disabled?: boolean;
    error?: string | null;
    hint?: string;
    oninput?: (value: string) => void;
    onchange?: (value: string) => void;
    onkeydown?: (event: KeyboardEvent) => void;
  }

  let {
    id,
    label,
    value,
    type = "text",
    placeholder = "",
    disabled = false,
    error,
    hint,
    oninput,
    onchange,
    onkeydown,
  }: Props = $props();
</script>

<div class="hr-field" class:invalid={Boolean(error)}>
  <label for={id}>{label}</label>
  <input
    {id}
    {type}
    {value}
    {placeholder}
    {disabled}
    autocomplete="off"
    aria-invalid={error ? "true" : undefined}
    aria-describedby={error ? `${id}-error` : hint ? `${id}-hint` : undefined}
    oninput={(event) => oninput?.(event.currentTarget.value)}
    onchange={(event) => onchange?.(event.currentTarget.value)}
    {onkeydown}
  />
  {#if error}
    <span id="{id}-error" class="hr-field-error" role="alert">{error}</span>
  {:else if hint}
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

  input {
    min-height: 34px;
    border-radius: var(--hr-radius);
    border: 1px solid var(--hr-border);
    background: color-mix(in srgb, var(--hr-primary-dark) 55%, var(--hr-background));
    padding: 0 11px;
    color: var(--hr-background-text);
    transition: border-color var(--hr-transition);
  }

  input:hover:not(:disabled) {
    border-color: var(--hr-border-strong);
  }

  input:focus {
    border-color: var(--hr-accent-mid);
    outline: none;
  }

  input:disabled {
    opacity: 0.6;
  }

  .invalid input {
    border-color: var(--hr-danger);
  }

  .hr-field-error {
    color: var(--hr-danger);
    font-size: var(--hr-font-size-small);
  }

  .hr-field-hint {
    color: color-mix(in srgb, var(--hr-background-text) 58%, transparent);
    font-size: var(--hr-font-size-small);
  }
</style>
