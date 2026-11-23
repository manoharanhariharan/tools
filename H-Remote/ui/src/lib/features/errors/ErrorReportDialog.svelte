<script lang="ts">
  import { commands } from "../../ipc/client";
  import { i18n } from "../../i18n/index.svelte";
  import { app } from "../../state/app.svelte";
  import Button from "../../components/Button.svelte";
  import Icon from "../../components/Icon.svelte";
  import Modal from "../../components/Modal.svelte";

  let saving = $state(false);

  async function copyReport() {
    const report = app.errorReport;
    if (!report) return;
    try {
      await navigator.clipboard.writeText(report.markdown || report.details);
      app.showNotice(i18n.t("error.copied"), "info");
    } catch (error) {
      await app.reportError(error);
    }
  }

  async function saveReport() {
    const report = app.errorReport;
    if (!report) return;
    saving = true;
    try {
      const path = await commands.saveErrorReport(report.markdown || report.details);
      app.showNotice(i18n.t("error.saved", { path }), "info");
    } catch (error) {
      await app.reportError(error);
    } finally {
      saving = false;
    }
  }

  function openLogs() {
    void commands.openLogFolder().catch((error) => app.reportError(error));
  }
</script>

{#if app.errorReport}
  <Modal
    open={true}
    title={i18n.t("error.title")}
    description={i18n.t("error.unexpected")}
    width="lg"
    onclose={() => app.dismissError()}
  >
    <div class="report">
      <div class="headline">
        <span class="badge"><Icon name="error" size={18} /></span>
        <p>{app.errorReport.message}</p>
      </div>

      {#if app.errorReport.path}
        <p class="hr-small hr-muted">
          {i18n.t("crash.saved", { path: app.errorReport.path })}
        </p>
      {/if}

      <details>
        <summary>{i18n.t("error.details")}</summary>
        <pre>{app.errorReport.details}</pre>
      </details>

      <p class="hr-small hr-muted">
        {i18n.t("error.environment")}: {app.bootstrap?.app.targetOs}/{app.bootstrap?.app.targetArch}
        · v{app.bootstrap?.app.version}
      </p>
    </div>

    {#snippet footer()}
      <Button variant="ghost" onclick={openLogs}>
        <Icon name="folder" size={15} />
        {i18n.t("error.openLogs")}
      </Button>
      <Button variant="secondary" onclick={copyReport}>
        <Icon name="info" size={15} />
        {i18n.t("error.copy")}
      </Button>
      <Button variant="secondary" disabled={saving} onclick={saveReport}>
        <Icon name="folder" size={15} />
        {i18n.t("error.save")}
      </Button>
      <Button variant="primary" onclick={() => app.dismissError()}>
        {i18n.t("error.dismiss")}
      </Button>
    {/snippet}
  </Modal>
{/if}

<style>
  .report {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .headline {
    display: flex;
    gap: 12px;
    align-items: flex-start;
  }

  .badge {
    width: 34px;
    height: 34px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: color-mix(in srgb, var(--hr-danger) 18%, transparent);
    color: var(--hr-danger);
    flex: none;
  }

  .headline p {
    line-height: 1.45;
    word-break: break-word;
    padding-top: 5px;
  }

  details {
    border: 1px solid var(--hr-border);
    border-radius: var(--hr-radius);
    padding: 10px 12px;
    background: color-mix(in srgb, var(--hr-primary-dark) 35%, transparent);
  }

  summary {
    cursor: pointer;
    font-size: var(--hr-font-size-small);
    color: color-mix(in srgb, var(--hr-background-text) 70%, transparent);
  }

  pre {
    margin: 10px 0 0;
    max-height: 38vh;
    overflow: auto;
    white-space: pre-wrap;
    word-break: break-word;
  }
</style>
