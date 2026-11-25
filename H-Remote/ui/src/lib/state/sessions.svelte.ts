import type {
  AvailabilityEntry,
  Locality,
  SessionEvent,
  SessionCursor,
  SessionSnapshot,
  Server,
} from "../ipc/contracts";
import { commands } from "../ipc/client";
import { CleanupRegistry, subscribe } from "../ipc/events";
import { isTauriRuntime } from "../ipc/runtime";
import { app } from "./app.svelte";

export type TabWorkspaceSettings = {
  layout: string;
  gridMode: boolean;
  showIcons: boolean;
  showClose: boolean;
  showReconnect: boolean;
};

const DEFAULT_WORKSPACE: TabWorkspaceSettings = {
  layout: "auto",
  gridMode: false,
  showIcons: true,
  showClose: true,
  showReconnect: true,
};

/**
 * Owns the session tab workspace: snapshots, tab order, grid layout and the
 * frame polling loop. Every connection is hosted in the single shared session
 * window; frames are fetched only for visible sessions (one in focus mode, all
 * tabs in grid mode).
 */
class SessionsStore {
  sessions = $state<SessionSnapshot[]>([]);
  activeId = $state<string | null>(null);
  frames = $state<Record<string, string>>({});
  cursors = $state<Record<string, SessionCursor>>({});
  visibleIds = $state<string[]>([]);
  focusedId = $state<string | null>(null);
  availability = $state<AvailabilityEntry[] | null>(null);
  layout = $state(DEFAULT_WORKSPACE.layout);
  gridMode = $state(DEFAULT_WORKSPACE.gridMode);
  showIcons = $state(DEFAULT_WORKSPACE.showIcons);
  showClose = $state(DEFAULT_WORKSPACE.showClose);
  showReconnect = $state(DEFAULT_WORKSPACE.showReconnect);
  busy = $state(false);
  displaySessionId = $state<string | null>(null);
  displayOverrides = $state<
    Record<string, { width: number; height: number; monitors: number }>
  >({});
  private fitSizes = new Map<string, { width: number; height: number }>();
  private cleanup = new CleanupRegistry();
  private poll: ReturnType<typeof setInterval> | undefined;
  private subscribed = false;
  private polling = new Set<string>();
  private frameLoops = new Set<string>();
  private queuedRefreshes = new Set<string>();
  private frameSeqs = new Map<string, number>();
  private availabilityResolve: ((proceed: boolean) => void) | null = null;
  private autoReconnects = new Map<string, number>();
  private reconnectTimers = new Map<string, ReturnType<typeof setTimeout>>();
  private reportedFailures = new Set<string>();

  get active(): SessionSnapshot | null {
    return this.sessions.find((session) => session.id === this.activeId) ?? null;
  }

  get focused(): SessionSnapshot | null {
    return this.sessions.find((session) => session.id === this.focusedId) ?? null;
  }

  get activeFrame(): string | null {
    return this.activeId ? (this.frames[this.activeId] ?? null) : null;
  }

  cursorFor(id: string | null | undefined): SessionCursor | undefined {
    return id ? this.cursors[id] : undefined;
  }

  get tabs(): SessionSnapshot[] {
    return this.sessions;
  }

