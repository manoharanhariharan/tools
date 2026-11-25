<script lang="ts">
  import { onMount } from "svelte";
  import type { BootstrapDto, DatabaseInfo, I18nReport } from "../../ipc/contracts";
  import { commands } from "../../ipc/client";
  import { i18n } from "../../i18n/index.svelte";
  import { app } from "../../state/app.svelte";
  import { catalog } from "../../state/catalog.svelte";
  import { dialogs } from "../../state/dialogs.svelte";
  import Button from "../../components/Button.svelte";
  import Icon from "../../components/Icon.svelte";

  interface Props {
    bootstrap: BootstrapDto;
  }

  let { bootstrap }: Props = $props();

  // svelte-ignore state_referenced_locally
  let database = $state<DatabaseInfo>(bootstrap.database);
  let refreshing = $state(false);
  let report = $state<I18nReport | null>(null);

  onMount(() => {
    void refreshI18n();
  });

  async function refreshI18n() {
    try {
      report = await commands.i18nReport();
    } catch {
      // Diagnostics are best-effort; the rest of the page still works.
    }
  }

  async function refresh() {
    refreshing = true;
    try {
      database = await commands.databaseStatus();
    } catch (error) {
      await app.reportError(error);
    } finally {
      refreshing = false;
    }
  }

  async function repairSecrets() {
    const confirmed = await dialogs.confirm({
      title: "Repair database secrets?",
      message:
        "Every readable secret is re-encrypted with the current key. Values encrypted with a different installation cannot be recovered and will be cleared; affected servers and credentials will need their passwords entered again.",
      danger: true,
    });
    if (!confirmed) {
      return;
    }
    refreshing = true;
    try {
      const report = await commands.repairCatalogSecrets();
      await refresh();
      await catalog.load();
      app.showNotice(
        report.secretsCleared > 0
          ? `Repaired ${report.servers} servers and ${report.credentials} credentials; cleared ${report.secretsCleared} unreadable secret(s).`
          : `Re-encrypted ${report.servers} servers and ${report.credentials} credentials.`,
        "info",
      );
    } catch (error) {
      await app.reportError(error);
    } finally {
      refreshing = false;
    }
  }

  function openFolder(kind: "config" | "logs") {
    const action = kind === "config" ? commands.openConfigFolder() : commands.openLogFolder();
    void action.catch((error) => app.reportError(error));
  }

  const capabilityRows = $derived([
    ["Embedded RDP (IronRDP)", bootstrap.capabilities.embeddedRdp],
    ["Native mstsc fallback", bootstrap.capabilities.nativeRdpFallback],
    ["OS keyring", bootstrap.capabilities.osKeyring],
    ["Windows Hello", bootstrap.capabilities.windowsHello],
    ["Registry autostart", bootstrap.capabilities.registryAutostart],
    ["Native window embedding", bootstrap.capabilities.hwndEmbedding],
    ["Working-set trimming", bootstrap.capabilities.workingSetTrim],
    ["Global hotkeys", bootstrap.capabilities.globalHotkey],
  ] as Array<[string, boolean]>);
</script>

