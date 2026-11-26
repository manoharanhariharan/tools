<script lang="ts">
  interface Props {
    title: string;
    hint: string;
    action?: string;
    busy?: boolean;
    onaction?: () => void;
  }
  let { title, hint, action, busy = false, onaction }: Props = $props();
</script>

<div class="empty">
  <h3>{title}</h3>
  <p class="hint" style="max-width: 420px">{hint}</p>
  {#if action && onaction}
    <button class="primary" disabled={busy} onclick={onaction}>
      {busy ? "Working…" : action}
    </button>
  {/if}
</div>
