<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    variant?: "primary" | "secondary" | "ghost" | "danger";
    size?: "sm" | "md";
    type?: "button" | "submit";
    disabled?: boolean;
    title?: string;
    ariaLabel?: string;
    testId?: string;
    onclick?: (event: MouseEvent) => void;
    children?: Snippet;
  }

  let {
    variant = "secondary",
    size = "md",
    type = "button",
    disabled = false,
    title,
    ariaLabel,
    testId,
    onclick,
    children,
  }: Props = $props();
</script>

<button
  class="hr-button hr-button-{variant} hr-button-{size}"
  {type}
  {disabled}
  {title}
  aria-label={ariaLabel}
  data-testid={testId}
  {onclick}
>
  {@render children?.()}
</button>

<style>
  .hr-button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
    border-radius: var(--hr-radius);
    border: 1px solid transparent;
    font-weight: 550;
    line-height: 1;
    transition:
      background var(--hr-transition),
      border-color var(--hr-transition),
      transform var(--hr-transition);
    white-space: nowrap;
  }

  .hr-button-md {
    min-height: 34px;
    padding: 0 14px;
  }

  .hr-button-sm {
    min-height: 28px;
    padding: 0 10px;
    font-size: var(--hr-font-size-small);
  }

  .hr-button:active:not(:disabled) {
    transform: translateY(1px);
  }

  .hr-button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .hr-button-primary {
    background: var(--hr-accent-mid);
    color: var(--hr-accent-text);
  }

  .hr-button-primary:hover:not(:disabled) {
    background: var(--hr-accent-light);
  }

  .hr-button-secondary {
    background: color-mix(in srgb, var(--hr-primary-light) 72%, transparent);
    border-color: var(--hr-border);
    color: var(--hr-primary-text);
  }

  .hr-button-secondary:hover:not(:disabled) {
    border-color: var(--hr-accent-mid);
  }

  .hr-button-ghost {
    background: transparent;
    color: var(--hr-primary-text);
  }

  .hr-button-ghost:hover:not(:disabled) {
    background: color-mix(in srgb, var(--hr-primary-text) 10%, transparent);
  }

  .hr-button-danger {
    background: var(--hr-danger);
    color: #fff;
  }

  .hr-button-danger:hover:not(:disabled) {
    background: color-mix(in srgb, var(--hr-danger) 82%, #fff);
  }
</style>
