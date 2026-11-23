<script lang="ts">
  import { onMount } from "svelte";
  import type { Runner, RunnersConfig } from "../../ipc/contracts";
  import { commands } from "../../ipc/client";
  import { app } from "../../state/app.svelte";
  import Button from "../../components/Button.svelte";
  import Toggle from "../../components/Toggle.svelte";

  let runners = $state<Runner[]>([]);
  let defaults = $state<Record<string, string>>({});
  let selectedId = $state<string | null>(null);
  let draft = $state<Runner | null>(null);

  const protocols = ["SSH", "Telnet", "Serial"];

  onMount(() => {
    void load();
  });

  async function load(): Promise<void> {
    try {
      const config: RunnersConfig = await commands.getRunners();
      apply(config);
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  function apply(config: RunnersConfig): void {
    runners = config.runners;
    defaults = config.defaults;
    if (!selectedId && runners.length > 0) {
      select(runners[0]);
    } else if (selectedId) {
      const current = runners.find((runner) => runner.id === selectedId);
      if (current) {
        select(current);
      }
    }
  }

  function select(runner: Runner): void {
    selectedId = runner.id;
    draft = { ...runner, parameters: { ...runner.parameters }, environment: { ...runner.environment } };
  }

  function addRunner(): void {
    const runner: Runner = {
      id: `custom-${Date.now()}`,
      name: "New runner",
      protocol: "SSH",
      kind: "external",
      exePath: "",
      arguments: "-ssh {user}@{host} -P {port}",
      parameters: {},
      environment: {},
      runWithHosting: false,
      sessionConfigPath: null,
      installState: "missing",
      macros: [],
    };
    runners = [...runners, runner];
    select(runner);
  }

  async function save(): Promise<void> {
    if (!draft) {
      return;
    }
    try {
      const config = await commands.saveRunner(draft);
      apply(config);
      app.showNotice(`Runner '${draft.name}' saved.`, "info");
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async function remove(): Promise<void> {
    if (!draft) {
      return;
    }
    try {
      const config = await commands.deleteRunner(draft.id);
      selectedId = null;
      draft = null;
      apply(config);
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async function makeDefault(protocol: string, runnerId: string): Promise<void> {
    try {
      const config = await commands.setDefaultRunner(protocol, runnerId);
      apply(config);
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  function update<K extends keyof Runner>(field: K, value: Runner[K]): void {
    if (draft) {
      draft = { ...draft, [field]: value };
    }
  }
</script>

<div class="hr-stack">
  <section class="hr-card">
    <div class="hr-card-title"><h2>Runners</h2></div>
    <p class="hr-small hr-muted">
      Terminal protocols launch through an external runner. Set the executable path for PuTTY,
      KiTTY or any SSH client. Argument templates support {`{host}`}, {`{port}`}, {`{user}`},
      {`{password}`}, {`{privateKey}`}, {`{sessionConfig}`} and the %1RM_* macros.
    </p>

    <div class="runners">
      <div class="list">
        {#each runners as runner (runner.id)}
          <button
            type="button"
            class="runner-row"
            class:active={runner.id === selectedId}
            onclick={() => select(runner)}
          >
            <span class="protocol">{runner.protocol}</span>
            <span class="name">{runner.name}</span>
            {#if !runner.exePath && runner.kind === "external"}
              <span class="missing">no exe</span>
            {/if}
            {#if defaults[runner.protocol] === runner.id}
              <span class="default">default</span>
            {/if}
          </button>
        {/each}
      </div>

      {#if draft}
        <div class="editor">
          <label><span>Name</span><input bind:value={draft.name} /></label>
          <div class="row">
            <label>
              <span>Protocol</span>
              <select bind:value={draft.protocol}>
                {#each protocols as protocol (protocol)}
                  <option value={protocol}>{protocol}</option>
                {/each}
              </select>
            </label>
            <label>
              <span>Kind</span>
              <select bind:value={draft.kind}>
                <option value="external">External executable</option>
                <option value="externalSsh">External SSH client</option>
                <option value="internal">Internal (H-Remote)</option>
              </select>
            </label>
          </div>
          <label><span>Executable path</span><input bind:value={draft.exePath} placeholder="C:\Program Files\PuTTY\putty.exe" /></label>
          <label><span>Arguments</span><textarea rows="3" bind:value={draft.arguments}></textarea></label>
          <label><span>Session config path (optional)</span><input value={draft.sessionConfigPath ?? ""} oninput={(event) => update("sessionConfigPath", event.currentTarget.value || null)} /></label>
          <Toggle
            id="runner-hosting"
            label="Run with hosting when available"
            checked={draft.runWithHosting}
            onchange={(checked) => update("runWithHosting", checked)}
          />
          <div class="row">
            <Button variant="primary" onclick={() => void save()}>Save runner</Button>
            <Button variant="secondary" onclick={() => void makeDefault(draft!.protocol, draft!.id)}>Make default</Button>
            <Button variant="danger" onclick={() => void remove()}>Delete</Button>
          </div>
          <p class="hr-small hr-muted">
            Macros: {(draft.macros.length > 0 ? draft.macros : ["{host}", "{port}", "{user}", "{password}"]).join(" ")}
          </p>
        </div>
      {/if}
    </div>
    <div class="row">
      <Button variant="secondary" onclick={addRunner}>Add runner</Button>
    </div>
  </section>
</div>

<style>
  .runners {
    display: grid;
    grid-template-columns: 210px 1fr;
    gap: 12px;
    margin: 10px 0;
  }

  .list {
    display: flex;
    flex-direction: column;
    gap: 3px;
    max-height: 320px;
    overflow: auto;
  }

  .runner-row {
    display: flex;
    align-items: center;
    gap: 6px;
    border: 1px solid transparent;
    border-radius: 5px;
    background: transparent;
    color: inherit;
    padding: 5px 7px;
    text-align: left;
    font-size: 11px;
  }

  .runner-row:hover,
  .runner-row.active {
    background: color-mix(in srgb, var(--hr-accent-mid) 13%, transparent);
    border-color: color-mix(in srgb, var(--hr-accent-mid) 40%, transparent);
  }

  .protocol {
    font-size: 9px;
    color: var(--hr-accent-light);
    width: 38px;
  }

  .name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .missing,
  .default {
    font-size: 8px;
    border-radius: 999px;
    padding: 1px 5px;
    border: 1px solid var(--hr-border);
  }

  .missing {
    color: var(--hr-warning);
  }

  .default {
    color: var(--hr-accent-light);
  }

  .editor {
    display: flex;
    flex-direction: column;
    gap: 7px;
  }

  .row {
    display: flex;
    gap: 8px;
    align-items: flex-end;
    flex-wrap: wrap;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 10px;
    flex: 1;
  }

  input,
  select,
  textarea {
    border-radius: var(--hr-radius);
    border: 1px solid var(--hr-border);
    background: color-mix(in srgb, var(--hr-primary-dark) 55%, var(--hr-background));
    color: inherit;
    padding: 6px 8px;
    font: inherit;
    font-size: 11px;
    min-height: 30px;
  }
</style>
