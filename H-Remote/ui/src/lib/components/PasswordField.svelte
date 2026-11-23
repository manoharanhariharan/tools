<script lang="ts">
  import { app } from "../state/app.svelte";

  interface Props {
    id: string;
    label: string;
    value: string;
    placeholder?: string;
    hint?: string;
    disabled?: boolean;
    oninput: (value: string) => void;
  }
  let {
    id,
    label,
    value,
    placeholder = "",
    hint,
    disabled = false,
    oninput,
  }: Props = $props();

  let visible = $state(false);

  async function reveal(): Promise<void> {
    if (!visible && !(await app.ensureVerified("Reveal a stored password"))) {
      return;
    }
    visible = !visible;
  }

  async function copy(): Promise<void> {
    if (!value || !(await app.ensureVerified("Copy a stored password"))) {
      return;
    }
    try {
      await navigator.clipboard.writeText(value);
      app.showNotice("Password copied", "info");
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  function clear(): void {
    visible = false;
    oninput("");
  }
</script>

<div class="password-field">
  {#if label}
    <label for={id}>{label}</label>
  {/if}
  <div class="row">
    <input
      {id}
      type={visible ? "text" : "password"}
      {value}
      {placeholder}
      {disabled}
      autocomplete="off"
      spellcheck="false"
      oninput={(event) => oninput(event.currentTarget.value)}
    />
    <button type="button" class="icon" title={visible ? "Hide password" : "Reveal password"} aria-label={visible ? "Hide password" : "Reveal password"} onclick={() => void reveal()}>
      {visible ? "◉" : "◎"}
    </button>
    <button type="button" class="icon" title="Copy password" aria-label="Copy password" onclick={() => void copy()} disabled={!value}>⧉</button>
    <button type="button" class="icon danger" title="Clear password" aria-label="Clear password" onclick={clear} disabled={!value || disabled}>×</button>
  </div>
  {#if hint}
    <p class="hint">{hint}</p>
  {/if}
</div>

<style>
  .password-field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
    flex: 1;
  }

  label {
    font-size: var(--hr-font-size-small);
    color: color-mix(in srgb, var(--hr-background-text) 68%, transparent);
  }

  .row {
    display: flex;
    gap: 4px;
    align-items: center;
  }

  input {
    flex: 1;
    min-width: 0;
    min-height: 32px;
    border-radius: var(--hr-radius);
    border: 1px solid var(--hr-border);
    background: color-mix(in srgb, var(--hr-primary-dark) 50%, var(--hr-background));
    color: inherit;
    padding: 7px 9px;
    font: inherit;
    font-size: var(--hr-font-size-small);
  }

  .icon {
    border: 1px solid var(--hr-border);
    background: transparent;
    color: inherit;
    border-radius: var(--hr-radius-small);
    min-width: 30px;
    height: 32px;
  }

  .icon:hover:not(:disabled) {
    background: color-mix(in srgb, var(--hr-accent-mid) 14%, transparent);
  }

  .icon:disabled {
    opacity: 0.4;
  }

  .danger {
    color: var(--hr-danger);
  }

  .hint {
    font-size: 10px;
    color: color-mix(in srgb, var(--hr-background-text) 52%, transparent);
  }
</style>
