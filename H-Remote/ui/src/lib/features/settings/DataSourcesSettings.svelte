<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type {
    DataSourceConfig,
    DataSourceKind,
    SourceState,
    SourceStatus,
  } from "../../ipc/contracts";
  import { commands } from "../../ipc/client";
  import { CleanupRegistry, subscribe } from "../../ipc/events";
  import { app } from "../../state/app.svelte";
  import { catalog } from "../../state/catalog.svelte";
  import { dialogs } from "../../state/dialogs.svelte";
  import Button from "../../components/Button.svelte";
  import PasswordField from "../../components/PasswordField.svelte";

  let configs = $state<DataSourceConfig[]>([]);
  let statuses = $state<SourceStatus[]>([]);
  let editing = $state<DataSourceConfig | null>(null);
  let password = $state("");
  let error = $state<string | null>(null);
  let busy = $state(false);
  let localPath = $state("");
  let localInfo = $state<{ path: string; writable: boolean | null } | null>(null);

  const cleanup = new CleanupRegistry();
  let pollTimer: ReturnType<typeof setInterval> | undefined;

  const localStatus = $derived(statuses.find((status) => status.id === "local"));

  const stateLabel: Record<SourceState, string> = {
    ok: "OK",
    notConnectedYet: "Not connected yet",
    lostConnection: "Lost connection",
    accessDenied: "Access denied",
    encryptKeyError: "Encryption key error",
    otherError: "Error",
  };

  onMount(() => {
    void load();
    // The backend watcher emits on change; polling also refreshes counts when the
    // page is open so the list never goes stale (P8-06/P8-07).
    pollTimer = setInterval(() => void refreshStatuses(), 5000);
    subscribe<SourceStatus[]>("source:statuses", () => void refreshStatuses(), cleanup);
  });

  onDestroy(() => {
    cleanup.dispose();
    if (pollTimer) {
      clearInterval(pollTimer);
      pollTimer = undefined;
    }
  });

  async function load(): Promise<void> {
    try {
      const [list, statusList, info] = await Promise.all([
        commands.dataSourceList(),
        commands.dataSourceStatuses(),
        commands.databaseStatus(),
      ]);
      configs = list;
      statuses = statusList;
      localInfo = { path: info.path, writable: info.writable };
      localPath = info.path;
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async function refreshStatuses(): Promise<void> {
    try {
      statuses = await commands.dataSourceStatuses();
    } catch {
      // A transient IPC failure must not spam the user; the next poll retries.
    }
  }

  function statusFor(id: string): SourceStatus | undefined {
    return statuses.find((status) => status.id === id);
  }

  function add(): void {
    editing = {
      id: "",
      name: "",
      kind: "sqlite",
      path: null,
      host: "localhost",
      port: 0,
      database: "",
      username: "",
      password: "",
      readOnly: true,
      enabled: true,
    };
    password = "";
    error = null;
  }

  function edit(config: DataSourceConfig): void {
    editing = { ...config };
    password = "";
    error = null;
  }

  function defaultPort(kind: DataSourceKind): number {
    return kind === "mysql" ? 3306 : kind === "postgres" ? 5432 : 0;
  }

  async function test(): Promise<void> {
    if (!editing) {
      return;
    }
    busy = true;
    error = null;
    try {
      const status = await commands.dataSourceTest(editing, password || null);
      app.showNotice(
        `${status.name}: ${stateLabel[status.state]} (${status.serverCount} servers)`,
        "info",
      );
    } catch (reason) {
      error = reason instanceof Error ? reason.message : String(reason);
    } finally {
      busy = false;
    }
  }

  async function save(): Promise<void> {
    if (!editing) {
      return;
    }
    busy = true;
    error = null;
    try {
      const saved = await commands.dataSourceSave(editing, password || null);
      editing = null;
      password = "";
      try {
        await commands.dataSourceRefresh(saved.id);
      } catch (reason) {
        app.showNotice(
          `Saved '${saved.name}' but it did not connect: ${
            reason instanceof Error ? reason.message : String(reason)
          }`,
          "warning",
        );
      }
      await load();
      await catalog.reloadAfterSourceChange();
      app.showNotice(`Data source '${saved.name}' saved.`, "info");
    } catch (reason) {
      error = reason instanceof Error ? reason.message : String(reason);
    } finally {
      busy = false;
    }
  }

  async function remove(config: DataSourceConfig): Promise<void> {
    const confirmed = await dialogs.confirm({
      title: `Remove data source '${config.name}'?`,
      message:
        "Only the connection is removed. The remote database and its data stay untouched.",
      danger: true,
    });
    if (!confirmed) {
      return;
    }
    try {
      await commands.dataSourceDelete(config.id);
      if (editing?.id === config.id) {
        editing = null;
      }
      await load();
      await catalog.reloadAfterSourceChange();
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async function refresh(id: string): Promise<void> {
    busy = true;
    try {
      const status = await commands.dataSourceRefresh(id);
      statuses = statuses.map((candidate) =>
        candidate.id === status.id ? status : candidate,
      );
      await catalog.refresh();
    } catch (reason) {
      await app.reportError(reason);
    } finally {
      busy = false;
    }
  }

  async function changeLocalPath(): Promise<void> {
    busy = true;
    error = null;
    try {
      const info = await commands.changeLocalDatabasePath(localPath.trim() || null);
      localInfo = { path: info.path, writable: info.writable };
      localPath = info.path;
      await catalog.reloadAfterSourceChange();
      await refreshStatuses();
      app.showNotice(`Local catalog is now ${info.path}`, "info");
    } catch (reason) {
      error = reason instanceof Error ? reason.message : String(reason);
    } finally {
      busy = false;
    }
  }

  function resetLocalPath(): void {
    localPath = localInfo?.path ?? "";
  }
</script>

<div class="hr-stack">
  <section class="hr-card">
    <div class="hr-card-title">
      <h2>Local catalog</h2>
      <span class="state" class:ok={localStatus?.state === "ok"}>
        {localStatus ? stateLabel[localStatus.state] : "Checking…"}
      </span>
    </div>
    <p class="hr-small hr-muted">
      The writable SQLite source. Changing the path opens the new file, verifies the encryption
      key and reloads the catalog; a bad path leaves the current database untouched.
    </p>
    <div class="path-row">
      <label class="field grow">
        <span>Database file</span>
        <input bind:value={localPath} placeholder="Leave empty for the profile default" />
      </label>
      <Button size="sm" variant="secondary" disabled={busy} onclick={resetLocalPath}>Reset</Button>
      <Button size="sm" variant="primary" disabled={busy} onclick={() => void changeLocalPath()}>
        Change path
      </Button>
    </div>
    {#if localStatus}
      <p class="hr-small hr-muted">
        {localStatus.serverCount} servers · {localStatus.credentialCount} credentials
        {#if localInfo?.writable === false}· not writable{/if}
      </p>
    {/if}
  </section>

  <section class="hr-card">
    <div class="hr-card-title">
      <h2>Additional sources</h2>
      <Button size="sm" variant="primary" onclick={add}>Add source</Button>
    </div>
    <p class="hr-small hr-muted">
      MySQL, PostgreSQL and other SQLite catalogs are mounted read-only. They are used for search,
      launch and import only; writes always target the local catalog.
    </p>

    {#if configs.length === 0}
      <p class="hr-small hr-muted">No additional sources configured.</p>
    {:else}
      <div class="list">
        {#each configs as config (config.id)}
          {@const status = statusFor(config.id)}
          <div class="row">
            <span class="dot" class:ok={status?.state === "ok"} class:warn={status && status.state !== "ok"}></span>
            <div class="info">
              <span class="name">{config.name}</span>
              <span class="target">
                {config.kind.toUpperCase()}
                {#if config.kind === "sqlite"}
                  {config.path ?? "(no path)"}
                {:else}
                  {config.host}:{config.port || defaultPort(config.kind)}/{config.database}
                {/if}
              </span>
            </div>
            <span class="badge" class:offline={status && status.state !== "ok"}>
              {status ? stateLabel[status.state] : "Not connected yet"}
            </span>
            <span class="counts">
              {status?.serverCount ?? 0} / {status?.credentialCount ?? 0}
            </span>
            <Button size="sm" variant="ghost" disabled={busy} onclick={() => void refresh(config.id)}>
              Refresh
            </Button>
            <Button size="sm" variant="secondary" onclick={() => edit(config)}>Edit</Button>
            <Button size="sm" variant="danger" onclick={() => void remove(config)}>Remove</Button>
          </div>
          {#if status?.error}
            <p class="source-error">{status.error}</p>
          {/if}
        {/each}
      </div>
    {/if}
  </section>

  {#if editing}
    <section class="hr-card">
      <div class="hr-card-title">
        <h2>{configs.some((config) => config.id === editing!.id) ? "Edit source" : "New source"}</h2>
      </div>
      {#if error}
        <p class="error" role="alert">{error}</p>
      {/if}
      <div class="form">
        <div class="row-inline">
          <label class="field grow">
            <span>Name</span>
            <input bind:value={editing.name} placeholder="Team catalog" />
          </label>
          <label class="field">
            <span>Type</span>
            <select
              value={editing.kind}
              onchange={(event) => {
                if (!editing) return;
                editing.kind = event.currentTarget.value as DataSourceKind;
                editing.port = defaultPort(editing.kind);
              }}
            >
              <option value="sqlite">SQLite</option>
              <option value="mysql">MySQL</option>
              <option value="postgres">PostgreSQL</option>
            </select>
          </label>
        </div>

        {#if editing.kind === "sqlite"}
          <label class="field">
            <span>Database file</span>
            <input
              value={editing.path ?? ""}
              placeholder="C:\\shared\\hremote.db"
              oninput={(event) => {
                if (editing) editing.path = event.currentTarget.value || null;
              }}
            />
          </label>
        {:else}
          <div class="row-inline">
            <label class="field grow">
              <span>Host</span>
              <input bind:value={editing.host} />
            </label>
            <label class="field port">
              <span>Port</span>
              <input type="number" bind:value={editing.port} />
            </label>
            <label class="field grow">
              <span>Database</span>
              <input bind:value={editing.database} />
            </label>
          </div>
          <div class="row-inline">
            <label class="field grow">
              <span>Username</span>
              <input bind:value={editing.username} />
            </label>
            <PasswordField
              id="source-password"
              label="Password"
              value={password}
              hint="Stored encrypted in hremote.dataSources.json; leaving it empty keeps the saved password."
              oninput={(value) => (password = value)}
            />
          </div>
        {/if}

        <p class="hr-small hr-muted">
          Additional sources are always read-only: servers can be searched, launched and imported
          from them, but never edited or deleted here.
        </p>
      </div>
      <div class="hr-row">
        <Button variant="secondary" disabled={busy} onclick={() => void test()}>Test connection</Button>
        <Button variant="primary" disabled={busy} onclick={() => void save()}>Save</Button>
        <Button variant="ghost" onclick={() => (editing = null)}>Cancel</Button>
      </div>
    </section>
  {/if}
</div>

<style>
  .path-row {
    display: flex;
    align-items: flex-end;
    gap: 8px;
    margin: 8px 0;
  }

  .list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin-top: 8px;
  }

  .row {
    display: grid;
    grid-template-columns: 10px 1fr auto auto auto auto auto;
    gap: 8px;
    align-items: center;
    padding: 5px 6px;
    border-radius: 5px;
  }

  .row:hover {
    background: color-mix(in srgb, var(--hr-accent-mid) 9%, transparent);
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--hr-danger);
  }

  .dot.ok {
    background: #49b675;
  }

  .dot.warn {
    background: var(--hr-warning);
  }

  .info {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .name {
    font-size: 12px;
    font-weight: 600;
  }

  .target {
    font-size: 10px;
    color: color-mix(in srgb, var(--hr-background-text) 55%, transparent);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .badge {
    font-size: 9px;
    border: 1px solid color-mix(in srgb, #49b675 45%, transparent);
    color: #49b675;
    border-radius: 999px;
    padding: 1px 7px;
    white-space: nowrap;
  }

  .badge.offline {
    border-color: color-mix(in srgb, var(--hr-warning) 45%, transparent);
    color: var(--hr-warning);
  }

  .counts {
    font-size: 10px;
    font-variant-numeric: tabular-nums;
    color: color-mix(in srgb, var(--hr-background-text) 55%, transparent);
    min-width: 46px;
    text-align: right;
  }

  .state {
    font-size: 10px;
    color: var(--hr-warning);
  }

  .state.ok {
    color: #49b675;
  }

  .source-error {
    font-size: 10px;
    color: var(--hr-danger);
    margin: 0 0 6px 32px;
  }

  .form {
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-width: 640px;
  }

  .row-inline {
    display: flex;
    gap: 8px;
    align-items: flex-end;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 10px;
    flex: 1;
    min-width: 0;
  }

  .field.grow {
    flex: 2;
  }

  .field.port {
    max-width: 90px;
  }

  input,
  select {
    min-height: 31px;
    border-radius: var(--hr-radius);
    border: 1px solid var(--hr-border);
    background: color-mix(in srgb, var(--hr-primary-dark) 55%, var(--hr-background));
    color: inherit;
    padding: 6px 8px;
    font: inherit;
    font-size: 11px;
  }

  .error {
    color: var(--hr-danger);
    font-size: 11px;
    padding: 6px 8px;
    border-radius: 5px;
    background: color-mix(in srgb, var(--hr-danger) 10%, transparent);
  }
</style>
