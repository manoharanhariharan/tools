<script lang="ts">
  import { onMount } from "svelte";
  import Modal from "../../components/Modal.svelte";
  import Button from "../../components/Button.svelte";
  import { commands } from "../../ipc/client";
  import { app } from "../../state/app.svelte";

  const profile = $derived(app.bootstrap?.legacy ?? null);
  let database = $state<string | null>(null);

  onMount(() => {
    void (async () => {
      try {
        database = await commands.legacyDatabase();
      } catch {
        database = null;
      }
    })();
  });

  const kindLabel = $derived(
    profile?.kind === "premoteM" ? "PRemoteM" : profile?.kind === "oneRemote" ? "1Remote" : "legacy",
  );
</script>

<Modal
  open={app.legacyOpen && Boolean(profile)}
  title={`${kindLabel} profile detected`}
  description={profile?.path ?? ""}
  width="sm"
  onclose={() => void app.dismissLegacyMigration()}
>
  <div class="hr-stack">
    <p class="hr-small hr-muted">
      H-Remote found an existing profile. Importing settings copies supported options; importing
      servers reads the catalog database into the local source. Nothing is deleted.
    </p>
    {#if database}
      <p class="hr-small">Catalog: <code>{database}</code></p>
    {:else}
      <p class="hr-small hr-muted">No legacy catalog database was found next to the profile.</p>
    {/if}
  </div>

  {#snippet footer()}
    <Button variant="ghost" onclick={() => void app.dismissLegacyMigration()}>Not now</Button>
    <Button variant="secondary" onclick={() => void app.importLegacySettings()}>
      Import settings
    </Button>
    <Button variant="primary" disabled={!database} onclick={() => void app.importLegacyServers()}>
      Import servers
    </Button>
  {/snippet}
</Modal>

<style>
  code {
    font-size: 10px;
    word-break: break-all;
  }
</style>
