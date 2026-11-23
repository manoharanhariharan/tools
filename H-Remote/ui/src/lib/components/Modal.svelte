<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";

  interface Props {
    open: boolean;
    title: string;
    description?: string;
    width?: "sm" | "md" | "lg";
    dismissible?: boolean;
    onclose: () => void;
    children?: Snippet;
    footer?: Snippet;
  }

  let {
    open,
    title,
    description,
    width = "md",
    dismissible = true,
    onclose,
    children,
    footer,
  }: Props = $props();

  let panel = $state<HTMLDivElement | null>(null);
  const titleId = `modal-title-${Math.random().toString(36).slice(2, 9)}`;

  $effect(() => {
    if (!open) {
      return;
    }
    const element = panel;
    if (!element) {
      return;
    }
    const previous = document.activeElement as HTMLElement | null;
    const focusable = () =>
      Array.from(
        element.querySelectorAll<HTMLElement>(
          'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
        ),
      ).filter((node) => node.offsetParent !== null || node === document.activeElement);

    const first = focusable()[0];
    first?.focus();

    const onKeydown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && dismissible) {
        event.preventDefault();
        onclose();
        return;
      }
      if (event.key !== "Tab") {
        return;
      }
      const nodes = focusable();
      if (nodes.length === 0) {
        event.preventDefault();
        return;
      }
      const start = nodes[0];
      const end = nodes[nodes.length - 1];
      if (event.shiftKey && document.activeElement === start) {
        event.preventDefault();
        end.focus();
      } else if (!event.shiftKey && document.activeElement === end) {
        event.preventDefault();
        start.focus();
      }
    };

    document.addEventListener("keydown", onKeydown, true);
    return () => {
      document.removeEventListener("keydown", onKeydown, true);
      previous?.focus?.();
    };
  });
</script>

{#if open}
  <div
    class="hr-modal-layer"
    role="presentation"
    onmousedown={(event) => {
      if (dismissible && event.target === event.currentTarget) {
        onclose();
      }
    }}
  >
    <div
      class="hr-modal hr-modal-{width} hr-fade-in"
      role="dialog"
      aria-modal="true"
      aria-labelledby={titleId}
      bind:this={panel}
    >
      <header>
        <div class="titles">
          <h2 id={titleId}>{title}</h2>
          {#if description}<p>{description}</p>{/if}
        </div>
        {#if dismissible}
          <button type="button" class="close" aria-label="Close dialog" onclick={onclose}>
            <Icon name="close" size={16} />
          </button>
        {/if}
      </header>
      <div class="body">
        {@render children?.()}
      </div>
      {#if footer}
        <footer>{@render footer()}</footer>
      {/if}
    </div>
  </div>
{/if}

<style>
  .hr-modal-layer {
    position: fixed;
    inset: 0;
    z-index: 900;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 24px;
    background: var(--hr-overlay);
    backdrop-filter: blur(2px);
  }

  .hr-modal {
    width: min(100%, 560px);
    max-height: calc(100vh - 48px);
    display: flex;
    flex-direction: column;
    background: color-mix(in srgb, var(--hr-primary-mid) 86%, var(--hr-background));
    border: 1px solid var(--hr-border-strong);
    border-radius: var(--hr-radius-large);
    box-shadow: var(--hr-shadow);
    overflow: hidden;
  }

  .hr-modal-sm {
    width: min(100%, 420px);
  }

  .hr-modal-lg {
    width: min(100%, 820px);
  }

  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    padding: 18px 20px 12px;
  }

  .titles p {
    margin-top: 5px;
    color: color-mix(in srgb, var(--hr-background-text) 66%, transparent);
    font-size: var(--hr-font-size-small);
  }

  .close {
    border: 0;
    background: transparent;
    color: inherit;
    padding: 6px;
    border-radius: var(--hr-radius-small);
  }

  .close:hover {
    background: color-mix(in srgb, var(--hr-primary-text) 12%, transparent);
  }

  .body {
    padding: 4px 20px 18px;
    overflow: auto;
  }

  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 14px 20px;
    border-top: 1px solid var(--hr-border);
    background: color-mix(in srgb, var(--hr-primary-dark) 35%, transparent);
  }
</style>
