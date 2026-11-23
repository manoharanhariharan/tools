<script lang="ts">
  import { onMount } from "svelte";
  import type { Credential } from "../../ipc/contracts";
  import { commands } from "../../ipc/client";
  import { app } from "../../state/app.svelte";
  import { catalog } from "../../state/catalog.svelte";
  import { dialogs } from "../../state/dialogs.svelte";
  import Button from "../../components/Button.svelte";
  import PasswordField from "../../components/PasswordField.svelte";

  let credentials = $state<Credential[]>([]);
  let editing = $state<Credential | null>(null);
  let error = $state<string | null>(null);

  const grouped = $derived.by(() => {
    const map = new Map<string, Credential[]>();
    for (const credential of credentials) {
      const source = credential.databaseId ?? "local";
      map.set(source, [...(map.get(source) ?? []), credential]);
    }
    return [...map.entries()];
  });

  function sourceLabel(sourceId: string): string {
    return catalog.sourceLabel(sourceId);
  }

  onMount(() => {
    void load();
  });

  async function load(): Promise<void> {
    try {
      const [list] = await Promise.all([
        commands.listCredentials(),
        catalog.refreshSources(),
      ]);
      credentials = list;
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  function add(): void {
    editing = {
      id: `new-${Date.now()}`,
      name: "",
      address: null,
      port: null,
      username: "",
      password: "",
      privateKeyPath: null,
      databaseId: null,
      hash: "",
      isEditable: true,
    };
    error = null;
  }

  function edit(credential: Credential): void {
    editing = { ...credential };
    error = null;
  }

  async function save(): Promise<void> {
    if (!editing) {
      return;
    }
    error = null;
    try {
      const saved = await commands.saveCredential({
        ...editing,
        address: editing.address?.trim() || null,
        port: editing.port || null,
        privateKeyPath: editing.privateKeyPath?.trim() || null,
      });
      editing = null;
      await load();
      app.showNotice(`Credential '${saved.name}' saved.`, "info");
    } catch (reason) {
      error = reason instanceof Error ? reason.message : String(reason);
    }
  }

  async function remove(credential: Credential): Promise<void> {
    const confirmed = await dialogs.confirm({
      title: `Delete credential '${credential.name}'?`,
      message:
        "Servers inheriting it are detached and keep their own overrides. This cannot be undone.",
      danger: true,
    });
    if (!confirmed) {
      return;
    }
    try {
      await commands.deleteCredential(credential.id);
      if (editing?.id === credential.id) {
        editing = null;
      }
      await load();
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  function update<K extends keyof Credential>(field: K, value: Credential[K]): void {
    if (editing) {
      editing = { ...editing, [field]: value };
    }
  }
</script>

<div class="hr-stack">
  <section class="hr-card">
    <div class="hr-card-title">
      <h2>Credential vault</h2>
      <div class="hr-row">
        <Button size="sm" variant="secondary" onclick={() => void load()}>Refresh</Button>
        <Button size="sm" variant="primary" onclick={add}>Add credential</Button>
      </div>
    </div>
    <p class="hr-small hr-muted">
      Credentials are grouped by data source. Passwords are encrypted at rest and never shown in
      server lists. Copying or revealing a password requires Windows account verification.
    </p>

    {#each grouped as [source, items] (source)}
      <h3 class="source">{sourceLabel(source)}</h3>
      <div class="list">
        {#each items as credential (credential.id)}
          <div class="row" class:selected={editing?.id === credential.id}>
            <button type="button" class="main" onclick={() => edit(credential)}>
              <span class="name">{credential.name}</span>
              <span class="user">{credential.username || "no username"}</span>
              {#if credential.privateKeyPath}<span class="key">private key</span>{/if}
              {#if !credential.isEditable}<span class="ro">read-only</span>{/if}
            </button>
            <Button size="sm" variant="ghost" onclick={() => edit(credential)}>Edit</Button>
            <Button size="sm" variant="danger" disabled={!credential.isEditable} onclick={() => void remove(credential)}>Delete</Button>
          </div>
        {/each}
      </div>
    {:else}
      <p class="hr-small hr-muted">No credentials yet.</p>
    {/each}
  </section>

  {#if editing}
    <section class="hr-card">
      <div class="hr-card-title"><h2>{credentials.some((credential) => credential.id === editing!.id) ? "Edit credential" : "New credential"}</h2></div>
      {#if error}
        <p class="error" role="alert">{error}</p>
      {/if}
      {#if !editing.isEditable}
        <p class="hr-small" role="status">
          This credential belongs to a read-only data source and cannot be changed here.
        </p>
      {/if}
      <div class="form">
        <label><span>Name</span><input bind:value={editing.name} placeholder="prod-admin" /></label>
        <label><span>Username</span><input bind:value={editing.username} /></label>
        <PasswordField
          id="credential-password"
          label="Password"
          value={editing.password}
          hint="Setting a password clears the private key."
          oninput={(value) => {
            update("password", value);
            if (value) {
              update("privateKeyPath", null);
            }
          }}
        />
        <label>
          <span>Private key path</span>
          <input
            value={editing.privateKeyPath ?? ""}
            placeholder="C:\Users\me\.ssh\id_ed25519"
            oninput={(event) => {
              update("privateKeyPath", event.currentTarget.value || null);
              if (event.currentTarget.value) {
                update("password", "");
              }
            }}
          />
        </label>
        <div class="row">
          <label><span>Address override</span><input bind:value={editing.address} placeholder="optional" /></label>
          <label class="small"><span>Port override</span><input type="number" bind:value={editing.port} placeholder="optional" /></label>
        </div>
        <label>
          <span>Source</span>
          <select value={editing.databaseId ?? "local"} disabled>
            <option value="local">Local catalog</option>
          </select>
        </label>
      </div>
      <div class="hr-row">
        <Button variant="primary" disabled={!editing.isEditable} onclick={() => void save()}>Save</Button>
        <Button variant="secondary" onclick={() => (editing = null)}>Cancel</Button>
      </div>
    </section>
  {/if}
</div>

<style>
  .list {
    display: flex;
    flex-direction: column;
    gap: 3px;
    margin: 6px 0 12px;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 5px;
    border: 1px solid transparent;
    border-radius: 5px;
    padding: 3px 5px;
  }

  .row.selected,
  .row:hover {
    background: color-mix(in srgb, var(--hr-accent-mid) 10%, transparent);
    border-color: color-mix(in srgb, var(--hr-accent-mid) 30%, transparent);
  }

  .main {
    display: flex;
    align-items: baseline;
    gap: 8px;
    flex: 1;
    border: 0;
    background: transparent;
    color: inherit;
    text-align: left;
    padding: 5px;
  }

  .name {
    font-weight: 600;
    font-size: 12px;
  }

  .user {
    font-size: 10px;
    color: color-mix(in srgb, var(--hr-background-text) 55%, transparent);
  }

  .key,
  .ro {
    font-size: 8px;
    border: 1px solid var(--hr-border);
    border-radius: 999px;
    padding: 1px 6px;
    color: var(--hr-accent-light);
  }

  .ro {
    color: var(--hr-warning);
  }

  .source {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: color-mix(in srgb, var(--hr-background-text) 50%, transparent);
    margin-top: 8px;
  }

  .form {
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-width: 560px;
  }

  .form .row {
    display: flex;
    gap: 8px;
    border: 0;
    padding: 0;
    background: transparent;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 10px;
    flex: 1;
  }

  label.small {
    max-width: 120px;
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
