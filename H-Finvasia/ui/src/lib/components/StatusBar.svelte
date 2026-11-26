<script lang="ts">
  import type { StatusDto } from "../api";

  interface Props {
    status: StatusDto | null;
  }
  let { status }: Props = $props();

  const sourceLabel = $derived(
    status ? { idle: "Idle", replay: "Replay", live: "Live" }[status.source] : "…",
  );
</script>

<footer class="statusbar">
  <span class:up={status?.connected} class:dim={!status?.connected}>
    <strong>{sourceLabel}</strong>
  </span>
  <span>backend <strong>{status?.backend ?? "…"}</strong></span>
  <span>instruments <strong>{status?.instruments?.toLocaleString() ?? "0"}</strong></span>
  <span>watching <strong>{status?.watched ?? 0}</strong>/{status?.watch_limit ?? 0}</span>
  <span>frames <strong>{status?.frames_total?.toLocaleString() ?? "0"}</strong></span>
  <span>rate <strong>{status?.frames_per_sec ?? 0}</strong>/s</span>
  {#if status && status.frames_error > 0}
    <span class="down">errors <strong>{status.frames_error}</strong></span>
  {/if}
  <span class="spacer"></span>
  {#if status?.last_error}
    <span class="warn" title={status.last_error}>⚠ {status.last_error.slice(0, 80)}</span>
  {/if}
  <span class="dim">h-finvasia {status?.build ?? ""}</span>
</footer>