  async load(): Promise<void> {
    this.startEvents();
    try {
      const [sessions, locality] = await Promise.all([
        commands.sessionList(),
        commands.getLocality(),
      ]);
      this.sessions = sessions.filter(
        (session, index, all) =>
          all.findIndex((candidate) => candidate.id === session.id) === index,
      );
      this.applyWorkspace(locality);
      if (!this.activeId && sessions.length > 0) {
        this.activeId = sessions[sessions.length - 1].id;
      }
      this.syncVisible();
      this.startPolling();
      this.updateWindowTitle();
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  startEvents(): void {
    if (!this.subscribed) {
      this.subscribed = true;
      subscribe<SessionEvent>(
        "session:event",
        (event) => this.handleEvent(event),
        this.cleanup,
      );
      subscribe<string>(
        "session:focus",
        (id) => {
          if (this.sessions.some((session) => session.id === id)) {
            this.focus(id);
          } else {
            void this.load().then(() => {
              if (this.sessions.some((session) => session.id === id)) {
                this.focus(id);
              }
            });
          }
        },
        this.cleanup,
      );
    }
  }

  handleEvent(event: SessionEvent): void {
    if (event.kind === "state") {
      const index = this.sessions.findIndex(
        (session) => session.id === event.session.id,
      );
      if (index >= 0) {
        this.sessions[index] = event.session;
      } else {
        this.sessions = [...this.sessions, event.session];
      }
      if (event.session.state === "connected") {
        this.autoReconnects.delete(event.session.id);
        this.startPolling();
      } else if (
        event.session.state === "disconnected" &&
        (event.session.reason === "networkLoss" || event.session.reason === "noInfo")
      ) {
        this.scheduleAutoReconnect(event.session.id);
      }
      if (event.session.state !== "connected") {
        // A new backend connection restarts frame sequence numbers and may not
        // emit a pointer shape until it changes; never carry stale state over.
        this.frameSeqs.delete(event.session.id);
        if (this.cursors[event.session.id]) {
          const next = { ...this.cursors };
          delete next[event.session.id];
          this.cursors = next;
        }
      }
      if (event.session.state === "disconnected" || event.session.state === "failed") {
        this.polling.delete(event.session.id);
        if (!this.sessions.some((session) => session.state === "connected")) {
          this.stopPolling();
        }
      }
      this.syncVisible();
      this.updateWindowTitle();
      if (event.session.state === "connected") {
        this.reportedFailures.delete(event.session.id);
      } else if (event.session.state === "failed") {
        void this.reportFailure(event.session);
      }
      return;
    }
    if (event.kind === "frame") {
      this.polling.add(event.sessionId);
      const index = this.sessions.findIndex(
        (session) => session.id === event.sessionId,
      );
      if (index >= 0 && event.width > 0 && event.height > 0) {
        const current = this.sessions[index];
        // Resolution changes are rare; avoiding a new snapshot object for every
        // frame keeps the session panes from re-rendering during mouse-heavy use.
        if (current.width !== event.width || current.height !== event.height) {
          this.sessions[index] = {
            ...current,
            width: event.width,
            height: event.height,
          };
        }
      }
      this.queueFrameRefresh(event.sessionId);
      return;
    }
    if (event.kind === "cursor") {
      this.cursors = { ...this.cursors, [event.sessionId]: event.cursor };
      return;
    }
    if (event.kind === "clipboard" && event.text) {
      void navigator.clipboard?.writeText(event.text).catch(() => undefined);
    }
  }

  async connect(
    server: Server,
    options?: {
      username?: string;
      password?: string;
      privateKey?: string;
      alternateName?: string;
      skipAvailability?: boolean;
    },
  ): Promise<void> {
    this.busy = true;
    if (!isTauriRuntime) {
      this.startEvents();
    }
    try {
      if (!options?.alternateName) {
        const existing = await this.findOpenSession(server.id);
        if (existing) {
          await this.hostSession(existing.id);
          if (!isTauriRuntime) {
            this.focus(existing.id);
          }
          if (existing.state === "failed" || existing.state === "disconnected") {
            void this.reconnect(existing.id);
          }
          return;
        }
      }
      let username = options?.username;
      let password = options?.password;
      let privateKey = options?.privateKey;
      if (server.askPasswordWhenConnect && !password) {
        const { dialogs } = await import("./dialogs.svelte");
        const enteredUser = await dialogs.input({
          title: `Connect to ${server.displayName}`,
          label: "Username",
          value: server.username ?? "",
          submitLabel: "Next",
        });
        if (enteredUser === null) {
          return;
        }
        const enteredPassword = await dialogs.password({
          title: `Connect to ${server.displayName}`,
          label: "Password (leave empty to use a private key)",
        });
        if (enteredPassword === null) {
          return;
        }
        username = enteredUser;
        password = enteredPassword || undefined;
        privateKey = enteredPassword ? undefined : (server.privateKey ?? undefined);
      }
      if (
        !options?.skipAvailability &&
        (server.isPingBeforeConnect || server.isAutoAlternateAddressSwitching)
      ) {
        const entries = await commands.checkServerAvailability(server.id);
        const reachable = entries.some((entry) => entry.reachable);
        if (!reachable) {
          this.availability = entries;
          const proceed = await new Promise<boolean>((resolve) => {
            this.availabilityResolve = resolve;
          });
          this.availability = null;
          this.availabilityResolve = null;
          if (!proceed) {
            return;
          }
        }
      }
      const viewport = this.viewport();
      const scale = Math.round(window.devicePixelRatio * 100);
      const snapshot = await commands.connectServer(server.id, viewport, scale, {
        username,
        password,
        privateKey,
        alternateName: options?.alternateName,
      });
      this.replace(snapshot);
      this.activeId = snapshot.id;
      this.focusedId = null;
      if (isTauriRuntime) {
        // The shared session window owns frames, tabs and the grid. The window
        // that initiated the connection (catalog or launcher) stays a catalog.
        await this.hostSession(snapshot.id);
      } else {
        this.syncVisible();
        this.startPolling();
        this.updateWindowTitle();
      }
    } catch (reason) {
      const message = reason instanceof Error ? reason.message : String(reason);
      try {
        const report = await commands.reportConnectionFailure(server.id, message);
        app.showNotice(
          `Connection failed. Diagnostic report saved: ${report.path}`,
          "warning",
        );
      } catch {
        await app.reportError(reason);
      }
    } finally {
      this.busy = false;
    }
  }

  private async findOpenSession(serverId: string): Promise<SessionSnapshot | undefined> {
    const local = this.sessions.find((session) => session.serverId === serverId);
    if (local) {
      return local;
    }
    try {
      const all = await commands.sessionList();
      const existing = all.find((session) => session.serverId === serverId);
      if (existing) {
        this.replace(existing);
      }
      return existing;
    } catch {
      return undefined;
    }
  }

  private async reportFailure(session: SessionSnapshot): Promise<void> {
    if (this.reportedFailures.has(session.id)) {
      return;
    }
    this.reportedFailures.add(session.id);
    try {
      const report = await commands.reportSessionFailure(session.id);
      app.showNotice(
        `${session.serverName} failed. Diagnostic report saved: ${report.path}`,
        "warning",
      );
    } catch (reason) {
      console.debug("session failure report unavailable", reason);
    }
  }

  async connectVia(server: Server, alternateName: string): Promise<void> {
    await this.connect(server, { alternateName });
  }

  resolveAvailability(proceed: boolean): void {
    this.availabilityResolve?.(proceed);
  }

  /** Shows (or focuses) the shared window that hosts every session tab. */
  async hostSession(id: string): Promise<void> {
    if (!isTauriRuntime) {
      if (this.sessions.some((session) => session.id === id)) {
        this.focus(id);
      }
      return;
    }
    try {
      await commands.openSessionWindow(id);
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  /** Resolves CLI/shortcut activation (`ULID:`, `#tag`, name) into sessions. */
  async connectActivation(action: { kind: string; value?: string }): Promise<void> {
    const value = action.value ?? "";
    try {
      const snapshot = await commands.catalogSnapshot();
      let matches: Server[] = [];
      switch (action.kind) {
        case "connectById":
          matches = snapshot.servers.filter((server) => server.id === value);
          break;
        case "connectByTag":
          matches = snapshot.servers.filter((server) => server.tags.includes(value));
          break;
        case "connectByName":
          matches = snapshot.servers.filter(
            (server) =>
              server.displayName.toLowerCase() === value.toLowerCase() ||
              server.displayName.toLowerCase().includes(value.toLowerCase()),
          );
          break;
        default:
          break;
      }
      if (matches.length === 0) {
        app.showNotice(`No server matched '${value}'.`, "warning");
        return;
      }
      for (const server of matches) {
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
          await this.connect(server);
        }
      }
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  select(id: string): void {
    this.activeId = id;
    this.focusedId = null;
    this.syncVisible();
    this.startPolling();
    this.updateWindowTitle();
  }

  focus(id: string): void {
    this.activeId = id;
    this.focusedId = id;
    this.syncVisible();
    this.startPolling();
    this.updateWindowTitle();
  }

  leaveFocus(): void {
    this.focusedId = null;
    this.syncVisible();
  }

  cycle(delta: number): void {
    if (this.sessions.length < 2) {
      return;
    }
    const index = this.sessions.findIndex((session) => session.id === this.activeId);
    const next =
      (index + delta + this.sessions.length) % this.sessions.length;
    this.select(this.sessions[next].id);
  }

  selectIndex(index: number): void {
    const session = this.sessions[index];
    if (session) {
      this.select(session.id);
    }
  }

  async cancel(id = this.activeId): Promise<void> {
    if (!id) {
      return;
    }
    try {
      await commands.sessionCancel(id);
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  /** Retries a session that dropped unexpectedly (used on window resize/refit). */
  reconnectIfLost(id: string): void {
    const session = this.sessions.find((candidate) => candidate.id === id);
    if (!session || session.state !== "disconnected") {
      return;
    }
    if (session.reason === "networkLoss" || session.reason === "noInfo") {
      this.scheduleAutoReconnect(id);
    }
  }

  private scheduleAutoReconnect(id: string): void {
    const attempts = this.autoReconnects.get(id) ?? 0;
    if (attempts >= 2 || this.reconnectTimers.has(id)) {
      return;
    }
    this.reconnectTimers.set(
      id,
      setTimeout(() => {
        this.reconnectTimers.delete(id);
        this.autoReconnects.set(id, attempts + 1);
        app.showNotice("Connection lost — reconnecting automatically…", "warning");
        void this.reconnect(id);
      }, 1500),
    );
  }

  async reconnect(id = this.activeId): Promise<void> {
    if (!id) {
      return;
    }
    try {
      const snapshot = await commands.sessionReconnect(id);
      this.replace(snapshot);
      this.frames = { ...this.frames, [id]: "" };
      const override = this.displayOverrides[id];
      if (override) {
        await this.resize(id, override.width * override.monitors, override.height);
      }
      this.syncVisible();
      this.startPolling();
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async close(id = this.activeId): Promise<void> {
    if (!id) {
      return;
    }
    try {
      await this.cancel(id);
      await commands.sessionRemove(id);
      this.removeLocal(id);
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async closeOthers(keep = this.activeId): Promise<void> {
    const targets = this.sessions
      .filter((session) => session.id !== keep)
      .map((session) => session.id);
    for (const id of targets) {
      await this.close(id);
    }
  }

  async closeAll(): Promise<void> {
    for (const session of [...this.sessions]) {
      await this.close(session.id);
    }
  }

  reorder(fromId: string, toIndex: number): void {
    const from = this.sessions.findIndex((session) => session.id === fromId);
    if (from < 0) {
      return;
    }
    const next = [...this.sessions];
    const [moved] = next.splice(from, 1);
    next.splice(Math.max(0, Math.min(toIndex, next.length)), 0, moved);
    this.sessions = next;
  }

  async setLayout(layout: string): Promise<void> {
    this.layout = layout;
    await this.persistWorkspace();
  }

  async setGridMode(enabled: boolean): Promise<void> {
    this.gridMode = enabled && this.sessions.length >= 2;
    this.syncVisible();
    await this.persistWorkspace();
  }

  async setTabButtons(patch: Partial<Pick<TabWorkspaceSettings, "showIcons" | "showClose" | "showReconnect">>): Promise<void> {
    if (patch.showIcons !== undefined) this.showIcons = patch.showIcons;
    if (patch.showClose !== undefined) this.showClose = patch.showClose;
    if (patch.showReconnect !== undefined) this.showReconnect = patch.showReconnect;
    await this.persistWorkspace();
  }

  async sendCtrlAltDel(id = this.activeId): Promise<void> {
    if (!id) {
      return;
    }
    try {
      await commands.sessionCtrlAltDel(id);
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async sendSpecial(id: string, special: "ctrlAltDel" | "ctrlEsc" | "altF4"): Promise<void> {
    try {
      await commands.sessionSendSpecial(id, special);
    } catch (reason) {
      console.debug("session special key dropped", reason);
    }
  }

  async vncKey(id: string, keycode: number, down: boolean): Promise<void> {
    try {
      await commands.sessionVncKey(id, keycode, down);
    } catch (reason) {
      console.debug("vnc key dropped", reason);
    }
  }

  async vncPointer(id: string, x: number, y: number, buttons: number): Promise<void> {
    try {
      await commands.sessionVncPointer(id, x, y, buttons);
    } catch (reason) {
      console.debug("vnc pointer dropped", reason);
    }
  }

  async rdpKey(id: string, code: string, key: string, down: boolean): Promise<void> {
    try {
      await commands.sessionRdpKey(id, code, key, down);
    } catch (reason) {
      console.debug("rdp key dropped", reason);
    }
  }

  async rdpPointerMove(id: string, x: number, y: number): Promise<void> {
    try {
      await commands.sessionRdpPointerMove(id, x, y);
    } catch (reason) {
      console.debug("rdp pointer move dropped", reason);
    }
  }

  async rdpPointerButton(
    id: string,
    x: number,
    y: number,
    button: number,
    down: boolean,
  ): Promise<void> {
    try {
      await commands.sessionRdpPointerButton(id, x, y, button, down);
    } catch (reason) {
      console.debug("rdp pointer button dropped", reason);
    }
  }

  async rdpWheel(
    id: string,
    x: number,
    y: number,
    vertical: boolean,
    units: number,
  ): Promise<void> {
    try {
      await commands.sessionRdpWheel(id, x, y, vertical, units);
    } catch (reason) {
      console.debug("rdp wheel dropped", reason);
    }
  }

  openDisplay(id: string | null = this.activeId): void {
    this.displaySessionId = id;
  }

  closeDisplay(): void {
    this.displaySessionId = null;
  }

  /** Records the last fit-to-window size so "Fit" can be restored. */
  recordFitSize(id: string, width: number, height: number): void {
    if (width > 0 && height > 0) {
      this.fitSizes.set(id, { width, height });
    }
  }

  hasDisplayOverride(id: string): boolean {
    return Boolean(this.displayOverrides[id]);
  }

  /** Applies a fixed resolution and optional virtual-monitor multiplier. */
  async applyDisplay(
    id: string,
    width: number,
    height: number,
    monitors: number,
  ): Promise<void> {
    const safeWidth = Math.min(3840, Math.max(640, Math.round(width)));
    const safeHeight = Math.min(2160, Math.max(480, Math.round(height)));
    const safeMonitors = Math.min(4, Math.max(1, Math.round(monitors)));
    this.displayOverrides = {
      ...this.displayOverrides,
      [id]: { width: safeWidth, height: safeHeight, monitors: safeMonitors },
    };
    await this.resize(id, safeWidth * safeMonitors, safeHeight);
    app.showNotice(
      safeMonitors > 1
        ? `Display set to ${safeWidth}×${safeHeight} · ${safeMonitors} virtual monitors (${safeWidth * safeMonitors}×${safeHeight} canvas).`
        : `Display set to ${safeWidth}×${safeHeight}.`,
      "info",
    );
  }

  /** Returns to fit-to-window auto resizing. */
  async resetDisplay(id: string): Promise<void> {
    const next = { ...this.displayOverrides };
    delete next[id];
    this.displayOverrides = next;
    const fit = this.fitSizes.get(id);
    if (fit) {
      await this.resize(id, fit.width, fit.height);
    }
  }

  async resize(id: string, width: number, height: number): Promise<void> {
    try {
      await commands.sessionResize(
        id,
        Math.max(640, Math.round(width)),
        Math.max(480, Math.round(height)),
        Math.round(window.devicePixelRatio * 100),
      );
    } catch (reason) {
      console.debug("session resize dropped", reason);
    }
  }

  /**
   * Requests a frame fetch without allowing requests to pile up. Frame events
   * can arrive faster than a PNG encode round-trip; at most one fetch per
   * session is in flight and bursts collapse into the next fetch.
   */
  queueFrameRefresh(id: string): void {
    if (!this.isVisible(id)) {
      return;
    }
    this.queuedRefreshes.add(id);
    if (this.frameLoops.has(id)) {
      return;
    }
    this.frameLoops.add(id);
    void (async () => {
      try {
        while (this.queuedRefreshes.delete(id)) {
          if (!this.isVisible(id)) {
            break;
          }
          await this.refreshFrame(id);
        }
      } finally {
        this.frameLoops.delete(id);
        if (this.queuedRefreshes.has(id)) {
          this.queueFrameRefresh(id);
        }
      }
    })();
  }

  /** Fetches the latest frame; exported for tests and the poll loop. */
  async refreshFrame(id = this.activeId): Promise<void> {
    if (!id || !this.isVisible(id)) {
      return;
    }
    try {
      const image = await commands.sessionFrameImage(
        id,
        this.frameSeqs.get(id) ?? null,
      );
      // `null` means either no frame exists yet or the desktop has not changed
      // since the last sequence. Keeping the previous data URL is essential:
      // clearing it unmounts the canvas and caused periodic/enter-exit flashes.
      if (image && this.frameSeqs.get(id) !== image.seq) {
        this.frameSeqs.set(id, image.seq);
        if (this.frames[id] !== image.dataUrl) {
          this.frames = { ...this.frames, [id]: image.dataUrl };
        }
      }
    } catch {
      // A missing/being-replaced ring is normal during reconnects. Keep the last
      // good frame instead of blanking the pane.
    }
  }

  startPolling(): void {
    this.syncVisible();
    if (this.poll) {
      return;
    }
    void this.refreshVisible();
    // Frames are event-driven; this is only a safety net for dropped events.
    this.poll = setInterval(() => void this.refreshVisible(), 250);
  }

  stopPolling(): void {
    if (this.poll) {
      clearInterval(this.poll);
      this.poll = undefined;
    }
  }

  dispose(): void {
    this.stopPolling();
    this.frameLoops.clear();
    this.queuedRefreshes.clear();
    this.frameSeqs.clear();
    for (const timer of this.reconnectTimers.values()) {
      clearTimeout(timer);
    }
    this.reconnectTimers.clear();
    this.cleanup.dispose();
    this.subscribed = false;
  }

  resetForTests(): void {
    this.dispose();
    this.cleanup = new CleanupRegistry();
    this.sessions = [];
    this.activeId = null;
    this.frames = {};
    this.cursors = {};
    this.visibleIds = [];
    this.focusedId = null;
    this.availability = null;
    this.availabilityResolve = null;
    this.displaySessionId = null;
    this.displayOverrides = {};
    this.fitSizes.clear();
    this.autoReconnects.clear();
    this.reportedFailures.clear();
    for (const timer of this.reconnectTimers.values()) {
      clearTimeout(timer);
    }
    this.reconnectTimers.clear();
    this.layout = DEFAULT_WORKSPACE.layout;
    this.gridMode = DEFAULT_WORKSPACE.gridMode;
    this.showIcons = DEFAULT_WORKSPACE.showIcons;
    this.showClose = DEFAULT_WORKSPACE.showClose;
    this.showReconnect = DEFAULT_WORKSPACE.showReconnect;
    this.busy = false;
    this.polling.clear();
    this.frameLoops.clear();
    this.queuedRefreshes.clear();
    this.frameSeqs.clear();
  }

  private async refreshVisible(): Promise<void> {
    const ids = this.visibleIds.filter((id) => {
      const session = this.sessions.find((candidate) => candidate.id === id);
      return session?.state === "connected" && session.backend !== "fileTransfer";
    });
    for (const id of ids) {
      this.queueFrameRefresh(id);
    }
  }

  private isVisible(id: string): boolean {
    return this.visibleIds.includes(id);
  }

  private syncVisible(): void {
    if (this.focusedId) {
      this.visibleIds = [this.focusedId];
      return;
    }
    if (this.gridMode && this.sessions.length >= 2) {
      this.visibleIds = this.sessions.map((session) => session.id);
      return;
    }
    this.visibleIds = this.activeId ? [this.activeId] : [];
  }

  private removeLocal(id: string): void {
    this.sessions = this.sessions.filter((session) => session.id !== id);
    this.fitSizes.delete(id);
    this.autoReconnects.delete(id);
    this.reportedFailures.delete(id);
    const reconnect = this.reconnectTimers.get(id);
    if (reconnect) {
      clearTimeout(reconnect);
      this.reconnectTimers.delete(id);
    }
    if (this.displayOverrides[id]) {
      const next = { ...this.displayOverrides };
      delete next[id];
      this.displayOverrides = next;
    }
    if (this.displaySessionId === id) {
      this.displaySessionId = null;
    }
    const frames = { ...this.frames };
    delete frames[id];
    this.frames = frames;
    if (this.cursors[id]) {
      const cursors = { ...this.cursors };
      delete cursors[id];
      this.cursors = cursors;
    }
    this.frameSeqs.delete(id);
    this.polling.delete(id);
    this.queuedRefreshes.delete(id);
    this.frameLoops.delete(id);
    if (this.activeId === id) {
      this.activeId = this.sessions[0]?.id ?? null;
    }
    if (this.focusedId === id) {
      this.focusedId = null;
    }
    this.syncVisible();
    this.updateWindowTitle();
  }

  private replace(snapshot: SessionSnapshot): void {
    const index = this.sessions.findIndex((session) => session.id === snapshot.id);
    if (index >= 0) {
      this.sessions[index] = snapshot;
    } else {
      this.sessions = [...this.sessions, snapshot];
    }
  }

  private applyWorkspace(locality: Locality): void {
    this.layout = locality.ui.tabLayout || "auto";
    this.gridMode = locality.ui.gridMode;
    this.showIcons = locality.ui.showTabIcons;
    this.showClose = locality.ui.showTabClose;
    this.showReconnect = locality.ui.showTabReconnect;
  }

  private async persistWorkspace(): Promise<void> {
    try {
      await commands.updateTabWorkspace({
        layout: this.layout,
        gridMode: this.gridMode,
        showIcons: this.showIcons,
        showClose: this.showClose,
        showReconnect: this.showReconnect,
      });
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  private updateWindowTitle(): void {
    const title = this.active ? `${this.active.serverName} — H-Remote` : "H-Remote";
    void commands.setWindowTitle(title).catch(() => undefined);
  }

  private viewport(): [number, number] {
    // The workspace bar plus the content padding/border leave roughly this much
    // room; overshooting used to force an avoidable resize right after connect.
    const width = Math.max(640, (window.innerWidth || 1280) - 16);
    const height = Math.max(480, (window.innerHeight || 720) - 64);
    return [Math.round(width), Math.round(height)];
  }
}

export const sessions = new SessionsStore();
