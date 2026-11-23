<script lang="ts">
  interface Props {
    id: string;
    label: string;
    description?: string;
    checked: boolean;
    disabled?: boolean;
    onchange: (checked: boolean) => void;
  }

  let { id, label, description, checked, disabled = false, onchange }: Props = $props();
</script>

<div class="hr-toggle-row" class:disabled>
  <div class="hr-toggle-text">
    <span id="{id}-label" class="hr-toggle-label">{label}</span>
    {#if description}
      <span id="{id}-description" class="hr-toggle-description">{description}</span>
    {/if}
  </div>
  <button
    type="button"
    class="hr-toggle"
    role="switch"
    aria-checked={checked}
    aria-labelledby="{id}-label"
    aria-describedby={description ? `${id}-description` : undefined}
    {disabled}
    data-testid={id}
    onclick={() => onchange(!checked)}
  >
    <span class="hr-toggle-knob"></span>
  </button>
</div>

<style>
  .hr-toggle-row {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 18px;
    padding: 9px 0;
  }

  .hr-toggle-row.disabled {
    opacity: 0.55;
  }

  .hr-toggle-text {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }

  .hr-toggle-label {
    font-weight: 500;
  }

  .hr-toggle-description {
    color: color-mix(in srgb, var(--hr-background-text) 62%, transparent);
    font-size: var(--hr-font-size-small);
    max-width: 62ch;
  }

  .hr-toggle {
    position: relative;
    width: 42px;
    height: 24px;
    flex: none;
    border-radius: 999px;
    border: 1px solid var(--hr-border-strong);
    background: color-mix(in srgb, var(--hr-primary-text) 18%, transparent);
    transition: background var(--hr-transition);
    padding: 0;
  }

  .hr-toggle[aria-checked="true"] {
    background: var(--hr-accent-mid);
    border-color: var(--hr-accent-mid);
  }

  .hr-toggle-knob {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: #fff;
    transition: transform var(--hr-transition);
    box-shadow: 0 1px 3px rgb(0 0 0 / 0.35);
  }

  .hr-toggle[aria-checked="true"] .hr-toggle-knob {
    transform: translateX(18px);
  }
</style>
