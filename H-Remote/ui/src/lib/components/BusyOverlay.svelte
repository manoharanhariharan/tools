<script lang="ts">
  import { i18n } from "../i18n/index.svelte";
  import { busy } from "../state/busy.svelte";
  import Icon from "./Icon.svelte";
</script>

{#if busy.active}
  <div class="mask" role="presentation">
    <div class="busy" role="status" aria-live="polite" aria-busy="true">
      <span class="ring" aria-hidden="true">
        <Icon name="spinner" size={26} />
      </span>
      <span>{busy.message || i18n.t("busy.working")}</span>
    </div>
  </div>
{/if}

<style>
  .mask {
    position: fixed;
    inset: 0;
    z-index: 850;
    display: grid;
    place-items: center;
    background: color-mix(in srgb, var(--hr-background) 45%, transparent);
    backdrop-filter: blur(1.5px);
    cursor: progress;
  }

  .busy {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 16px 22px;
    border-radius: var(--hr-radius-large);
    background: color-mix(in srgb, var(--hr-primary-mid) 92%, var(--hr-background));
    border: 1px solid var(--hr-border-strong);
    box-shadow: var(--hr-shadow);
  }

  .ring {
    color: var(--hr-accent-light);
    display: grid;
    place-items: center;
    animation: spin 900ms linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
