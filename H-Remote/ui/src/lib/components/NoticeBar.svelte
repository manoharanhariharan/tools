<script lang="ts">
  import { app } from "../state/app.svelte";
  import Icon from "./Icon.svelte";
</script>

{#if app.notice}
  <div class="notice notice-{app.notice.kind}" role="alert">
    <Icon
      name={app.notice.kind === "error" ? "error" : app.notice.kind === "warning" ? "warning" : "info"}
      size={16}
    />
    <span>{app.notice.text}</span>
    <button type="button" aria-label="Dismiss" onclick={() => app.dismissNotice()}>
      <Icon name="close" size={14} />
    </button>
  </div>
{/if}

<style>
  .notice {
    position: fixed;
    top: calc(var(--hr-title-height) + 12px);
    right: 18px;
    z-index: 950;
    max-width: min(460px, calc(100vw - 36px));
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 11px 12px;
    border-radius: var(--hr-radius);
    border: 1px solid var(--hr-border-strong);
    background: color-mix(in srgb, var(--hr-primary-mid) 92%, var(--hr-background));
    box-shadow: var(--hr-shadow);
    animation: slide 180ms ease both;
  }

  .notice-info {
    border-left: 3px solid var(--hr-info);
  }

  .notice-warning {
    border-left: 3px solid var(--hr-warning);
  }

  .notice-error {
    border-left: 3px solid var(--hr-danger);
  }

  .notice span {
    flex: 1;
    line-height: 1.35;
  }

  .notice button {
    border: 0;
    background: transparent;
    color: inherit;
    padding: 2px;
    border-radius: var(--hr-radius-small);
  }

  .notice button:hover {
    background: color-mix(in srgb, var(--hr-primary-text) 12%, transparent);
  }

  @keyframes slide {
    from {
      opacity: 0;
      transform: translateX(10px);
    }
    to {
      opacity: 1;
      transform: translateX(0);
    }
  }
</style>