<div class="hr-stack">
  <section class="hr-card">
    <div class="hr-card-title">
      <h2>{i18n.t("data.localDatabase")}</h2>
      <Button size="sm" variant="ghost" disabled={refreshing} onclick={refresh}>
        <Icon name="refresh" size={15} />
        {i18n.t("common.refresh")}
      </Button>
    </div>
    <div class="status-row">
      <span class="hr-status-dot" class:error={!database.available} class:warning={database.available && database.writable === false}></span>
      <strong>
        {database.available ? i18n.t("common.enabled") : i18n.t("common.error")}
        {#if database.writable === false}· {i18n.t("common.readOnly")}{/if}
      </strong>
      {#if database.schemaVersion !== null}
        <span class="hr-pill">schema v{database.schemaVersion}</span>
      {/if}
    </div>
    <p class="hr-mono">{database.path}</p>
    {#if database.error}<p class="error">{database.error}</p>{/if}
    <div class="db-facts">
      <span>{i18n.t_args("data.schema", { version: database.schemaVersion ?? "?" })}</span>
      <span>
        {database.encryptionVerified
          ? i18n.t("data.encryptionVerified")
          : i18n.t("data.encryptionNotVerified")}
      </span>
      <span>{i18n.t_args("data.serverCount", { count: database.serverCount ?? 0 })}</span>
      <span>{i18n.t_args("data.credentialCount", { count: database.credentialCount ?? 0 })}</span>
    </div>
    <div class="hr-row actions">
      <Button variant="secondary" onclick={() => openFolder("config")}>
        <Icon name="folder" size={15} />
        {i18n.t("about.configuration")}
      </Button>
      <Button variant="secondary" onclick={() => openFolder("logs")}>
        <Icon name="folder" size={15} />
        {i18n.t("about.logs")}
      </Button>
      <Button variant="secondary" disabled={refreshing} onclick={() => void repairSecrets()}>
        <Icon name="refresh" size={15} />
        Repair secrets
      </Button>
    </div>
    <p class="hr-small hr-muted">
      If the keyring key changed or a database was copied from another profile, repair
      re-encrypts readable values and clears the ones that cannot be decrypted.
    </p>
  </section>

  <section class="hr-card">
    <div class="hr-card-title"><h2>{i18n.t("profile.portable")} / {i18n.t("profile.appdata")}</h2></div>
    <dl class="facts">
      <dt>Mode</dt>
      <dd>{bootstrap.paths.profileMode}</dd>
      <dt>{i18n.t("about.configuration")}</dt>
      <dd class="hr-mono">{bootstrap.paths.configFile}</dd>
      <dt>Locality</dt>
      <dd class="hr-mono">{bootstrap.paths.localityDir}</dd>
      <dt>{i18n.t("about.logs")}</dt>
      <dd class="hr-mono">{bootstrap.paths.logFile}</dd>
      <dt>{i18n.t("about.database")}</dt>
      <dd class="hr-mono">{bootstrap.paths.databaseFile}</dd>
    </dl>
    {#if bootstrap.markerLocked}
      <p class="hr-small hr-muted">{i18n.t("profile.lockedByMarker")}</p>
    {/if}
  </section>

  <section class="hr-card">
    <div class="hr-card-title">
      <h2>Encrypted storage</h2>
      <span class="hr-status-dot" class:error={!bootstrap.vault.ready} class:warning={!bootstrap.vault.ready}></span>
    </div>
    <p class="hr-small">
      {#if bootstrap.vault.ready}
        Ready — key store: <strong>{bootstrap.vault.keyStore}</strong>
      {:else}
        Unavailable — <strong>{bootstrap.vault.message}</strong>
      {/if}
    </p>
    <p class="hr-mono">{bootstrap.vault.saltPath}</p>
  </section>

  <section class="hr-card">
    <div class="hr-card-title">
      <h2>{i18n.t("data.protocols")}</h2>
      <span class="hr-pill">{bootstrap.protocols.length}</span>
    </div>
    {#if bootstrap.protocols.length > 0}
      <div class="protocol-grid">
        {#each bootstrap.protocols as protocol (protocol.id)}
          <div class="protocol">
            <strong>{protocol.displayName}</strong>
            <span class="hr-mono">{protocol.id}</span>
            <span class="hr-small hr-muted">{protocol.capabilities.sessionKind}</span>
          </div>
        {/each}
      </div>
    {:else}
      <p class="hr-small hr-muted">{i18n.t("data.noProtocols")}</p>
    {/if}
  </section>

  <section class="hr-card">
    <div class="hr-card-title"><h2>Platform capabilities</h2></div>
    <ul class="capabilities">
      {#each capabilityRows as [label, available] (label)}
        <li>
          <span>{label}</span>
          <span class={available ? "ok" : "off"}>
            {available ? i18n.t("capability.available") : i18n.t("capability.unavailable")}
          </span>
        </li>
      {/each}
    </ul>
    <p class="hr-small hr-muted">
      Unavailable APIs degrade gracefully: Windows Hello falls back to the native credential
      prompt, DPAPI-protected exports omit passwords off Windows, unavailable verification asks
      for explicit confirmation, and Store startup uses the registry Run key.
    </p>
  </section>

  <section class="hr-card">
    <div class="hr-card-title"><h2>{i18n.t("data.sources")}</h2></div>
    <p class="hr-small hr-muted">{i18n.t("data.sourcesHelp")}</p>
    {#if bootstrap.auxiliary.dataSourcesPresent}
      <p class="hr-small">Data-source file discovered and validated at startup.</p>
    {:else}
      <p class="hr-small">{i18n.t("data.noSources")}</p>
    {/if}
    {#if bootstrap.auxiliary.protocolConfigs.length > 0}
      <p class="hr-small hr-muted">
        Runner configurations loaded:
        {#each bootstrap.auxiliary.protocolConfigs as config (config.protocol)}
          <span class="hr-pill">{config.protocol}</span>
        {/each}
      </p>
    {/if}
    {#each bootstrap.auxiliary.warnings as warning (warning)}
      <p class="error">{warning}</p>
    {/each}
  </section>

  <section class="hr-card">
    <div class="hr-card-title">
      <h2>Localisation</h2>
      <Button size="sm" variant="ghost" onclick={() => void refreshI18n()}>Recheck</Button>
    </div>
    {#if report}
      <p class="hr-small">
        Active locale: <strong>{report.locale}</strong>
      </p>
      <div class="locale-grid">
        {#each report.locales as locale (locale.id)}
          <span class="hr-pill" class:incomplete={!locale.complete} title={locale.englishName}>
            {locale.nativeName}{locale.complete ? "" : " · partial"}
          </span>
        {/each}
      </div>
      {#if report.missing.length > 0}
        <p class="hr-small hr-muted">
          Missing keys fall back to English. Reported at runtime:
        </p>
        <ul class="missing">
          {#each report.missing.slice(0, 25) as key (key)}
            <li class="hr-mono">{key}</li>
          {/each}
        </ul>
        {#if report.missing.length > 25}
          <p class="hr-small hr-muted">…and {report.missing.length - 25} more.</p>
        {/if}
      {:else}
        <p class="hr-small hr-muted">No missing translation keys have been requested.</p>
      {/if}
    {:else}
      <p class="hr-small hr-muted">Localisation diagnostics are unavailable.</p>
    {/if}
  </section>

  <section class="hr-card">
    <div class="hr-card-title"><h2>Startup trace</h2></div>
    <div class="checkpoints">
      {#each bootstrap.startup as checkpoint (checkpoint.name)}
        <div>
          <span>{checkpoint.name}</span>
          <code>{checkpoint.elapsedMs} ms</code>
        </div>
      {/each}
    </div>
  </section>
</div>

<style>
  .status-row {
    display: flex;
    align-items: center;
    gap: 9px;
    margin-bottom: 8px;
  }

  .actions {
    margin-top: 14px;
  }

  .facts {
    display: grid;
    grid-template-columns: 130px 1fr;
    gap: 8px 16px;
    margin: 0;
    font-size: var(--hr-font-size-small);
  }

  .facts dt {
    color: color-mix(in srgb, var(--hr-background-text) 60%, transparent);
  }

  .facts dd {
    margin: 0;
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .error {
    color: var(--hr-danger);
    font-size: var(--hr-font-size-small);
  }

  .db-facts {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 10px;
    font-size: var(--hr-font-size-small);
  }

  .db-facts span {
    border: 1px solid var(--hr-border);
    border-radius: 999px;
    padding: 2px 9px;
    color: color-mix(in srgb, var(--hr-background-text) 70%, transparent);
  }

  .protocol-grid {
    display: grid;
    gap: 8px;
    grid-template-columns: repeat(auto-fill, minmax(170px, 1fr));
  }

  .protocol {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 9px 11px;
    border-radius: var(--hr-radius);
    border: 1px solid var(--hr-border);
    background: color-mix(in srgb, var(--hr-primary-dark) 30%, transparent);
    font-size: var(--hr-font-size-small);
  }

  .capabilities {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 7px;
    font-size: var(--hr-font-size-small);
  }

  .locale-grid {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    margin: 6px 0;
  }

  .hr-pill.incomplete {
    color: var(--hr-warning);
    border-color: color-mix(in srgb, var(--hr-warning) 40%, transparent);
  }

  .missing {
    margin: 4px 0 0;
    padding-left: 18px;
    font-size: 10px;
    color: color-mix(in srgb, var(--hr-background-text) 62%, transparent);
  }

  .capabilities li {
    display: flex;
    justify-content: space-between;
    gap: 18px;
    padding-bottom: 7px;
    border-bottom: 1px solid var(--hr-border);
  }

  .ok {
    color: var(--hr-success);
  }

  .off {
    color: color-mix(in srgb, var(--hr-background-text) 50%, transparent);
  }

  .checkpoints {
    display: grid;
    gap: 6px;
    font-size: var(--hr-font-size-small);
  }

  .checkpoints div {
    display: flex;
    justify-content: space-between;
    gap: 18px;
    padding: 6px 9px;
    border-radius: var(--hr-radius-small);
    background: color-mix(in srgb, var(--hr-primary-dark) 35%, transparent);
  }
</style>
