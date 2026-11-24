<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type {
    ConnectionRecords,
    Server,
    TextRange,
  } from "../../ipc/contracts";
  import { commands } from "../../ipc/client";
  import { CleanupRegistry, subscribe } from "../../ipc/events";
  import { i18n } from "../../i18n/index.svelte";
  import { app } from "../../state/app.svelte";
  import { catalog } from "../../state/catalog.svelte";
  import { dialogs } from "../../state/dialogs.svelte";
  import { sessions } from "../../state/sessions.svelte";
  import Button from "../../components/Button.svelte";
  import HighlightedText from "../../components/HighlightedText.svelte";
  import ServerIcon from "../../components/ServerIcon.svelte";

  let query = $state("");
  let selectedIndex = $state(0);
  let mode = $state<"search" | "quick">("search");
  let servers = $state<Server[]>([]);
  let records = $state<ConnectionRecords>({
    schemaVersion: 1,
    records: {},
    quickConnect: [],
  });
  let searchInput = $state<HTMLInputElement | null>(null);
  let resizeTimer: ReturnType<typeof setTimeout> | undefined;
  let cleanup = new CleanupRegistry();

  let quickProtocol = $state("RDP");
  let quickAddress = $state("");
  let quickPort = $state<number | undefined>(3389);
  let quickUsername = $state("");
  let quickPassword = $state("");
  let quickSave = $state(false);
  let quickMatch = $state(true);
  let quickBusy = $state(false);
  let quickError = $state<string | null>(null);

  const protocols = $derived(
    (catalog.snapshot?.protocols ?? []).filter(
      (protocol) =>
        ["RDP", "VNC", "SSH", "Telnet", "Serial", "SFTP", "FTP", "Custom"].includes(
          protocol.id,
        ),
    ),
  );
  const quickable = new Set(["RDP", "VNC", "SSH", "Telnet", "Serial"]);
  const recentOrder = $derived(
    Object.entries(records.records)
      .sort((left, right) => right[1].lastConnectedAt - left[1].lastConnectedAt)
      .map(([id]) => id),
  );
  const keywords = $derived(
    query
      .split(/\s+/)
      .map((token) => token.trim().toLowerCase())
      .filter((token) => token && !token.startsWith("#")),
  );
  const tagFilters = $derived(
    query
      .split(/\s+/)
      .filter((token) => token.startsWith("#"))
      .map((token) => token.replace(/^#/, "").toLowerCase()),
  );
  const filtered = $derived.by(() => {
    const tagMatch = (server: Server) =>
      tagFilters.every((tag) =>
        server.tags.some((candidate) => candidate.toLowerCase().startsWith(tag)),
      );
    const keywordMatch = (server: Server) =>
      keywords.every((keyword) =>
        `${server.displayName} ${server.subTitle} ${server.tags.join(" ")} ${server.note}`
          .toLowerCase()
          .includes(keyword),
      );
    const matching = servers.filter(
      (server) => tagMatch(server) && keywordMatch(server),
    );
    matching.sort((left, right) => {
      const leftIndex = recentOrder.indexOf(left.id);
      const rightIndex = recentOrder.indexOf(right.id);
      const leftKey = leftIndex < 0 ? Number.MAX_SAFE_INTEGER : leftIndex;
      const rightKey = rightIndex < 0 ? Number.MAX_SAFE_INTEGER : rightIndex;
      return leftKey - rightKey || left.displayName.localeCompare(right.displayName);
    });
    return matching;
  });
  const selected = $derived(filtered[selectedIndex] ?? null);
  const showNote = $derived(app.config?.launcher.showNote ?? true);
  const showCredentials = $derived(app.config?.launcher.showCredentials ?? false);

  $effect(() => {
    const height =
      mode === "quick"
        ? 520
        : Math.min(760, Math.max(240, 118 + Math.min(filtered.length, 9) * 58));
    if (resizeTimer) {
      clearTimeout(resizeTimer);
    }
    resizeTimer = setTimeout(() => {
      void commands.resizeLauncher(height).catch(() => undefined);
    }, 80);
  });

  onMount(() => {
    void refresh();
    focusSearch();
    subscribe("launcher:shown", () => focusSearch(), cleanup);
    subscribe("launcher:toggle", () => focusSearch(), cleanup);
    window.addEventListener("keydown", onGlobalKey);
  });

  onDestroy(() => {
    if (resizeTimer) {
      clearTimeout(resizeTimer);
    }
    window.removeEventListener("keydown", onGlobalKey);
    cleanup.dispose();
  });

  function focusSearch(): void {
    mode = "search";
    setTimeout(() => {
      searchInput?.focus();
      searchInput?.select();
    }, 10);
  }

  async function refresh(): Promise<void> {
    try {
      const [snapshot, connectionRecords] = await Promise.all([
        commands.catalogSnapshot(),
        commands.getConnectionRecords(),
      ]);
      catalog.snapshot = snapshot;
      servers = snapshot.servers;
      records = connectionRecords;
      selectedIndex = Math.min(selectedIndex, Math.max(0, filtered.length - 1));
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  function onGlobalKey(event: KeyboardEvent): void {
    if (event.key === "Escape") {
      event.preventDefault();
      void commands.hideLauncher().catch(() => undefined);
    }
  }

  function onSearchKey(event: KeyboardEvent): void {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      selectedIndex = Math.min(selectedIndex + 1, Math.max(0, filtered.length - 1));
      return;
    }
    if (event.key === "ArrowUp") {
      event.preventDefault();
      selectedIndex = Math.max(0, selectedIndex - 1);
      return;
    }
    if (event.key === "Enter") {
      event.preventDefault();
      if (selected) {
        void connect(selected);
      }
      return;
    }
    if (event.key === "Tab" && !event.shiftKey) {
      event.preventDefault();
      if (selected) {
        prefillQuick(selected);
      }
      mode = "quick";
    }
  }

  function prefillQuick(server: Server): void {
    quickProtocol = protocolId(server.protocol);
    quickAddress = server.address ?? "";
    quickPort = server.port;
    quickUsername = server.username ?? "";
    quickPassword = server.password ?? "";
    quickSave = false;
    quickMatch = app.config?.launcher.matchCredential ?? true;
    quickError = null;
  }

  function protocolId(protocol: Server["protocol"]): string {
    switch (protocol) {
      case "remoteApp":
        return "RDP";
      case "localApp":
        return "RDP";
      case "dummy":
        return "RDP";
      default:
        return protocol.toUpperCase();
    }
  }

  async function connect(server: Server): Promise<void> {
    await sessions.connect(server);
    await commands.hideLauncher().catch(() => undefined);
    await refresh();
  }

  function highlight(text: string): TextRange[] {
    if (keywords.length === 0) {
      return [];
    }
    const lower = text.toLowerCase();
    const ranges: TextRange[] = [];
    for (const keyword of keywords) {
      let start = lower.indexOf(keyword);
      while (start >= 0) {
        ranges.push({ start, end: start + keyword.length });
        start = lower.indexOf(keyword, start + 1);
      }
    }
    return ranges.sort((left, right) => left.start - right.start);
  }

  async function copyPassword(value: string): Promise<void> {
    if (!value || !(await app.ensureVerified("Copy a stored password"))) {
      return;
    }
    await copy(value, "Password");
  }

  async function copy(value: string, label: string): Promise<void> {
    try {
      await navigator.clipboard.writeText(value);
      app.showNotice(`${label} copied`, "info");
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async function createShortcut(server: Server): Promise<void> {
    try {
      const result = await commands.createShortcutForServers(
        [server.id],
        server.displayName,
        false,
      );
      app.showNotice(result.path, "info");
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async function exportRdp(server: Server): Promise<void> {
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

  async function duplicate(server: Server): Promise<void> {
    try {
      const template = await commands.newServer(server.protocol);
      const copy = {
        ...structuredClone(server),
        id: template.id,
        sourceId: "local",
        displayName: `${server.displayName} (copy)`,
      };
      await commands.saveServer(copy);
      await refresh();
      app.showNotice(`Duplicated '${server.displayName}'.`, "info");
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async function remove(server: Server): Promise<void> {
    const confirmed = await dialogs.confirm({
      title: `Delete '${server.displayName}'?`,
      message: "This cannot be undone.",
      danger: true,
    });
    if (!confirmed) {
      return;
    }
    try {
      await commands.deleteServer(server.id);
      await refresh();
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async function edit(server: Server): Promise<void> {
    try {
      await commands.focusServerInCatalog(server.id);
      await commands.hideLauncher().catch(() => undefined);
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async function runQuickConnect(): Promise<void> {
    quickBusy = true;
    quickError = null;
    try {
      const outcome = await commands.quickConnect({
        protocol: quickProtocol,
        address: quickAddress,
        port: quickPort,
        username: quickUsername,
        password: quickPassword || undefined,
        save: quickSave,
        matchCredential: quickMatch,
      });
      if (outcome.saved) {
        await refresh();
      }
      sessions.handleEvent({ kind: "state", session: outcome.session });
      await sessions.hostSession(outcome.session.id);
      await commands.hideLauncher().catch(() => undefined);
      await refresh();
    } catch (reason) {
      quickError = reason instanceof Error ? reason.message : String(reason);
    } finally {
      quickBusy = false;
    }
  }

  function useHistory(entry: (typeof records.quickConnect)[number]): void {
    quickProtocol = entry.protocol;
    quickAddress = entry.address;
    quickPort = entry.port;
    quickUsername = entry.username;
    quickError = null;
  }
</script>

<div class="launcher">
  <header data-tauri-drag-region>
    <div class="identity" data-tauri-drag-region>
      <span class="mark" aria-hidden="true">H</span>
      <span>H-Remote</span>
    </div>
    <span class="hint">
      {mode === "search" ? "Type to search · Tab for quick connect · Esc to hide" : "Tab/Shift+Tab: search · Esc to hide"}
    </span>
  </header>

  {#if mode === "search"}
    <div class="search-row">
      <input
        bind:this={searchInput}
        type="search"
        class="search"
        placeholder="Search servers, tags or notes…"
        aria-label="Search servers"
        value={query}
        oninput={(event) => {
          query = event.currentTarget.value;
          selectedIndex = 0;
        }}
        onkeydown={onSearchKey}
      />
      <span class="count">{filtered.length}</span>
    </div>

    <div class="results" role="listbox" aria-label="Servers">
      {#each filtered as server, index (server.id)}
        <button
          type="button"
          role="option"
          aria-selected={index === selectedIndex}
          class="result"
          class:selected={index === selectedIndex}
          onclick={() => {
            selectedIndex = index;
            void connect(server);
          }}
          onmouseenter={() => (selectedIndex = index)}
        >
          <ServerIcon {server} size={30} />
          <span class="text">
            <span class="name"><HighlightedText text={server.displayName} ranges={highlight(server.displayName)} /></span>
            <span class="sub">
              <HighlightedText text={server.subTitle} ranges={highlight(server.subTitle)} />
              {#if showCredentials && server.username}
                <em>· {server.username}</em>
              {/if}
            </span>
            {#if showNote && server.note}
              <span class="note">{server.note}</span>
            {/if}
          </span>
          <span class="tags">
            {#if catalog.hasMultipleSources()}
              <span class="source" class:readonly={(server.sourceId ?? "local") !== "local"}>
                {catalog.sourceLabel(server.sourceId)}
              </span>
            {/if}
            {#each server.tags.slice(0, 3) as tag (tag)}
              <span>#{tag}</span>
            {/each}
          </span>
        </button>
      {:else}
        <p class="empty">No matching servers. Press Tab to quick connect.</p>
      {/each}
    </div>

    {#if selected}
      <footer class="actions">
        <Button size="sm" variant="primary" onclick={() => void connect(selected)}>Connect</Button>
        {#each selected.alternateCredentials ?? [] as alternate (alternate.name)}
          <Button size="sm" variant="ghost" onclick={() => void sessions.connectVia(selected, alternate.name)}>Via {alternate.name || alternate.address}</Button>
        {/each}
        <Button size="sm" variant="ghost" onclick={() => void edit(selected)}>Edit</Button>
        <Button size="sm" variant="ghost" onclick={() => void duplicate(selected)}>Duplicate</Button>
        <Button size="sm" variant="ghost" onclick={() => void createShortcut(selected)}>Shortcut</Button>
        <Button size="sm" variant="ghost" disabled={selected.protocol !== "rdp"} onclick={() => void exportRdp(selected)}>Export .rdp</Button>
        <Button size="sm" variant="ghost" onclick={() => void copy(selected.address ?? "", "Hostname")}>Copy host</Button>
        <Button size="sm" variant="ghost" onclick={() => void copy(selected.username ?? "", "Username")}>Copy user</Button>
        <Button size="sm" variant="ghost" onclick={() => void copyPassword(selected.password ?? "")}>Copy pass</Button>
        <Button size="sm" variant="ghost" onclick={() => app.showNotice("Alternate credentials arrive with the protocol runners (Phase 6).", "info")}>Alternate</Button>
        <Button size="sm" variant="ghost" onclick={() => app.showNotice("Runner selection arrives in Phase 6.", "info")}>Runner</Button>
        <Button size="sm" variant="danger" onclick={() => void remove(selected)}>Delete</Button>
        {#if selected.protocol === "ssh"}
          <Button size="sm" variant="ghost" onclick={() => app.showNotice("Open SFTP arrives with the SSH engine (Phase 6).", "info")}>Open SFTP</Button>
        {/if}
      </footer>
    {/if}
  {:else}
    <div class="quick">
      <div class="quick-head">
        <h2>Quick connect</h2>
        <button type="button" class="link" onclick={() => (mode = "search")}>Back to search</button>
      </div>
      <div class="quick-grid">
        <label>
          <span>Protocol</span>
          <select bind:value={quickProtocol}>
            {#each protocols as protocol (protocol.id)}
              <option value={protocol.id} disabled={!quickable.has(protocol.id)}>
                {protocol.displayName}{quickable.has(protocol.id) ? "" : " (Phase 6b)"}
              </option>
            {/each}
          </select>
        </label>
        <label class="grow">
          <span>Address</span>
          <input bind:value={quickAddress} placeholder="host or IP" />
        </label>
        <label class="port">
          <span>Port</span>
          <input type="number" bind:value={quickPort} min="1" max="65535" />
        </label>
        <label>
          <span>Username</span>
          <input bind:value={quickUsername} placeholder="optional" />
        </label>
        <label>
          <span>Password</span>
          <input type="password" bind:value={quickPassword} placeholder="optional" />
        </label>
      </div>
      <div class="quick-options">
        <label><input type="checkbox" bind:checked={quickSave} /> Save server in the catalog</label>
        <label><input type="checkbox" bind:checked={quickMatch} /> Match stored credentials</label>
      </div>
      {#if quickError}
        <p class="error" role="alert">{quickError}</p>
      {/if}
      <div class="quick-actions">
        <Button variant="primary" disabled={quickBusy || !quickAddress.trim()} onclick={() => void runQuickConnect()}>
          {quickBusy ? i18n.t("common.loading") : "Connect"}
        </Button>
        <Button variant="secondary" onclick={() => (mode = "search")}>{i18n.t("common.cancel")}</Button>
      </div>
      <div class="history">
        <h3>Recent quick connections</h3>
        {#each records.quickConnect as entry, index (index)}
          <button type="button" class="history-row" onclick={() => useHistory(entry)}>
            <span>{entry.protocol}</span>
            <strong>{entry.address}:{entry.port}</strong>
            <span>{entry.username}</span>
          </button>
        {:else}
          <p class="empty">No quick connections yet.</p>
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  .launcher {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--hr-primary-dark);
    color: var(--hr-primary-text);
    border: 1px solid var(--hr-border-strong);
    overflow: hidden;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--hr-border);
    background: color-mix(in srgb, var(--hr-primary-mid) 80%, var(--hr-background));
  }

  .identity {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 650;
    font-size: 12px;
  }

  .mark {
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    border-radius: 5px;
    background: var(--hr-accent-mid);
    color: var(--hr-accent-text);
    font-size: 11px;
  }

  .hint {
    font-size: 10px;
    color: color-mix(in srgb, var(--hr-background-text) 55%, transparent);
  }

  .search-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px;
    border-bottom: 1px solid var(--hr-border);
  }

  .search {
    flex: 1;
    min-height: 38px;
    border-radius: var(--hr-radius);
    border: 1px solid var(--hr-border-strong);
    background: color-mix(in srgb, var(--hr-primary-dark) 60%, var(--hr-background));
    color: inherit;
    padding: 0 12px;
    font-size: 14px;
  }

  .count {
    font-size: 10px;
    color: color-mix(in srgb, var(--hr-background-text) 50%, transparent);
  }

  .results {
    flex: 1;
    overflow: auto;
    padding: 4px;
  }

  .result {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    min-height: 52px;
    padding: 5px 8px;
    border: 1px solid transparent;
    border-radius: 7px;
    background: transparent;
    color: inherit;
    text-align: left;
  }

  .result.selected {
    background: color-mix(in srgb, var(--hr-accent-mid) 16%, transparent);
    border-color: color-mix(in srgb, var(--hr-accent-mid) 50%, transparent);
  }

  .text {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
    flex: 1;
  }

  .name {
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .sub,
  .note {
    font-size: 10px;
    color: color-mix(in srgb, var(--hr-background-text) 60%, transparent);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .sub em {
    font-style: normal;
    color: var(--hr-accent-light);
  }

  .tags {
    display: flex;
    gap: 4px;
    flex: none;
    font-size: 9px;
    color: var(--hr-accent-light);
  }

  .tags .source {
    color: color-mix(in srgb, var(--hr-background-text) 48%, transparent);
    border: 1px solid var(--hr-border);
    border-radius: 999px;
    padding: 0 5px;
  }

  .tags .source.readonly {
    color: var(--hr-warning);
    border-color: color-mix(in srgb, var(--hr-warning) 40%, transparent);
  }

  .empty {
    padding: 18px;
    text-align: center;
    font-size: 12px;
    color: color-mix(in srgb, var(--hr-background-text) 55%, transparent);
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    padding: 8px 10px;
    border-top: 1px solid var(--hr-border);
    background: color-mix(in srgb, var(--hr-primary-mid) 55%, transparent);
  }

  .quick {
    flex: 1;
    overflow: auto;
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .quick-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .quick-head h2 {
    font-size: 15px;
  }

  .link {
    border: 0;
    background: transparent;
    color: var(--hr-accent-light);
    font-size: 11px;
  }

  .quick-grid {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .quick-grid label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 10px;
    min-width: 120px;
  }

  .quick-grid label.grow {
    flex: 1;
  }

  .quick-grid label.port {
    max-width: 90px;
  }

  .quick-grid input,
  .quick-grid select {
    min-height: 32px;
    border-radius: var(--hr-radius);
    border: 1px solid var(--hr-border);
    background: color-mix(in srgb, var(--hr-primary-dark) 55%, var(--hr-background));
    color: inherit;
    padding: 0 8px;
  }

  .quick-options {
    display: flex;
    gap: 16px;
    font-size: 11px;
  }

  .quick-actions {
    display: flex;
    gap: 8px;
  }

  .error {
    color: var(--hr-danger);
    font-size: 11px;
  }

  .history {
    border-top: 1px solid var(--hr-border);
    padding-top: 8px;
  }

  .history h3 {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: color-mix(in srgb, var(--hr-background-text) 55%, transparent);
    margin-bottom: 5px;
  }

  .history-row {
    display: flex;
    gap: 8px;
    width: 100%;
    border: 0;
    border-radius: 5px;
    background: transparent;
    color: inherit;
    padding: 5px 6px;
    font-size: 11px;
    text-align: left;
  }

  .history-row:hover {
    background: color-mix(in srgb, var(--hr-accent-mid) 12%, transparent);
  }

  .history-row span:first-child {
    width: 48px;
    color: var(--hr-accent-light);
  }

  .history-row span:last-child {
    margin-left: auto;
    color: color-mix(in srgb, var(--hr-background-text) 55%, transparent);
  }
</style>
