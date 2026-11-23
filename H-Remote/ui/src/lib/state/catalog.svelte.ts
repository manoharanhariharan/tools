import { SvelteSet } from "svelte/reactivity";
import type {
  CatalogSnapshot,
  CatalogTree,
  CatalogView,
  Folder,
  ListViewLocality,
  Server,
  SourceStatus,
  Tag,
  TreeViewLocality,
} from "../ipc/contracts";
import { commands } from "../ipc/client";
import { effectiveMatchers } from "./matchers";
import { app } from "./app.svelte";

export type CatalogViewMode = "list" | "card" | "tree";
export type EditorMode = "add" | "edit" | "duplicate" | "bulk";

export interface CatalogEditor {
  mode: EditorMode;
  draft: Server;
  servers: Server[];
  dirty: SvelteSet<string>;
  original: Server | null;
}

const DEFAULT_COLUMNS: Record<string, number> = {
  protocol: 118,
  name: 240,
  ip: 170,
  note: 260,
  target: 230,
};

class CatalogStore {
  snapshot = $state<CatalogSnapshot | null>(null);
  view = $state<CatalogView | null>(null);
  loading = $state(true);
  refreshing = $state(false);
  error = $state<string | null>(null);
  query = $state("");
  providers = $state<string[]>(["direct"]);
  sort = $state("insertion");
  viewMode = $state<CatalogViewMode>("list");
  selected = new SvelteSet<string>();
  expanded = new SvelteSet<string>();
  editor = $state<CatalogEditor | null>(null);
  listView = $state<ListViewLocality | null>(null);
  treeView = $state<TreeViewLocality | null>(null);
  /** Local + additional source health (P8-06). */
  sources = $state<SourceStatus[]>([]);
  /** `all` or a source id; extra sources are read-only (P8-10/P8-11). */
  sourceFilter = $state("all");
  columnWidths = $state<Record<string, number>>({ ...DEFAULT_COLUMNS });
  private refreshTimer: ReturnType<typeof setTimeout> | undefined;
  private refreshQueued = false;
  private disposed = false;

  async load(): Promise<void> {
    this.disposed = false;
    this.loading = true;
    this.error = null;
    try {
      const [snapshot, locality, listView, treeView, sources] = await Promise.all([
        commands.catalogSnapshot(),
        commands.getLocality(),
        commands.getListView(),
        commands.getTreeView(),
        commands.dataSourceStatuses().catch(() => [] as SourceStatus[]),
      ]);
      this.snapshot = snapshot;
      this.listView = listView;
      this.treeView = treeView;
      this.sources = sources;
      this.sort = listView.sort;
      this.viewMode = locality.ui.serverView;
      this.columnWidths = { ...DEFAULT_COLUMNS, ...listView.columnWidths };
      this.expanded.clear();
      for (const path of treeView.expanded) {
        this.expanded.add(path);
      }
      for (const [path, open] of Object.entries(listView.groupExpanded)) {
        if (open) {
          this.expanded.add(path);
        }
      }
      this.providers = effectiveMatchers(
        app.config?.keywordMatch,
        app.bootstrap?.locale ?? "en-US",
      );
      await this.refresh();
    } catch (reason) {
      this.error = reason instanceof Error ? reason.message : String(reason);
    } finally {
      this.loading = false;
    }
  }

  async refresh(): Promise<void> {
    if (this.disposed) {
      return;
    }
    if (this.refreshing) {
      // Coalesce bursts (typing, bulk actions) into one follow-up run (P10-07).
      this.refreshQueued = true;
      return;
    }
    this.refreshing = true;
    try {
      do {
        this.refreshQueued = false;
        const view = await commands.catalogView({
          query: this.query,
          providers: this.providers,
          sort: this.sort,
          customOrder: this.listView?.customOrder ?? [],
          folderOrder: this.treeView?.nodeOrder ?? [],
          source: this.sourceFilter,
        });
        this.view = view;
        this.snapshot = this.snapshot
          ? { ...this.snapshot, tags: view.tags }
          : this.snapshot;
        const visible = new Set(this.visibleServerIds());
        for (const id of [...this.selected]) {
          if (!visible.has(id)) {
            this.selected.delete(id);
          }
        }
      } while (this.refreshQueued && !this.disposed);
    } catch (reason) {
      await app.reportError(reason);
    } finally {
      this.refreshing = false;
    }
  }

