<script lang="ts">
  import { app } from "../../state/app.svelte";
  import { commands } from "../../ipc/client";
  import Button from "../../components/Button.svelte";
  import Icon from "../../components/Icon.svelte";
  import Modal from "../../components/Modal.svelte";
  import { i18n } from "../../i18n/index.svelte";

  let reportContent = $state<string | null>(null);
  let reportName = $state("");

  async function openReport(fileName: string) {
    try {
      reportContent = await commands.readCrashReport(fileName);
      reportName = fileName;
    } catch (error) {
      await app.reportError(error);
    }
  }

  async function copyDiagnostics() {
    const diagnostics = {
      app: app.bootstrap?.app,
      paths: app.bootstrap?.paths,
      capabilities: app.bootstrap?.capabilities,
      database: app.bootstrap?.database,
      vault: app.bootstrap?.vault,
      startup: app.bootstrap?.startup,
    };
    try {
      await navigator.clipboard.writeText(JSON.stringify(diagnostics, null, 2));
      app.showNotice(i18n.t("common.copied"), "info");
    } catch (error) {
      await app.reportError(error);
    }
  }

  function formatTime(seconds: number | null): string {
    if (!seconds) return "—";
    return new Date(seconds * 1000).toLocaleString();
  }
</script>

{#if app.bootstrap}
  <div class="hr-page hr-fade-in">
    <header class="hr-page-header">
      <div>
        <h1>{i18n.t("common.about")} H-Remote</h1>
        <p>{i18n.t("about.description")}</p>
      </div>
      <Button variant="secondary" onclick={copyDiagnostics}>
        <Icon name="info" size={16} />
        Copy diagnostics
      </Button>
    </header>

    <section class="hr-card">
      <div class="hr-card-title"><h2>{i18n.t("about.version")}</h2></div>
      <dl class="facts">
        <dt>{i18n.t("common.version")}</dt>
        <dd>{app.bootstrap.app.version}</dd>
        <dt>{i18n.t("about.platform")}</dt>
        <dd>{app.bootstrap.app.targetOs} / {app.bootstrap.app.targetArch}</dd>
        <dt>{i18n.t("about.build")}</dt>
        <dd>{app.bootstrap.app.debug ? "debug" : "release"}</dd>
        <dt>{i18n.t("about.profile")}</dt>
        <dd>{app.bootstrap.paths.profileMode}</dd>
        <dt>{i18n.t("about.license")}</dt>
        <dd>MIT OR Apache-2.0</dd>
      </dl>
    </section>

    <section class="hr-card">
      <div class="hr-card-title">
        <h2>Diagnostics</h2>
        <Button
          size="sm"
          variant="ghost"
          onclick={() =>
            commands.openLogFolder().catch((error) => app.reportError(error))}
        >
          <Icon name="folder" size={15} />
          {i18n.t("settings.openLogFolder")}
        </Button>
      </div>
      <div class="checkpoints">
        {#each app.bootstrap.startup as checkpoint (checkpoint.name)}
          <div class="checkpoint">
            <span>{checkpoint.name}</span>
            <span class="hr-mono">{checkpoint.elapsedMs} ms</span>
          </div>
        {/each}
      </div>
      <div class="paths">
        <div><span>{i18n.t("about.configuration")}</span><code>{app.bootstrap.paths.configFile}</code></div>
        <div><span>{i18n.t("about.logs")}</span><code>{app.bootstrap.paths.logFile}</code></div>
        <div><span>{i18n.t("about.database")}</span><code>{app.bootstrap.paths.databaseFile}</code></div>
      </div>
    </section>

    <section class="hr-card">
      <div class="hr-card-title">
        <h2>{i18n.t("error.crashReports")}</h2>
        <Button
          size="sm"
          variant="ghost"
          onclick={() => {
            reportContent = null;
            reportName = "";
          }}
        >
          <Icon name="refresh" size={15} />
          {i18n.t("common.refresh")}
        </Button>
      </div>
      <p class="hr-muted hr-small">
        Reports are written to <code>{app.bootstrap.paths.logsDir}</code>. Review them before
        sharing.
      </p>
      <p class="hr-small hr-muted">{i18n.t("crash.noReports")}</p>
      <p class="hr-small">
        Open the log folder from Settings to inspect <code>hremote.log</code> and crash reports.
      </p>
    </section>

    <section class="hr-card">
      <div class="hr-card-title"><h2>Feedback</h2></div>
      <p class="hr-small hr-muted">
        Report issues with the diagnostics above. The feedback link is configurable by the
        distributor.
      </p>
      <Button
        variant="secondary"
        onclick={() => commands.openUrl(app.bootstrap!.feedbackUrl).catch((error) => app.reportError(error))}
      >
        <Icon name="external" size={15} />
        {i18n.t("tray.feedback")}
      </Button>
    </section>
  </div>

  <Modal
    open={reportContent !== null}
    title={reportName || i18n.t("crash.title")}
    width="lg"
    onclose={() => (reportContent = null)}
  >
    <pre class="report">{reportContent}</pre>
    {#snippet footer()}
      <Button variant="secondary" onclick={() => (reportContent = null)}>
        {i18n.t("common.close")}
      </Button>
    {/snippet}
  </Modal>
{/if}

<style>
  .facts {
    display: grid;
    grid-template-columns: minmax(120px, auto) 1fr;
    gap: 8px 18px;
    margin: 0;
  }

  .facts dt {
    color: color-mix(in srgb, var(--hr-background-text) 62%, transparent);
    font-size: var(--hr-font-size-small);
  }

  .facts dd {
    margin: 0;
  }

  .checkpoints {
    display: grid;
    gap: 6px;
    margin-bottom: 14px;
  }

  .checkpoint {
    display: flex;
    justify-content: space-between;
    gap: 18px;
    padding: 7px 10px;
    border-radius: var(--hr-radius-small);
    background: color-mix(in srgb, var(--hr-primary-dark) 40%, transparent);
  }

  .paths {
    display: grid;
    gap: 8px;
    font-size: var(--hr-font-size-small);
  }

  .paths div {
    display: grid;
    grid-template-columns: 120px 1fr;
    gap: 12px;
    align-items: baseline;
  }

  .paths span {
    color: color-mix(in srgb, var(--hr-background-text) 62%, transparent);
  }

  .paths code {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .report {
    margin: 0;
    padding: 12px;
    border-radius: var(--hr-radius);
    background: color-mix(in srgb, #000 30%, var(--hr-primary-dark));
    max-height: 50vh;
    overflow: auto;
    white-space: pre-wrap;
  }
</style>
