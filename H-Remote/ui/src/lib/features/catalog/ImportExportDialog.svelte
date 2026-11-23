<script lang="ts">
  import type { ImportOutcome } from "../../ipc/contracts";
  import { commands } from "../../ipc/client";
  import { i18n } from "../../i18n/index.svelte";
  import { app } from "../../state/app.svelte";
  import { catalog } from "../../state/catalog.svelte";
  import Button from "../../components/Button.svelte";
  import Modal from "../../components/Modal.svelte";

  interface Props {
    open: boolean;
    onclose: () => void;
  }
  let { open, onclose }: Props = $props();
  let tab = $state<"json" | "csv" | "rdp" | "db" | "export">("json");
  let text = $state("");
  let rdpName = $state("");
  let rdpPassword = $state("");
  let dbPath = $state("");
  let target = $state("local");
  let outcome = $state<ImportOutcome | null>(null);
  let busy = $state(false);

  async function readFile(event: Event): Promise<void> {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    text = await file.text();
    if (!rdpName) rdpName = file.name.replace(/\.rdp$/i, "");
  }

  async function runImport(): Promise<void> {
    busy = true;
    outcome = null;
    try {
      let result: ImportOutcome;
      if (tab === "json") {
        result = await commands.importJson(text);
      } else if (tab === "csv") {
        result = await commands.importCsv(text);
      } else if (tab === "rdp") {
        result = await commands.importRdp(
          text,
          rdpName || "Imported RDP",
          rdpPassword || null,
        );
      } else {
        result = await commands.importDatabase(dbPath);
      }
      outcome = result;
      await catalog.load();
    } catch (reason) {
      await app.reportError(reason);
    } finally {
      busy = false;
    }
  }

  async function exportSelected(): Promise<void> {
    if (!(await app.ensureVerified("Export credentials"))) {
      return;
    }
    await catalog.exportSelected();
  }

  async function exportRdp(): Promise<void> {
    if (!(await app.ensureVerified("Export an .rdp file"))) {
      return;
    }
    for (const server of catalog.selectedServers()) {
      try {
        const result = await commands.exportRdpServer(server.id);
        const blob = new Blob([result.content], { type: "application/x-rdp" });
        const url = URL.createObjectURL(blob);
        const anchor = document.createElement("a");
        anchor.href = url;
        anchor.download = `${server.displayName.replace(/[\\/:*?"<>|]/g, "_")}.rdp`;
        anchor.click();
        URL.revokeObjectURL(url);
        if (result.warning) {
          app.showNotice(result.warning, "warning");
        }
      } catch (reason) {
        await app.reportError(reason);
      }
    }
  }
</script>

<Modal {open} title={i18n.t("catalog.import") + " / " + i18n.t("catalog.export")} width="lg" onclose={onclose}>
  <div class="tabs">
    {#each [
      ["json", "JSON"],
      ["csv", "mRemoteNG CSV"],
      ["rdp", ".rdp"],
      ["db", "Database"],
      ["export", "Export"],
    ] as [id, label] (id)}
      <button type="button" class:active={tab === id} onclick={() => { tab = id as typeof tab; outcome = null; }}>
        {label}
      </button>
    {/each}
  </div>

  {#if tab === "export"}
    <div class="stack">
      <p class="warn">
        JSON export is unencrypted. Passwords and private keys will be readable in the file.
      </p>
      <p class="hr-small hr-muted">
        Selected servers: {catalog.selected.size}. Exporting an empty selection exports the whole catalog.
      </p>
      <div class="row">
        <Button variant="primary" onclick={() => void exportSelected()}>Export JSON</Button>
        <Button variant="secondary" onclick={() => void exportRdp()}>Export selected as .rdp</Button>
      </div>
    </div>
  {:else}
    <div class="stack">
      {#if tab !== "db"}
        <label class="field">
          <span>Choose file</span>
          <input type="file" accept={tab === "json" ? ".json" : tab === "csv" ? ".csv" : ".rdp"} onchange={readFile} />
        </label>
        <label class="field">
          <span>Content</span>
          <textarea rows="10" bind:value={text} placeholder="Paste file content or choose a file"></textarea>
        </label>
      {:else}
        <label class="field">
          <span>Database path</span>
          <input bind:value={dbPath} placeholder="C:\\path\\1Remote.db" />
        </label>
        <p class="hr-small hr-muted">
          Imports our schema and best-effort legacy rows. Encrypted legacy passwords cannot be recovered
          without the original key.
        </p>
      {/if}

      {#if tab === "rdp"}
        <div class="row">
          <label class="field"><span>Display name</span><input bind:value={rdpName} /></label>
          <label class="field"><span>Password (optional)</span><input type="password" bind:value={rdpPassword} /></label>
        </div>
      {/if}

      <label class="field">
        <span>Target data source</span>
        <select bind:value={target}>
          <option value="local">Local (SQLite) — writable</option>
          {#each catalog.configuredSources() as source (source.id)}
            <option value={source.id} disabled>
              {source.name} ({source.kind}) — read-only
            </option>
          {/each}
        </select>
      </label>
      <p class="hr-small hr-muted">
        Additional data sources are read-only; imports are written to the local catalog.
      </p>

      <div class="row">
        <Button variant="primary" disabled={busy} onclick={() => void runImport()}>
          {busy ? i18n.t("common.loading") : "Import"}
        </Button>
      </div>

      {#if outcome}
        <div class="outcome">
          <strong>{outcome.imported} server(s) imported</strong>
          {#each outcome.warnings as warning (warning)}
            <p class="warn">{warning}</p>
          {/each}
        </div>
      {/if}
    </div>
  {/if}

  {#snippet footer()}
    <Button variant="secondary" onclick={onclose}>{i18n.t("common.close")}</Button>
  {/snippet}
</Modal>

<style>
  .tabs {
    display: flex;
    gap: 4px;
    margin-bottom: 14px;
    flex-wrap: wrap;
  }

  .tabs button {
    border: 1px solid var(--hr-border);
    background: transparent;
    color: inherit;
    border-radius: var(--hr-radius);
    padding: 6px 10px;
    font-size: var(--hr-font-size-small);
  }

  .tabs button.active {
    border-color: var(--hr-accent-mid);
    color: var(--hr-accent-light);
    background: color-mix(in srgb, var(--hr-accent-mid) 12%, transparent);
  }

  .stack {
    display: flex;
    flex-direction: column;
    gap: 11px;
  }

  .row {
    display: flex;
    gap: 10px;
    align-items: flex-end;
    flex-wrap: wrap;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 5px;
    font-size: var(--hr-font-size-small);
    flex: 1;
  }

  input,
  select,
  textarea {
    border-radius: var(--hr-radius);
    border: 1px solid var(--hr-border);
    background: color-mix(in srgb, var(--hr-primary-dark) 50%, var(--hr-background));
    color: inherit;
    padding: 7px 9px;
    font: inherit;
    font-size: var(--hr-font-size-small);
  }

  textarea {
    resize: vertical;
    font-family: "Cascadia Code", Consolas, monospace;
  }

  .warn {
    color: var(--hr-warning);
    font-size: var(--hr-font-size-small);
    padding: 7px 9px;
    border-radius: var(--hr-radius-small);
    background: color-mix(in srgb, var(--hr-warning) 10%, transparent);
  }

  .outcome {
    border-top: 1px solid var(--hr-border);
    padding-top: 10px;
    font-size: var(--hr-font-size-small);
  }
</style>
