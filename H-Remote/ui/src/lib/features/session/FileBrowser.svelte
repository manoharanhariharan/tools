<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type {
    FileEntry,
    FileEvent,
    FileListing,
    TransferSnapshot,
  } from "../../ipc/contracts";
  import { commands } from "../../ipc/client";
  import { CleanupRegistry, subscribe } from "../../ipc/events";
  import { app } from "../../state/app.svelte";
  import { dialogs } from "../../state/dialogs.svelte";
  import Button from "../../components/Button.svelte";
  import Icon from "../../components/Icon.svelte";

  interface Props {
    sessionId: string;
    serverName: string;
  }
  let { sessionId, serverName }: Props = $props();

  type SortKey = "name" | "size" | "modified";
  let listing = $state<FileListing | null>(null);
  let loading = $state(true);
  let pathInput = $state("");
  let selected = $state<string[]>([]);
  let sortKey = $state<SortKey>("name");
  let sortAsc = $state(true);
  let transfers = $state<TransferSnapshot[]>([]);
  let dateFormat = $state(localStorage.getItem("hremote.fileDateFormat") ?? "short");
  let cleanup = new CleanupRegistry();

  const dateFormats: Array<[string, string]> = [
    ["iso", "ISO (2026-11-22 17:00)"],
    ["short", "Short (22/11/2026 17:00)"],
    ["medium", "Medium (22 Nov 2026 17:00)"],
    ["date", "Date only (22/11/2026)"],
    ["time", "Time only (17:00)"],
    ["relative", "Relative (2 h ago)"],
  ];

  const entries = $derived.by(() => {
    const list = [...(listing?.entries ?? [])];
    list.sort((left, right) => {
      if (left.isDir !== right.isDir) {
        return left.isDir ? -1 : 1;
      }
      let ordering = 0;
      if (sortKey === "name") {
        ordering = left.name.toLowerCase().localeCompare(right.name.toLowerCase());
      } else if (sortKey === "size") {
        ordering = left.size - right.size;
      } else {
        ordering = (left.modified ?? 0) - (right.modified ?? 0);
      }
      return sortAsc ? ordering : -ordering;
    });
    return list;
  });
  const selectedEntries = $derived(
    entries.filter((entry) => selected.includes(entry.path)),
  );
  const canDownload = $derived(
    selectedEntries.length > 0 && selectedEntries.every((entry) => !entry.isDir),
  );

  $effect(() => {
    localStorage.setItem("hremote.fileDateFormat", dateFormat);
  });

  onMount(() => {
    void load();
    void commands.fileTransfers().then((items) => (transfers = items)).catch(() => undefined);
    subscribe<FileEvent>(
      "file:event",
      (event) => {
        if (event.kind !== "transfer") {
          return;
        }
        const index = transfers.findIndex((item) => item.id === event.transfer.id);
        if (index >= 0) {
          transfers[index] = event.transfer;
        } else {
          transfers = [...transfers, event.transfer];
        }
      },
      cleanup,
    );
  });

  onDestroy(() => cleanup.dispose());

  async function load(path?: string): Promise<void> {
    loading = true;
    try {
      listing = await commands.fileList(sessionId, path);
      pathInput = listing.path;
      selected = [];
    } catch (reason) {
      await app.reportError(reason);
    } finally {
      loading = false;
    }
  }

  function toggleSort(key: SortKey): void {
    if (sortKey === key) {
      sortAsc = !sortAsc;
    } else {
      sortKey = key;
      sortAsc = true;
    }
  }

  function toggle(entry: FileEntry): void {
    selected = selected.includes(entry.path)
      ? selected.filter((path) => path !== entry.path)
      : [...selected, entry.path];
  }

  function parent(path: string): string {
    const trimmed = path.replace(/\/+$/, "");
    const index = trimmed.lastIndexOf("/");
    if (index <= 0) {
      return "/";
    }
    return trimmed.slice(0, index);
  }

  async function open(entry: FileEntry): Promise<void> {
    if (entry.isDir) {
      await load(entry.path);
    } else {
      toggle(entry);
    }
  }

  async function upload(): Promise<void> {
    try {
      const tasks = await commands.fileUploadDialog(sessionId, listing?.path ?? "/");
      if (tasks.length === 0) {
        return;
      }
      transfers = [...transfers, ...tasks];
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async function download(): Promise<void> {
    if (!canDownload) {
      app.showNotice("Select one or more files (folders are not downloaded yet).", "warning");
      return;
    }
    try {
      const tasks = await commands.fileDownloadDialog(sessionId, selected);
      if (tasks.length > 0) {
        transfers = [...transfers, ...tasks];
      }
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async function rename(): Promise<void> {
    if (selectedEntries.length !== 1) {
      app.showNotice("Select one entry to rename.", "warning");
      return;
    }
    const entry = selectedEntries[0];
    const name = await dialogs.input({
      title: "Rename",
      label: "New name",
      value: entry.name,
      submitLabel: "Rename",
    });
    if (!name?.trim() || name.trim() === entry.name) {
      return;
    }
    try {
      await commands.fileRename(
        sessionId,
        entry.path,
        `${listing?.path.replace(/\/$/, "")}/${name.trim()}`,
      );
      await load();
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async function remove(): Promise<void> {
    if (selectedEntries.length === 0) {
      return;
    }
    const confirmed = await dialogs.confirm({
      title: `Delete ${selectedEntries.length} item(s)?`,
      message: selectedEntries.map((entry) => entry.name).join(", "),
      danger: true,
    });
    if (!confirmed) {
      return;
    }
    try {
      for (const entry of selectedEntries) {
        await commands.fileDelete(sessionId, entry.path, entry.isDir);
      }
      await load();
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async function newFolder(): Promise<void> {
    const name = await dialogs.input({
      title: "New folder",
      label: "Folder name",
      submitLabel: "Create",
    });
    if (!name?.trim()) {
      return;
    }
    try {
      await commands.fileMkdir(
        sessionId,
        `${listing?.path.replace(/\/$/, "")}/${name.trim()}`,
      );
      await load();
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async function cancelTransfer(id: string): Promise<void> {
    await commands.fileCancelTransfer(id).catch(() => false);
  }

  function formatDate(value: number | null): string {
    if (!value) {
      return "—";
    }
    const date = new Date(value * 1000);
    switch (dateFormat) {
      case "iso":
        return date.toISOString().slice(0, 16).replace("T", " ");
      case "date":
        return date.toLocaleDateString();
      case "time":
        return date.toLocaleTimeString();
      case "medium":
        return date.toLocaleString(undefined, {
          day: "2-digit",
          month: "short",
          year: "numeric",
          hour: "2-digit",
          minute: "2-digit",
        });
      case "relative": {
        const seconds = Math.max(1, Math.round((Date.now() - date.getTime()) / 1000));
        if (seconds < 60) return `${seconds} s ago`;
        if (seconds < 3600) return `${Math.round(seconds / 60)} min ago`;
        if (seconds < 86400) return `${Math.round(seconds / 3600)} h ago`;
        return `${Math.round(seconds / 86400)} d ago`;
      }
      default:
        return date.toLocaleString();
    }
  }

  function formatSize(value: number): string {
    if (value < 1024) return `${value} B`;
    if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KiB`;
    return `${(value / (1024 * 1024)).toFixed(1)} MiB`;
  }

  function progress(transfer: TransferSnapshot): number {
    if (transfer.total <= 0) {
      return transfer.state === "completed" ? 100 : 0;
    }
    return Math.min(100, Math.round((transfer.bytes / transfer.total) * 100));
  }
</script>

<section class="files" aria-label={`${serverName} files`}>
  <header>
    <div class="path">
      <button type="button" title="Home" aria-label="Home folder" onclick={() => void load()}>
        <Icon name="folder" size={14} />
      </button>
      <button type="button" title="Parent folder" aria-label="Parent folder" onclick={() => void load(parent(listing?.path ?? "/"))}>
        ↑
      </button>
      <input
        aria-label="Remote path"
        bind:value={pathInput}
        onkeydown={(event) => {
          if (event.key === "Enter") {
            void load(pathInput);
          }
        }}
      />
      <Button size="sm" variant="secondary" onclick={() => void load(pathInput)}>Go</Button>
      <Button size="sm" variant="ghost" title="Refresh" onclick={() => void load()}><Icon name="refresh" size={14} /></Button>
    </div>
    <div class="actions">
      <Button size="sm" variant="secondary" onclick={() => void upload()}>Upload</Button>
      <Button size="sm" variant="secondary" disabled={!canDownload} onclick={() => void download()}>Download</Button>
      <Button size="sm" variant="ghost" onclick={() => void newFolder()}>New folder</Button>
      <Button size="sm" variant="ghost" disabled={selected.length !== 1} onclick={() => void rename()}>Rename</Button>
      <Button size="sm" variant="danger" disabled={selected.length === 0} onclick={() => void remove()}>Delete</Button>
      <select bind:value={dateFormat} aria-label="Date format">
        {#each dateFormats as [value, label] (value)}
          <option value={value}>{label}</option>
        {/each}
      </select>
    </div>
  </header>

  <div class="table" role="table" aria-label="Remote files">
    <div class="row head" role="row">
      <span></span>
      <button type="button" role="columnheader" onclick={() => toggleSort("name")}>Name {sortKey === "name" ? (sortAsc ? "↑" : "↓") : ""}</button>
      <button type="button" role="columnheader" onclick={() => toggleSort("size")}>Size {sortKey === "size" ? (sortAsc ? "↑" : "↓") : ""}</button>
      <button type="button" role="columnheader" onclick={() => toggleSort("modified")}>Modified {sortKey === "modified" ? (sortAsc ? "↑" : "↓") : ""}</button>
      <span role="columnheader">Type</span>
    </div>
    <div class="scroll">
      {#if loading}
        <p class="empty">Loading…</p>
      {:else}
        {#each entries as entry (entry.path)}
          <div
            class="row"
            class:selected={selected.includes(entry.path)}
            role="row"
            onclick={() => toggle(entry)}
            ondblclick={() => void open(entry)}
            onkeydown={(event) => {
              if (event.key === "Enter") {
                void open(entry);
              }
            }}
            tabindex="0"
          >
            <input
              type="checkbox"
              checked={selected.includes(entry.path)}
              aria-label={`Select ${entry.name}`}
              onclick={(event) => event.stopPropagation()}
              onchange={() => toggle(entry)}
            />
            <span class="name">
              <span class="glyph">{entry.isDir ? "▣" : "▤"}</span>
              {entry.name}
            </span>
            <span>{entry.isDir ? "—" : formatSize(entry.size)}</span>
            <span>{formatDate(entry.modified)}</span>
            <span>{entry.isDir ? "Folder" : "File"}</span>
          </div>
        {:else}
          <p class="empty">This folder is empty.</p>
        {/each}
      {/if}
    </div>
  </div>

  {#if transfers.length > 0}
    <div class="transfers">
      <h3>Transfers</h3>
      {#each transfers as transfer (transfer.id)}
        <div class="transfer">
          <span class="direction">{transfer.direction === "upload" ? "↑" : "↓"}</span>
          <span class="file" title={transfer.remotePath}>{transfer.remotePath.split("/").pop()}</span>
          <span class="meter"><i style:width="{progress(transfer)}%"></i></span>
          <span class="state">
            {transfer.state === "running" ? `${progress(transfer)}%` : transfer.state}
          </span>
          {#if transfer.state === "running"}
            <button type="button" title="Cancel transfer" onclick={() => void cancelTransfer(transfer.id)}>×</button>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</section>

<style>
  .files {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
    color: #dbe8f3;
    font-size: 12px;
  }

  header {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 7px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.1);
    background: #132436;
  }

  .path,
  .actions {
    display: flex;
    align-items: center;
    gap: 5px;
    flex-wrap: wrap;
  }

  .path input {
    flex: 1;
    min-width: 160px;
    min-height: 28px;
    border-radius: 5px;
    border: 1px solid rgba(255, 255, 255, 0.16);
    background: #0b1520;
    color: inherit;
    padding: 0 8px;
    font: inherit;
  }

  .path button {
    border: 1px solid rgba(255, 255, 255, 0.14);
    background: transparent;
    color: inherit;
    border-radius: 5px;
    height: 28px;
    min-width: 28px;
  }

  select {
    height: 26px;
    border-radius: 5px;
    border: 1px solid rgba(255, 255, 255, 0.16);
    background: #0b1520;
    color: inherit;
    font-size: 10px;
  }

  .table {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .row {
    display: grid;
    grid-template-columns: 28px minmax(120px, 1fr) 90px 150px 70px;
    gap: 6px;
    align-items: center;
    padding: 3px 8px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    cursor: default;
  }

  .row.head {
    color: #8aa3b8;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    background: #0e1c29;
  }

  .row.head button {
    border: 0;
    background: transparent;
    color: inherit;
    text-align: left;
    font: inherit;
    padding: 0;
  }

  .row.selected {
    background: rgba(83, 158, 226, 0.16);
  }

  .row:hover {
    background: rgba(255, 255, 255, 0.05);
  }

  .scroll {
    flex: 1;
    overflow: auto;
    min-height: 0;
  }

  .name {
    display: flex;
    align-items: center;
    gap: 6px;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .glyph {
    color: #8fd0ff;
  }

  .empty {
    padding: 20px;
    text-align: center;
    color: #8aa3b8;
  }

  .transfers {
    max-height: 130px;
    overflow: auto;
    border-top: 1px solid rgba(255, 255, 255, 0.12);
    padding: 6px 8px;
    background: #0e1c29;
  }

  .transfers h3 {
    font-size: 10px;
    text-transform: uppercase;
    color: #8aa3b8;
    margin-bottom: 4px;
  }

  .transfer {
    display: grid;
    grid-template-columns: 16px minmax(80px, 1fr) 120px 54px 18px;
    gap: 6px;
    align-items: center;
    font-size: 10px;
    padding: 2px 0;
  }

  .file {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .meter {
    height: 5px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.1);
    overflow: hidden;
  }

  .meter i {
    display: block;
    height: 100%;
    background: #4ea1e8;
  }

  .state {
    text-align: right;
    color: #9fb6c9;
  }

  .transfer button {
    border: 0;
    background: transparent;
    color: inherit;
  }
</style>
