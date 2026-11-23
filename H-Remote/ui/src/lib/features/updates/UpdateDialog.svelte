<script lang="ts">
  import Modal from "../../components/Modal.svelte";
  import Button from "../../components/Button.svelte";
  import { commands } from "../../ipc/client";
  import { app } from "../../state/app.svelte";

  const status = $derived(app.updateStatus);

  async function download(): Promise<void> {
    const url = status?.url;
    app.dismissUpdate();
    if (url) {
      try {
        await commands.openUrl(url);
      } catch (error) {
        await app.reportError(error);
      }
    }
  }
</script>

<Modal
  open={app.updateDialogOpen}
  title={status?.breaking ? "Breaking update available" : "Update available"}
  description={status?.version ? `Version ${status.version} is available.` : undefined}
  onclose={() => app.dismissUpdate()}
>
  {#if status}
    <div class="hr-stack">
      <p class="version">
        Installed <strong>{status.current}</strong> → available
        <strong>{status.version ?? "unknown"}</strong>
        {#if status.prerelease}<span class="badge">pre-release</span>{/if}
      </p>
      {#if status.breaking}
        <p class="breaking" role="alert">
          This release requires a newer base version. Back up your profile before upgrading.
        </p>
      {/if}
      {#if status.notes}
        <pre class="notes">{status.notes}</pre>
      {/if}
      {#if status.url}
        <p class="hr-small hr-muted">The installer opens in your browser; the app is not updated silently.</p>
      {/if}
    </div>
  {/if}

  {#snippet footer()}
    <Button variant="ghost" onclick={() => app.dismissUpdate()}>Later</Button>
    <Button variant="primary" disabled={!status?.url} onclick={() => void download()}>
      Download
    </Button>
  {/snippet}
</Modal>

<style>
  .version {
    font-size: var(--hr-font-size-small);
  }

  .badge {
    margin-left: 6px;
    font-size: 9px;
    border: 1px solid var(--hr-border);
    border-radius: 999px;
    padding: 0 6px;
    color: var(--hr-accent-light);
  }

  .breaking {
    font-size: var(--hr-font-size-small);
    color: var(--hr-warning);
    border: 1px solid color-mix(in srgb, var(--hr-warning) 35%, transparent);
    border-radius: var(--hr-radius-small);
    padding: 7px 9px;
    background: color-mix(in srgb, var(--hr-warning) 10%, transparent);
  }

  .notes {
    margin: 0;
    max-height: 180px;
    overflow: auto;
    white-space: pre-wrap;
    font: inherit;
    font-size: var(--hr-font-size-small);
    color: color-mix(in srgb, var(--hr-background-text) 72%, transparent);
    border: 1px solid var(--hr-border);
    border-radius: var(--hr-radius-small);
    padding: 8px 10px;
  }
</style>