  setQuery(query: string): void {
    this.query = query;
    if (this.refreshTimer) {
      clearTimeout(this.refreshTimer);
    }
    this.refreshTimer = setTimeout(() => {
      void this.refresh();
    }, 150);
  }

  async setViewMode(mode: CatalogViewMode): Promise<void> {
    this.viewMode = mode;
    try {
      await commands.setServerView(mode as "card" | "list" | "tree");
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async setSort(sort: string): Promise<void> {
    this.sort = sort;
    if (this.listView) {
      this.listView = { ...this.listView, sort: sort as ListViewLocality["sort"] };
      try {
        await commands.updateListView(this.listView);
      } catch (reason) {
        await app.reportError(reason);
      }
    }
    await this.refresh();
  }

  async setColumnWidth(column: string, width: number): Promise<void> {
    this.columnWidths = { ...this.columnWidths, [column]: Math.max(80, width) };
    if (this.listView) {
      this.listView = {
        ...this.listView,
        columnWidths: { ...this.columnWidths },
      };
      try {
        await commands.updateListView(this.listView);
      } catch (reason) {
        await app.reportError(reason);
      }
    }
  }

  async toggleNotes(): Promise<void> {
    await app.patchConfig((draft) => {
      draft.general.showNoteInList = !draft.general.showNoteInList;
    });
  }

  async toggleFolder(path: string[]): Promise<void> {
    const key = path.join("/");
    if (this.expanded.has(key)) {
      this.expanded.delete(key);
    } else {
      this.expanded.add(key);
    }
    if (this.listView) {
      this.listView = {
        ...this.listView,
        groupExpanded: { ...this.listView.groupExpanded, [key]: this.expanded.has(key) },
      };
    }
    if (this.treeView) {
      this.treeView = {
        ...this.treeView,
        expanded: [...this.expanded],
      };
    }
    try {
      await Promise.all([
        this.listView ? commands.updateListView(this.listView) : Promise.resolve(null),
        this.treeView ? commands.updateTreeView(this.treeView) : Promise.resolve(null),
      ]);
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  isExpanded(path: string[]): boolean {
    return this.expanded.has(path.join("/"));
  }

  rootFolders(): CatalogTree["folders"] {
    return this.view?.tree.folders ?? [];
  }

  rootServers(): Server[] {
    return this.view?.tree.servers ?? [];
  }

  visibleServerIds(): string[] {
    const ids: string[] = [];
    const walk = (servers: Server[], folders: CatalogTree["folders"]) => {
      for (const server of servers) {
        ids.push(server.id);
      }
      for (const folder of folders) {
        walk(folder.servers, folder.folders);
      }
    };
    walk(this.view?.tree.servers ?? [], this.view?.tree.folders ?? []);
    return ids;
  }

  visibleServers(): Server[] {
    const all: Server[] = [];
    const walk = (servers: Server[], folders: CatalogTree["folders"]) => {
      all.push(...servers);
      for (const folder of folders) {
        walk(folder.servers, folder.folders);
      }
    };
    walk(this.view?.tree.servers ?? [], this.view?.tree.folders ?? []);
    return all;
  }

  /** Folder paths in the current display order, used to seed custom ordering. */
  visibleFolderPaths(): string[] {
    const paths: string[] = [];
    const walk = (folders: CatalogTree["folders"]) => {
      for (const folder of folders) {
        paths.push(folder.path.join("/"));
        walk(folder.folders);
      }
    };
    walk(this.view?.tree.folders ?? []);
    return paths;
  }

  /** Sibling folder paths (same parent) in display order. */
  siblingFolderPaths(path: string[]): string[] {
    const parent = path.slice(0, -1).join("/");
    return this.visibleFolderPaths().filter(
      (candidate) => candidate.split("/").slice(0, -1).join("/") === parent,
    );
  }

  /** Sibling server ids (same folder) in display order. */
  siblingServerIds(serverId: string): string[] {
    const target = this.serverById(serverId) ?? this.visibleServers().find(
      (server) => server.id === serverId,
    );
    if (!target) {
      return [];
    }
    const folder = target.treeNodes.join("/");
    return this.visibleServers()
      .filter((server) => server.treeNodes.join("/") === folder)
      .map((server) => server.id);
  }

  allSelected(): boolean {
    const visible = this.visibleServerIds();
    return visible.length > 0 && visible.every((id) => this.selected.has(id));
  }

  someSelected(): boolean {
    const visible = this.visibleServerIds();
    return visible.some((id) => this.selected.has(id)) && !this.allSelected();
  }

  toggleSelected(id: string): void {
    if (this.selected.has(id)) {
      this.selected.delete(id);
    } else {
      this.selected.add(id);
    }
  }

  toggleSelectAll(): void {
    if (this.allSelected()) {
      this.selected.clear();
      return;
    }
    for (const id of this.visibleServerIds()) {
      this.selected.add(id);
    }
  }

  selectFolder(servers: Server[]): void {
    for (const server of servers) {
      this.selected.add(server.id);
    }
  }

  clearSelection(): void {
    this.selected.clear();
  }

  selectedServers(): Server[] {
    return (this.snapshot?.servers ?? []).filter((server) =>
      this.selected.has(server.id),
    );
  }

  serverById(id: string): Server | undefined {
    return this.snapshot?.servers.find((server) => server.id === id);
  }

  hitFor(id: string) {
    return this.view?.hits.find((hit) => hit.id === id);
  }

  isReadOnly(server: Server): boolean {
    return (server.sourceId ?? "local") !== "local";
  }

  sourceLabel(sourceId: string | null | undefined): string {
    const id = sourceId ?? "local";
    if (id === "local") {
      return "Local";
    }
    return this.sources.find((source) => source.id === id)?.name ?? "Source";
  }

  /** Extra sources that are configured, not just the local catalog. */
  configuredSources(): SourceStatus[] {
    return this.sources.filter((source) => source.id !== "local");
  }

  hasMultipleSources(): boolean {
    return this.configuredSources().length > 0;
  }

  sourceState(sourceId: string): SourceStatus | undefined {
    return this.sources.find((source) => source.id === sourceId);
  }

  async setSourceFilter(source: string): Promise<void> {
    this.sourceFilter = source;
    this.clearSelection();
    await this.refresh();
  }

  async refreshSources(): Promise<void> {
    try {
      this.sources = await commands.dataSourceStatuses();
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  /** Forces a reload of every configured source and the visible catalog (P8-14). */
  async refreshAllSources(): Promise<void> {
    try {
      this.sources = await commands.dataSourceRefreshAll();
      await this.load();
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  /** Called after a source is added, edited or removed in Settings. */
  async reloadAfterSourceChange(): Promise<void> {
    this.sourceFilter = "all";
    await this.load();
  }

  /** Groups a flat server list by source, local first (P8-10). */
  groupServers(servers: Server[]): Array<{
    id: string;
    label: string;
    readOnly: boolean;
    servers: Server[];
  }> {
    const groups = new Map<string, Server[]>();
    for (const server of servers) {
      const id = server.sourceId ?? "local";
      groups.set(id, [...(groups.get(id) ?? []), server]);
    }
    return [...groups.entries()]
      .sort(([left], [right]) => {
        if (left === right) return 0;
        if (left === "local") return -1;
        if (right === "local") return 1;
        return this.sourceLabel(left).localeCompare(this.sourceLabel(right));
      })
      .map(([id, list]) => ({
        id,
        label: this.sourceLabel(id),
        readOnly: id !== "local",
        servers: list,
      }));
  }

  async connect(servers: Server[]): Promise<void> {
    if (servers.length === 0) {
      return;
    }
    const { sessions } = await import("./sessions.svelte");
    for (const server of servers) {
      if (
        [
          "rdp",
          "vnc",
          "ssh",
          "telnet",
          "serial",
          "sftp",
          "ftp",
          "localApp",
          "remoteApp",
          "custom",
        ].includes(server.protocol)
      ) {
        await sessions.connect(server);
      } else {
        app.showNotice(
          `${server.protocol} session support arrives in the protocol phase (Phase 5).`,
          "info",
        );
      }
    }
  }

  async connectVia(server: Server, alternateName: string): Promise<void> {
    const { sessions } = await import("./sessions.svelte");
    await sessions.connectVia(server, alternateName);
  }

  async openAdd(protocol = "rdp"): Promise<void> {
    try {
      const server = await commands.newServer(protocol);
      this.editor = {
        mode: "add",
        draft: server,
        servers: [],
        dirty: new SvelteSet(),
        original: null,
      };
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  openEdit(server: Server): void {
    const draft = cloneJson(server);
    this.editor = {
      mode: "edit",
      draft,
      servers: [server],
      dirty: new SvelteSet(),
      original: server,
    };
  }

  async openDuplicate(server: Server): Promise<void> {
    try {
      const template = await commands.newServer(server.protocol);
      const draft = { ...cloneJson(server), id: template.id, sourceId: "local" } as Server;
      draft.displayName = `${server.displayName} (copy)`;
      this.editor = {
        mode: "duplicate",
        draft,
        servers: [],
        dirty: new SvelteSet(),
        original: server,
      };
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  openBulk(servers: Server[]): void {
    const writable = servers.filter((server) => !this.isReadOnly(server));
    if (writable.length < 2) {
      if (writable.length !== servers.length) {
        app.showNotice("Read-only source servers cannot be bulk-edited.", "warning");
      }
      return;
    }
    this.editor = {
      mode: "bulk",
      draft: cloneJson(writable[0]),
      servers: writable,
      dirty: new SvelteSet(),
      original: null,
    };
  }

  updateDraft<K extends keyof Server>(field: K, value: Server[K]): void {
    if (!this.editor) {
      return;
    }
    this.editor.draft[field] = value;
    this.editor.dirty.add(String(field));
  }

  async changeEditorProtocol(protocol: string): Promise<void> {
    if (!this.editor || this.editor.mode === "edit" || this.editor.mode === "bulk") {
      return;
    }
    try {
      const template = await commands.newServer(protocol);
      const previous = this.editor.draft;
      const draft = {
        ...template,
        displayName: previous.displayName || template.displayName,
        colorHex: previous.colorHex,
        tags: previous.tags,
        treeNodes: previous.treeNodes,
        note: previous.note,
        iconBase64: previous.iconBase64,
        commandBeforeConnected: previous.commandBeforeConnected,
        commandAfterDisconnected: previous.commandAfterDisconnected,
        hideCommandBeforeConnectedWindow: previous.hideCommandBeforeConnectedWindow,
        selectedRunnerName: previous.selectedRunnerName,
        alwaysOpenInNewTabWindow: previous.alwaysOpenInNewTabWindow,
      } as Server;
      this.editor = {
        ...this.editor,
        draft,
        dirty: new SvelteSet(),
      };
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async saveInline(server: Server): Promise<boolean> {
    try {
      const saved = await commands.saveServer(server);
      this.snapshot = this.snapshot
        ? {
            ...this.snapshot,
            servers: this.snapshot.servers.map((candidate) =>
              candidate.id === saved.id ? saved : candidate,
            ),
          }
        : this.snapshot;
      await this.refresh();
      return true;
    } catch (reason) {
      await app.reportError(reason);
      return false;
    }
  }

  async moveServerOrder(serverId: string, delta: number): Promise<void> {
    const siblings = this.siblingServerIds(serverId);
    const siblingIndex = siblings.indexOf(serverId);
    const siblingTarget = siblingIndex + delta;
    if (
      siblingIndex < 0 ||
      siblingTarget < 0 ||
      siblingTarget >= siblings.length
    ) {
      return;
    }
    const current = this.visibleServerIds();
    const index = current.indexOf(serverId);
    const target = current.indexOf(siblings[siblingTarget]);
    if (index < 0 || target < 0) return;
    [current[index], current[target]] = [current[target], current[index]];
    try {
      this.listView = await commands.setCustomOrder(current);
      this.sort = "custom";
      await this.refresh();
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async moveFolderOrder(path: string[], delta: number): Promise<void> {
    const key = path.join("/");
    const siblings = this.siblingFolderPaths(path);
    const siblingIndex = siblings.indexOf(key);
    const siblingTarget = siblingIndex + delta;
    if (
      siblingIndex < 0 ||
      siblingTarget < 0 ||
      siblingTarget >= siblings.length
    ) {
      return;
    }
    const current = this.visibleFolderPaths();
    const index = current.indexOf(key);
    const target = current.indexOf(siblings[siblingTarget]);
    if (index < 0 || target < 0) return;
    [current[index], current[target]] = [current[target], current[index]];
    try {
      this.treeView = await commands.updateTreeView({
        schemaVersion: 1,
        expanded: [...this.expanded],
        nodeOrder: current,
      });
      await this.refresh();
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async setTagOrder(order: string[]): Promise<void> {    try {
      await commands.setTagOrder(order);
      await this.load();
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  closeEditor(): void {
    this.editor = null;
  }

  async saveEditor(): Promise<boolean> {
    if (!this.editor) {
      return false;
    }
    const editor = this.editor;
    try {
      if (editor.mode === "bulk") {
        const updated = editor.servers.map((server) => {
          const copy = cloneJson(server);
          for (const field of editor.dirty) {
            (copy as Record<string, unknown>)[field] = (
              editor.draft as unknown as Record<string, unknown>
            )[field];
          }
          return copy;
        });
        await commands.saveServers(updated);
      } else {
        await commands.saveServer(cloneJson(editor.draft));
      }
      this.editor = null;
      await this.load();
      return true;
    } catch (reason) {
      await app.reportError(reason);
      return false;
    }
  }

  async duplicateSelected(): Promise<void> {
    const selected = this.selectedServers();
    if (selected.length === 0) {
      return;
    }
    try {
      const copies: Server[] = [];
      for (const server of selected) {
        const template = await commands.newServer(server.protocol);
        const copy = { ...cloneJson(server), id: template.id, sourceId: "local" } as Server;
        copy.displayName = `${server.displayName} (copy)`;
        copies.push(copy);
      }
      await commands.saveServers(copies);
      this.clearSelection();
      await this.load();
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async deleteSelected(): Promise<void> {
    const selected = this.selectedServers();
    const deletable = selected.filter((server) => !this.isReadOnly(server));
    if (deletable.length === 0) {
      if (selected.length > 0) {
        app.showNotice("Servers from read-only sources cannot be deleted.", "warning");
      }
      return;
    }
    if (deletable.length !== selected.length) {
      app.showNotice(
        `${selected.length - deletable.length} read-only server(s) were skipped.`,
        "warning",
      );
    }
    const confirmed = await dialogConfirm(
      `Delete ${deletable.length} server${deletable.length === 1 ? "" : "s"}?`,
      "This cannot be undone.",
    );
    if (!confirmed) {
      return;
    }
    try {
      for (const server of deletable) {
        await commands.deleteServer(server.id);
      }
      this.clearSelection();
      await this.load();
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async exportSelected(): Promise<void> {
    const ids = [...this.selected];
    try {
      const content = await commands.exportServersJson(ids);
      downloadText(
        `hremote-export-${new Date().toISOString().slice(0, 10)}.json`,
        content,
      );
      app.showNotice(
        "Exported unencrypted JSON. Passwords will be readable in the file.",
        "warning",
      );
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async shortcutSelected(startMinimized: boolean): Promise<void> {
    const ids = [...this.selected];
    if (ids.length === 0) {
      return;
    }
    try {
      const name = ids.length === 1
        ? this.serverById(ids[0])?.displayName ?? "H-Remote session"
        : `H-Remote (${ids.length} sessions)`;
      const result = await commands.createShortcutForServers(
        ids,
        name,
        startMinimized,
      );
      app.showNotice(result.path, "info");
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async createTagShortcut(tag: string, startMinimized: boolean): Promise<void> {
    try {
      const result = await commands.createShortcutForTag(
        tag,
        `H-Remote ${tag}`,
        startMinimized,
      );
      app.showNotice(result.path, "info");
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async togglePinTag(tag: string): Promise<void> {
    const pinned = new Set(this.snapshot?.tags.filter((t) => t.isPinned).map((t) => t.name));
    if (pinned.has(tag)) pinned.delete(tag);
    else pinned.add(tag);
    try {
      await commands.setPinnedTags([...pinned]);
      await this.load();
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async renameTag(tag: string, newName: string): Promise<void> {
    const name = newName.trim().replace(/^#/, "").toLowerCase();
    if (!name || /\s/.test(name)) {
      app.showNotice("Tag names cannot contain spaces.", "warning");
      return;
    }
    const servers = this.snapshot?.servers ?? [];
    const changed = servers
      .filter((server) => server.tags.includes(tag))
      .map((server) => ({
        ...cloneJson(server),
        tags: [...new Set(server.tags.map((value) => (value === tag ? name : value)))].sort(),
      }));
    try {
      await commands.saveServers(changed);
      await this.load();
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async deleteTag(tag: string): Promise<void> {
    const changed = (this.snapshot?.servers ?? [])
      .filter((server) => server.tags.includes(tag))
      .map((server) => ({
        ...cloneJson(server),
        tags: server.tags.filter((value) => value !== tag),
      }));
    try {
      await commands.saveServers(changed);
      await this.load();
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async createFolder(path: string[]): Promise<void> {
    try {
      await commands.ensureFolder(path);
      await this.load();
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async renameFolder(folder: Folder, newName: string): Promise<void> {
    try {
      await commands.renameFolder(folder.id, newName);
      await this.load();
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async deleteFolder(folder: Folder): Promise<void> {
    const confirmed = await dialogConfirm(
      `Delete folder '${folder.path.join(" / ")}'?`,
      "Contained servers are moved to the parent folder.",
    );
    if (!confirmed) {
      return;
    }
    try {
      await commands.deleteFolder(folder.path);
      await this.load();
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async duplicateFolder(folder: Folder): Promise<void> {
    try {
      await commands.duplicateFolder(folder.id);
      await this.load();
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async moveFolder(folder: Folder, parent: string[]): Promise<void> {
    try {
      await commands.moveFolder(folder.id, parent);
      await this.load();
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async moveServer(server: Server, path: string[]): Promise<void> {
    try {
      await commands.moveServerToFolder(server.id, path);
      await this.load();
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async toggleTagFilter(tag: string): Promise<void> {
    const tokens = this.query.split(/\s+/).filter(Boolean);
    const token = `#${tag}`;
    this.setQuery(
      tokens.includes(token)
        ? tokens.filter((candidate) => candidate !== token).join(" ")
        : [...tokens, token].join(" "),
    );
  }

  activeTagFilters(): string[] {
    return this.query
      .split(/\s+/)
      .filter((token) => token.startsWith("#") || token.startsWith("+#"))
      .map((token) => token.replace(/^\+?#/, "").toLowerCase());
  }

  tags(): Tag[] {
    return this.snapshot?.tags ?? [];
  }

  dispose(): void {
    this.disposed = true;
    if (this.refreshTimer) {
      clearTimeout(this.refreshTimer);
      this.refreshTimer = undefined;
    }
  }
}

async function dialogConfirm(title: string, message: string): Promise<boolean> {
  const { dialogs } = await import("./dialogs.svelte");
  return dialogs.confirm({ title, message, danger: true });
}

function downloadText(fileName: string, content: string): void {
  const blob = new Blob([content], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = fileName;
  anchor.click();
  URL.revokeObjectURL(url);
}

export const catalog = new CatalogStore();

/** Clones reactive proxies into plain objects (safe for IPC/structured data). */
export function cloneJson<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}
