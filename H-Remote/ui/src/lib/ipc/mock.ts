// In-memory IPC backend used by Vitest, Playwright and `npm run dev` in a plain
// browser. It mirrors the Rust DTOs closely enough to exercise the real UI.

import type {
  BootstrapDto,
  Capabilities,
  CatalogSnapshot,
  CatalogTree,
  CatalogView,
  CliInvocationSummary,
  Configuration,
  ConnectionRecords,
  CrashReport,
  CrashReportInfo,
  Credential,
  DataSourceConfig,
  DatabaseInfo,
  Folder,
  GuidanceResult,
  GuidanceSelection,
  ImportOutcome,
  ListViewLocality,
  LocaleInfo,
  Locality,
  ProfileMode,
  ProtocolDescriptor,
  RecentSessionInfo,
  ResolvedTheme,
  Runner,
  RunnersConfig,
  Server,
  SessionBackend,
  SessionEvent,
  SessionSnapshot,
  ShortcutOutcome,
  SourceState,
  SourceStatus,
  Tag,
  TextRange,
  ThemeColors,
  ThemeConfig,
  ThemePreset,
  ThemeValidation,
  TreeViewLocality,
  TrimOutcome,
  VaultStatus,
} from "./contracts";

const STORAGE_KEY = "hremote.mock.config.v1";
const CATALOG_KEY = "hremote.mock.catalog.v1";
const LIST_VIEW_KEY = "hremote.mock.listview.v1";
const TREE_VIEW_KEY = "hremote.mock.treeview.v1";
const TAB_WORKSPACE_KEY = "hremote.mock.tabworkspace.v1";
const RECORDS_KEY = "hremote.mock.records.v1";
const LOCALE_KEY = "hremote.mock.locale.v1";

const darkColors: ThemeColors = {
  primaryMid: "#323233",
  primaryLight: "#474748",
  primaryDark: "#2d2d2d",
  primaryText: "#cccccc",
  accentMid: "#FF007ACC",
  accentLight: "#FF32A7F4",
  accentDark: "#FF0061A3",
  accentText: "#FFFFFFFF",
  background: "#1e1e1e",
  backgroundText: "#cccccc",
};

const lightColors: ThemeColors = {
  primaryMid: "#FFF2F3F5",
  primaryLight: "#FFFFFFFF",
  primaryDark: "#FFE4E7EB",
  primaryText: "#FF232323",
  accentMid: "#FFE83D61",
  accentLight: "#FFED6884",
  accentDark: "#FFB5304C",
  accentText: "#FFFFFFFF",
  background: "#FFFFFFFF",
  backgroundText: "#000000",
};

export const mockPresets: ThemePreset[] = [
  { name: "Light", displayName: "Light", colors: lightColors },
  { name: "Dark", displayName: "Dark", colors: darkColors },
  {
    name: "PRemoteM",
    displayName: "PRemoteM",
    colors: {
      ...darkColors,
      primaryMid: "#102b3e",
      primaryLight: "#445a68",
      primaryDark: "#0c2230",
      primaryText: "#FFFFFFFF",
      background: "#ced8e1",
      backgroundText: "#000000",
    },
  },
  {
    name: "SecretKey",
    displayName: "SecretKey",
    colors: {
      ...darkColors,
      primaryMid: "#FF473368",
      primaryLight: "#796090",
      primaryDark: "#382853",
      primaryText: "#FFFFFFFF",
      accentMid: "#FFEF6D3B",
      background: "#FFF2F1EC",
      backgroundText: "#000000",
    },
  },
  {
    name: "Greystone",
    displayName: "Greystone",
    colors: {
      ...lightColors,
      primaryMid: "#FFC7D0D5",
      primaryLight: "#F9FDFD",
      primaryDark: "#9FA6AA",
      primaryText: "#FF1B2C3F",
      accentMid: "#FFFF7247",
      background: "#FFF5F5F5",
    },
  },
  {
    name: "Asphalt",
    displayName: "Asphalt",
    colors: {
      ...darkColors,
      primaryMid: "#FF393939",
      primaryLight: "#6B6661",
      primaryDark: "#2D2D2D",
      background: "#FFF5F5F5",
      backgroundText: "#000000",
    },
  },
  {
    name: "Wine",
    displayName: "Wine",
    colors: {
      ...darkColors,
      primaryMid: "#FF57112D",
      primaryLight: "#893E55",
      primaryDark: "#450D24",
      accentMid: "#FFA82159",
      background: "#FFFDEAD9",
      backgroundText: "#FF450D24",
    },
  },
  {
    name: "Forest",
    displayName: "Forest",
    colors: {
      ...darkColors,
      primaryMid: "#FF253938",
      primaryLight: "#576660",
      primaryDark: "#1D2D2C",
      accentMid: "#FF5FA291",
      background: "#FFF5F5F5",
      backgroundText: "#FF303030",
    },
  },
  {
    name: "Soil",
    displayName: "Soil",
    colors: {
      ...darkColors,
      primaryMid: "#FF776245",
      primaryLight: "#A98F6D",
      primaryDark: "#FF735E41",
      accentMid: "#FF0193B8",
      background: "#FFCFC3B5",
      backgroundText: "#FF080000",
    },
  },
  {
    name: "HighContrast",
    displayName: "High Contrast",
    colors: {
      ...darkColors,
      primaryMid: "#FF000000",
      primaryLight: "#FF1A1A1A",
      primaryDark: "#FF000000",
      primaryText: "#FFFFFFFF",
      accentMid: "#FFFFD400",
      accentLight: "#FFFFE766",
      accentDark: "#FFB89A00",
      accentText: "#FF000000",
      background: "#FF000000",
      backgroundText: "#FFFFFFFF",
    },
  },
];

const locales: LocaleInfo[] = [
  { id: "en-US", englishName: "English (United States)", nativeName: "English", complete: true },
  { id: "zh-CN", englishName: "Chinese (Simplified)", nativeName: "简体中文", complete: false },
  { id: "zh-TW", englishName: "Chinese (Traditional)", nativeName: "繁體中文", complete: false },
  { id: "ja-JP", englishName: "Japanese", nativeName: "日本語", complete: false },
  { id: "ru-RU", englishName: "Russian", nativeName: "Русский", complete: false },
  { id: "de-DE", englishName: "German", nativeName: "Deutsch", complete: false },
  { id: "fr-FR", englishName: "French", nativeName: "Français", complete: false },
  { id: "it-IT", englishName: "Italian", nativeName: "Italiano", complete: false },
  { id: "es-AR", englishName: "Spanish (Argentina)", nativeName: "Español", complete: false },
  { id: "gl-ES", englishName: "Galician", nativeName: "Galego", complete: false },
  { id: "pl-PL", englishName: "Polish", nativeName: "Polski", complete: false },
  { id: "pt-BR", englishName: "Portuguese (Brazil)", nativeName: "Português", complete: false },
  { id: "pt-PT", englishName: "Portuguese (Portugal)", nativeName: "Português", complete: false },
  { id: "cs-CZ", englishName: "Czech", nativeName: "Čeština", complete: false },
];

interface MockCatalog {
  servers: Server[];
  folders: Folder[];
  credentials: Credential[];
}

function baseServer(
  id: string,
  protocol: Server["protocol"],
  displayName: string,
  subTitle: string,
  tags: string[],
  treeNodes: string[],
): Server {
  return {
    protocol,
    id,
    classVersion: 1,
    displayName,
    subTitle,
    iconBase64: null,
    colorHex: "",
    tags,
    treeNodes,
    note: "",
    commandBeforeConnected: "",
    hideCommandBeforeConnectedWindow: false,
    commandAfterDisconnected: "",
    selectedRunnerName: null,
    alwaysOpenInNewTabWindow: false,
    sessionId: null,
    sourceId: "local",
  };
}

export function defaultCatalog(): MockCatalog {
  const rdp: Server = {
    ...baseServer("mock-rdp-1", "rdp", "Production RDP", "10.0.0.5:3389 (admin)", ["prod", "windows"], ["Prod"]),
    address: "10.0.0.5",
    port: 3389,
    username: "admin",
    password: "not-returned-in-production",
    domain: "CORP",
    gatewayMode: "doNotUseGateway",
    rdpWidth: 1920,
    rdpHeight: 1080,
    rdpWindowResizeMode: "autoResize",
    displayPerformance: "auto",
    enableClipboard: true,
    enableDiskDrives: true,
    enablePrinters: true,
    audioRedirectionMode: "redirectToLocal",
    note: "Primary Windows jump host.",
  };
  const staging: Server = {
    ...baseServer("mock-rdp-2", "rdp", "Staging RDP", "10.0.0.6:3389 (admin)", ["prod", "staging", "windows"], ["Prod"]),
    address: "10.0.0.6",
    port: 3389,
    username: "admin",
    password: "mock",
    domain: "CORP",
    gatewayMode: "doNotUseGateway",
    rdpWidth: 1280,
    rdpHeight: 720,
    rdpWindowResizeMode: "autoResize",
    displayPerformance: "auto",
    enableClipboard: true,
    note: "Staging jump host.",
  };
  const ssh: Server = {
    ...baseServer("mock-ssh-1", "ssh", "Build Server", "10.0.0.20:22 (root)", ["prod", "linux"], ["Prod", "CI"]),
    address: "10.0.0.20",
    port: 22,
    username: "root",
    password: "mock",
    sshVersion: "SSH-2",
    openSftpOnConnected: false,
    note: "CI runners live here.",
  };
  const vnc: Server = {
    ...baseServer("mock-vnc-1", "vnc", "Lab VNC", "192.168.1.10:5900", ["lab"], ["Lab"]),
    address: "192.168.1.10",
    port: 5900,
    password: "mock",
    vncWindowResizeMode: "stretch",
  };
  const sftp: Server = {
    ...baseServer("mock-sftp-1", "sftp", "File Server", "files.example.com:22 (deploy)", ["files"], ["Storage"]),
    address: "files.example.com",
    port: 22,
    username: "deploy",
    password: "mock",
    startupPath: "/srv/data",
  };
  const local: Server = {
    ...baseServer("mock-local-1", "localApp", "Chrome", "C:\\Program Files\\Google\\Chrome\\chrome.exe", ["local"], ["Tools"]),
    exePath: "C:\\Program Files\\Google\\Chrome\\chrome.exe",
    appProtocolDisplayName: "",
    argumentList: [],
    runWithHosting: true,
    arguments: "",
  };
  const ftp: Server = {
    ...baseServer("mock-ftp-1", "ftp", "FTP Drop", "ftp.example.com:21 (anonymous)", ["files"], []),
    address: "ftp.example.com",
    port: 21,
    username: "anonymous",
    password: "mock",
    startupPath: "/",
  };
  return {
    servers: [rdp, staging, ssh, vnc, sftp, local, ftp],
    folders: [
      { id: "folder-prod", path: ["Prod"] },
      { id: "folder-ci", path: ["Prod", "CI"] },
      { id: "folder-lab", path: ["Lab"] },
      { id: "folder-storage", path: ["Storage"] },
      { id: "folder-tools", path: ["Tools"] },
    ],
    credentials: [
      {
        id: "cred-1",
        name: "Prod admin",
        address: null,
        port: null,
        username: "admin",
        password: "mock",
        privateKeyPath: null,
        databaseId: null,
        hash: "mockhash",
        isEditable: true,
      },
    ],
  };
}

function loadCatalog(): MockCatalog {
  try {
    const raw = localStorage.getItem(CATALOG_KEY);
    if (!raw) {
      return defaultCatalog();
    }
    const parsed = JSON.parse(raw) as MockCatalog;
    if (!Array.isArray(parsed.servers)) {
      return defaultCatalog();
    }
    return parsed;
  } catch {
    return defaultCatalog();
  }
}

function saveCatalog(): void {
  localStorage.setItem(CATALOG_KEY, JSON.stringify(catalog));
}

function defaultListView(): ListViewLocality {
  return {
    schemaVersion: 1,
    sort: "insertion",
    groupExpanded: {
      Prod: true,
      "Prod/CI": true,
      Lab: true,
      Storage: true,
      Tools: true,
    },
    customOrder: [],
    columnWidths: {},
  };
}

function defaultTreeView(): TreeViewLocality {
  return {
    schemaVersion: 1,
    expanded: ["Prod", "Prod/CI", "Lab", "Storage", "Tools"],
    nodeOrder: [],
  };
}

function loadListView(): ListViewLocality {
  try {
    const raw = localStorage.getItem(LIST_VIEW_KEY);
    return raw ? { ...defaultListView(), ...JSON.parse(raw) } : defaultListView();
  } catch {
    return defaultListView();
  }
}

function loadTreeView(): TreeViewLocality {
  try {
    const raw = localStorage.getItem(TREE_VIEW_KEY);
    return raw ? { ...defaultTreeView(), ...JSON.parse(raw) } : defaultTreeView();
  } catch {
    return defaultTreeView();
  }
}

function saveListTree(): void {
  localStorage.setItem(LIST_VIEW_KEY, JSON.stringify(listView));
  localStorage.setItem(TREE_VIEW_KEY, JSON.stringify(treeView));
}

interface TabWorkspaceState {
  layout: string;
  gridMode: boolean;
  showIcons: boolean;
  showClose: boolean;
  showReconnect: boolean;
}

function defaultTabWorkspace(): TabWorkspaceState {
  return {
    layout: "auto",
    gridMode: false,
    showIcons: true,
    showClose: true,
    showReconnect: true,
  };
}

function loadTabWorkspace(): TabWorkspaceState {
  try {
    const raw = localStorage.getItem(TAB_WORKSPACE_KEY);
    return raw ? { ...defaultTabWorkspace(), ...JSON.parse(raw) } : defaultTabWorkspace();
  } catch {
    return defaultTabWorkspace();
  }
}

function saveTabWorkspace(): void {
  localStorage.setItem(TAB_WORKSPACE_KEY, JSON.stringify(tabWorkspace));
}

function loadRecords(): ConnectionRecords {
  try {
    const raw = localStorage.getItem(RECORDS_KEY);
    if (raw) {
      return { schemaVersion: 1, records: {}, quickConnect: [], ...JSON.parse(raw) };
    }
  } catch {
    // fall through
  }
  return { schemaVersion: 1, records: {}, quickConnect: [] };
}

function saveRecords(): void {
  localStorage.setItem(RECORDS_KEY, JSON.stringify(records));
}

function recordMockConnection(serverId: string): void {
  const now = Math.floor(Date.now() / 1000);
  const existing = records.records[serverId] ?? {
    lastConnectedAt: 0,
    connectCount: 0,
  };
  records.records[serverId] = {
    lastConnectedAt: now,
    connectCount: existing.connectCount + 1,
  };
  saveRecords();
}

function mockRecentIds(limit: number): string[] {
  return Object.entries(records.records)
    .sort((left, right) => right[1].lastConnectedAt - left[1].lastConnectedAt)
    .slice(0, limit)
    .map(([id]) => id);
}

function mockLocality(): Locality {  return {
    schemaVersion: 1,
    ui: {
      serverView: "list",
      serverSort: listView.sort,
      columnWidths: listView.columnWidths,
      mainWindow: null,
      launcherWindow: null,
      preferLastDisplay: false,
      lastActiveTab: null,
      tabLayout: tabWorkspace.layout,
      gridMode: tabWorkspace.gridMode,
      showTabIcons: tabWorkspace.showIcons,
      showTabClose: tabWorkspace.showClose,
      showTabReconnect: tabWorkspace.showReconnect,
    },
    session: { fullscreenMonitors: {}, lastTabTokens: {}, sessionWindows: {} },
  };
}

export function defaultThemeConfig(): ThemeConfig {
  return {
    themeName: "Dark",
    isCustom: false,
    colors: { ...darkColors },
    fontFamily: "system-ui",
    fontSize: 12,
  };
}

export function defaultRunners(): RunnersConfig {
  const runner = (
    id: string,
    name: string,
    protocol: string,
    args: string,
  ): Runner => ({
    id,
    name,
    protocol,
    kind: "external",
    exePath: "",
    arguments: args,
    parameters: {},
    environment: {},
    runWithHosting: true,
    sessionConfigPath: null,
    installState: "missing",
    macros: [],
  });
  return {
    runners: [
      runner("putty-ssh", "PuTTY (SSH)", "SSH", "-ssh {user}@{host} -P {port} -pw {password}"),
      runner("kitty-ssh", "KiTTY (SSH)", "SSH", "-ssh {user}@{host} -P {port} -pw {password}"),
      runner("putty-telnet", "PuTTY (Telnet)", "Telnet", "-telnet {host} -P {port}"),
      runner(
        "putty-serial",
        "PuTTY (Serial)",
        "Serial",
        "-serial {serialPort} -sercfg {baud},{dataBits},{parity},{stopBits},{flow}",
      ),
    ],
    defaults: {
      SSH: "putty-ssh",
      Telnet: "putty-telnet",
      Serial: "putty-serial",
    },
  };
}

export function defaultConfiguration(): Configuration {  return {
    schemaVersion: 1,
    general: {
      language: "system",
      runAtStartup: false,
      confirmBeforeClosingSession: false,
      showSessionIconInTitle: true,
      tabHeader: {
        showIcon: true,
        showCloseButton: true,
        showReconnectButton: false,
        closeButtonOnLeft: false,
      },
      showNoteInList: true,
      logLevel: "warn",
      copyPortWithAddress: true,
      moveFocusToLocalOnMouseLeave: false,
      doNotCheckForUpdates: false,
      includePrereleaseUpdates: false,
      updateCheckPeriodHours: 24,
      updateFeedUrl: null,
      startMinimized: false,
      guidanceCompleted: false,
      skipLegacyMigration: false,
    },
    launcher: {
      enabled: true,
      hotkey: "Alt+M",
      showNote: true,
      showCredentials: false,
      saveEnteredInfo: false,
      matchCredential: true,
      recentLimit: 20,
      rememberPosition: true,
    },
    runners: defaultRunners(),
    keywordMatch: {
      matchers: ["direct"],
      debounceMs: 150,
      parallel: true,
      autoLocale: true,
    },
    theme: defaultThemeConfig(),
    engagement: {
      launches: 0,
      firstRunAt: null,
      lastUpdateCheckAt: null,
      lastRatingPromptAt: null,
      doNotShowRating: false,
    },
    databaseCheckPeriod: 10,
    databaseReconnectPeriod: 120,
    sqliteDatabasePath: null,
    pinnedTags: [],
  };
}

let config: Configuration = loadConfig();
let currentLocale = localStorage.getItem(LOCALE_KEY) ?? "en-US";
let catalog: MockCatalog = loadCatalog();
let mockSources: DataSourceConfig[] = [];
let listView: ListViewLocality = loadListView();
let treeView: TreeViewLocality = loadTreeView();
let tabWorkspace = loadTabWorkspace();
let records = loadRecords();
let mockSessions: SessionSnapshot[] = [];
let mockSessionSeq = 0;
const mockFileSessions = new Map<string, { path: string; home: string }>();
const MOCK_FILE_ENTRIES = [
  { name: "docs", isDir: true, size: 0 },
  { name: "readme.txt", isDir: false, size: 1240 },
  { name: "logo.png", isDir: false, size: 8421 },
  { name: "archive.zip", isDir: false, size: 204800 },
];
const mockSessionTimers = new Set<ReturnType<typeof setTimeout>>();

function later(action: () => void, delay: number): void {
  const timer = setTimeout(() => {
    mockSessionTimers.delete(timer);
    action();
  }, delay);
  mockSessionTimers.add(timer);
}

function clearSessionTimers(): void {
  for (const timer of mockSessionTimers) {
    clearTimeout(timer);
  }
  mockSessionTimers.clear();
}

function emitSession(session: SessionSnapshot): void {
  mockEmit("session:event", { kind: "state", session } satisfies SessionEvent);
}

function updateSession(id: string, update: Partial<SessionSnapshot>): SessionSnapshot | undefined {
  const session = mockSessions.find((candidate) => candidate.id === id);
  if (!session) {
    return undefined;
  }
  Object.assign(session, update);
  emitSession(session);
  return session;
}

function mockSessionFrame(serverName: string): string {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="1280" height="720">
    <rect width="1280" height="720" fill="#0e1b2a"/>
    <rect x="24" y="24" width="1232" height="672" rx="12" fill="#13273c" stroke="#2f4d6a"/>
    <text x="640" y="330" fill="#8fd0ff" font-family="Segoe UI, sans-serif" font-size="34" text-anchor="middle">H-Remote session</text>
    <text x="640" y="386" fill="#d8e7f5" font-family="Segoe UI, sans-serif" font-size="24" text-anchor="middle">${serverName.replace(/[<>&]/g, "")}</text>
    <text x="640" y="430" fill="#6f8ba5" font-family="Segoe UI, sans-serif" font-size="16" text-anchor="middle">Synthetic frame (browser mock backend)</text>
  </svg>`;
  return `data:image/svg+xml;base64,${btoa(svg)}`;
}

function mockBackendFor(protocol: string): SessionBackend {
  switch (protocol.toLowerCase()) {
    case "rdp":
      return "ironRdp";
    case "vnc":
      return "vnc";
    case "sftp":
    case "ftp":
      return "fileTransfer";
    case "ssh":
    case "telnet":
    case "serial":
      return "terminal";
    case "localapp":
      return "localApp";
    case "remoteapp":
      return "remoteApp";
    default:
      return "synthetic";
  }
}

function loadConfig(): Configuration {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) {
      return defaultConfiguration();
    }
    const parsed = JSON.parse(raw) as Configuration;
    const defaults = defaultConfiguration();
    return {
      ...defaults,
      ...parsed,
      general: { ...defaults.general, ...parsed.general },
      launcher: { ...defaults.launcher, ...parsed.launcher },
      keywordMatch: { ...defaults.keywordMatch, ...parsed.keywordMatch },
      engagement: { ...defaults.engagement, ...parsed.engagement },
      theme: { ...defaults.theme, ...parsed.theme },
    };
  } catch {
    return defaultConfiguration();
  }
}

function persist(): void {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(config));
  localStorage.setItem(LOCALE_KEY, currentLocale);
}

export function resetMockState(): void {
  clearSessionTimers();
  mockSessions = [];
  mockSessionSeq = 0;
  mockFileSessions.clear();
  tabWorkspace = defaultTabWorkspace();
  records = { schemaVersion: 1, records: {}, quickConnect: [] };
  localStorage.removeItem(RECORDS_KEY);
  localStorage.removeItem(TAB_WORKSPACE_KEY);
  localStorage.removeItem(STORAGE_KEY);
  localStorage.removeItem(CATALOG_KEY);
  localStorage.removeItem(LIST_VIEW_KEY);
  localStorage.removeItem(TREE_VIEW_KEY);
  localStorage.removeItem(LOCALE_KEY);
  config = defaultConfiguration();
  catalog = defaultCatalog();
  mockSources = [];
  listView = defaultListView();
  treeView = defaultTreeView();
  currentLocale = "en-US";
  mockEvents.clear();
}

function localSourceStatus(): SourceStatus {
  return {
    id: "local",
    name: "Local",
    kind: "sqlite",
    state: "ok",
    writable: true,
    serverCount: catalog.servers.filter((server) => (server.sourceId ?? "local") === "local").length,
    credentialCount: catalog.credentials.filter((credential) => !credential.databaseId).length,
    error: null,
    lastChecked: Math.floor(Date.now() / 1000),
  };
}

function mockSourceStatus(config: DataSourceConfig, state: SourceState = "ok"): SourceStatus {
  return {
    id: config.id,
    name: config.name,
    kind: config.kind,
    state,
    writable: false,
    serverCount: catalog.servers.filter((server) => server.sourceId === config.id).length,
    credentialCount: 0,
    error: state === "ok" ? null : `The ${config.kind} source is unavailable (mock).`,
    lastChecked: Math.floor(Date.now() / 1000),
  };
}

function mockSourceConfig(input: DataSourceConfig): DataSourceConfig {
  return {
    id: input.id || `source-${Date.now()}`,
    name: input.name.trim() || "New source",
    kind: input.kind,
    path: input.path,
    host: input.host,
    port: input.port,
    database: input.database,
    username: input.username,
    password: "",
    readOnly: true,
    enabled: input.enabled ?? true,
  };
}

/** Seeds one read-only server so grouping and badges are exercised. */
function seedSourceServer(config: DataSourceConfig): void {
  const id = `mock-${config.id}-1`;
  if (catalog.servers.some((server) => server.id === id)) {
    return;
  }
  const server = {
    ...baseServer(id, "ssh", `${config.name} host`, "10.9.0.10:22 (ops)", ["imported"], []),
    sourceId: config.id,
    address: "10.9.0.10",
    port: 22,
    username: "ops",
    password: "mock",
  } as Server;
  catalog.servers = [...catalog.servers, server];
  saveCatalog();
}

function caps(): Capabilities {
  return {
    os: "windows",
    arch: "x86_64",
    osKeyring: true,
    windowsHello: true,
    registryAutostart: true,
    shortcutFlavor: "lnk",
    hwndEmbedding: true,
    nativeRdpFallback: true,
    embeddedRdp: true,
    globalHotkey: true,
    workingSetTrim: true,
    supportsMultipleMonitors: true,
  };
}

function resolveTheme(theme: ThemeConfig): ResolvedTheme {
  const preset = mockPresets.find(
    (candidate) => candidate.name.toLowerCase() === theme.themeName.toLowerCase(),
  );
  const source = theme.isCustom ? theme : preset;
  if (!source) {
    const fallback = mockPresets.find((candidate) => candidate.name === "Dark")!;
    return {
      id: fallback.name,
      displayName: fallback.displayName,
      colors: fallback.colors,
      fontFamily: theme.fontFamily,
      fontSize: clampFont(theme.fontSize),
    };
  }
  return {
    id: theme.isCustom ? "Custom" : theme.themeName,
    displayName: theme.isCustom ? "Custom" : (preset?.displayName ?? theme.themeName),
    colors: { ...source.colors },
    fontFamily: theme.fontFamily || "system-ui",
    fontSize: clampFont(theme.fontSize),
  };
}

function clampFont(size: number): number {
  if (!Number.isFinite(size)) {
    return 12;
  }
  return Math.min(20, Math.max(10, Math.round(size)));
}

function sanitize(next: Configuration): Configuration {
  const sanitized: Configuration = structuredClone(next);
  sanitized.databaseCheckPeriod = Math.min(99, Math.max(0, sanitized.databaseCheckPeriod));
  sanitized.databaseReconnectPeriod = Math.min(
    3600,
    Math.max(0, sanitized.databaseReconnectPeriod),
  );
  sanitized.launcher.recentLimit = Math.min(20, Math.max(0, sanitized.launcher.recentLimit));
  sanitized.theme.fontSize = clampFont(sanitized.theme.fontSize);
  sanitized.launcher.hotkey = sanitized.launcher.hotkey.trim() || "Alt+M";
  if (sanitized.general.language.trim() === "") {
    sanitized.general.language = "system";
  }
  return sanitized;
}

function bootstrap(): BootstrapDto {
  return {
    app: {
      name: "H-Remote",
      version: "0.1.0",
      targetOs: "windows",
      targetArch: "x86_64",
      debug: true,
    },
    paths: {
      profileMode: "portable",
      baseDir: "C:\\H-Remote",
      dataDir: "C:\\H-Remote",
      localityDir: "C:\\H-Remote\\.locality",
      logsDir: "C:\\H-Remote\\.logs",
      configFile: "C:\\H-Remote\\hremote.json",
      dataSourcesFile: "C:\\H-Remote\\hremote.dataSources.json",
      databaseFile: "C:\\H-Remote\\hremote.db",
      logFile: "C:\\H-Remote\\.logs\\hremote.log.md",
      markerFile: null,
    },
    firstRun: !config.general.guidanceCompleted,
    config: structuredClone(config),
    theme: resolveTheme(config.theme),
    themes: mockPresets,
    locales,
    locale: currentLocale,
    systemLocale: "en-US",
    capabilities: caps(),
    vault: {
      ready: true,
      keyStore: "osKeyring",
      saltPath: "C:\\H-Remote\\.locality\\.encryption_salt",
      message: null,
    },
    preflight: {
      ok: true,
      checks: [
        { key: "baseDir", path: "C:\\H-Remote", writable: true, error: null },
        { key: "logsDir", path: "C:\\H-Remote\\.logs", writable: true, error: null },
      ],
    },
    startup: [
      { name: "process", elapsedMs: 0 },
      { name: "configuration", elapsedMs: 12 },
      { name: "state", elapsedMs: 28 },
    ],
    database: {
      available: true,
      path: "C:\\H-Remote\\hremote.db",
      writable: true,
      schemaVersion: 2,
      encryptionVerified: true,
      serverCount: 0,
      credentialCount: 0,
      error: null,
    },
    legacy: null,
    auxiliary: {
      dataSourcesPresent: false,
      dataSources: [],
      protocolConfigs: [],
      warnings: [],
    },
    protocols: mockProtocols(),
    recoveredFiles: [],
    markerLocked: false,
    feedbackUrl: "https://github.com/h-remote/h-remote/issues",
    logLevel: config.general.logLevel,
    pendingActions: [],
    hotkeyError: null,
    showRatingPrompt: false,
  };
}

const mockEvents = new Map<string, Set<(payload: unknown) => void>>();

export function mockSubscribe<T>(event: string, handler: (payload: T) => void): () => void {
  const listeners = mockEvents.get(event) ?? new Set();
  const wrapped = (payload: unknown) => handler(payload as T);
  listeners.add(wrapped);
  mockEvents.set(event, listeners);
  return () => {
    listeners.delete(wrapped);
    if (listeners.size === 0) {
      mockEvents.delete(event);
    }
  };
}

export function mockEmit(event: string, payload?: unknown): void {
  for (const listener of mockEvents.get(event) ?? []) {
    listener(payload);
  }
}

function mockProtocols(): ProtocolDescriptor[] {
  const make = (
    id: string,
    displayName: string,
    kind: ProtocolDescriptor["capabilities"]["sessionKind"],
  ): ProtocolDescriptor => ({
    id,
    displayName,
    subtitleTemplate: "{address}:{port}",
    category: kind === "terminal" ? "terminal" : "remoteDesktop",
    listOrder: 0,
    capabilities: {
      sessionKind: kind,
      supportsEmbeddedSession: kind !== "terminal",
      supportsMultipleInstances: true,
      supportsFileTransfer: false,
      supportsPrivateKey: kind === "terminal",
      requiresOs: null,
    },
    helpUrl: null,
  });
  return [
    make("RDP", "RDP", "remoteDesktop"),
    make("RemoteApp", "RemoteApp", "remoteDesktop"),
    make("VNC", "VNC", "vnc"),
    make("SSH", "SSH", "terminal"),
    make("Telnet", "Telnet", "terminal"),
    make("Serial", "Serial", "terminal"),
    make("SFTP", "SFTP", "fileTransfer"),
    make("FTP", "FTP", "fileTransfer"),
    make("LocalApp", "Local application", "localProcess"),
    make("Dummy", "Placeholder", "placeholder"),
  ];
}

function catalogTags(servers: Server[]): Tag[] {
  const counts = new Map<string, number>();
  for (const server of servers) {
    for (const tag of server.tags) {
      counts.set(tag, (counts.get(tag) ?? 0) + 1);
    }
  }
  return [...counts.entries()]
    .map(([name, itemsCount]) => ({
      name,
      itemsCount,
      isPinned: false,
      customOrder: 0,
    }))
    .sort((a, b) => a.name.localeCompare(b.name));
}

function catalogTree(
  servers: Server[],
  folders: Folder[],
  sort: string,
  customOrder: string[],
  folderOrder: string[],
): CatalogTree {
  const nodes = new Map<string, { path: string[]; children: Set<string>; servers: Server[] }>();
  const ensure = (path: string[]) => {
    for (let index = 1; index <= path.length; index += 1) {
      const prefix = path.slice(0, index);
      const key = prefix.join("\u001f");
      if (!nodes.has(key)) {
        nodes.set(key, { path: prefix, children: new Set(), servers: [] });
      }
      if (index > 1) {
        nodes.get(path.slice(0, index - 1).join("\u001f"))!.children.add(key);
      }
    }
  };
  folders.forEach((folder) => ensure(folder.path));
  for (const server of servers) {
    ensure(server.treeNodes);
    const key = server.treeNodes.join("\u001f");
    if (key) {
      nodes.get(key)!.servers.push(server);
    }
  }
  const sortServers = (list: Server[]) =>
    list.sort((a, b) => {
      if (sort === "name") return a.displayName.localeCompare(b.displayName);
      if (sort === "protocol") return a.protocol.localeCompare(b.protocol) || a.displayName.localeCompare(b.displayName);
      if (sort === "address") return (a.address ?? "").localeCompare(b.address ?? "", undefined, { numeric: true });
      if (sort === "custom") {
        const ai = customOrder.indexOf(a.id);
        const bi = customOrder.indexOf(b.id);
        return (ai < 0 ? 1e9 : ai) - (bi < 0 ? 1e9 : bi);
      }
      return a.id.localeCompare(b.id);
    });
  const convert = (key: string): any => {
    const node = nodes.get(key)!;
    const childKeys = [...node.children].sort((a, b) => {
      const ap = nodes.get(a)!.path.join("/");
      const bp = nodes.get(b)!.path.join("/");
      const ai = folderOrder.indexOf(ap);
      const bi = folderOrder.indexOf(bp);
      return (ai < 0 ? 1e9 : ai) - (bi < 0 ? 1e9 : bi) || ap.localeCompare(bp);
    });
    return {
      path: node.path,
      name: node.path[node.path.length - 1] ?? "",
      folders: childKeys.map(convert),
      servers: sortServers([...node.servers]),
    };
  };
  const roots = [...nodes.keys()].filter((key) => nodes.get(key)!.path.length === 1);
  const rootServers = servers.filter((server) => server.treeNodes.length === 0);
  return { folders: roots.map(convert), servers: sortServers(rootServers) };
}

function rangesOf(text: string, keywords: string[]): TextRange[] {
  const lower = text.toLowerCase();
  const ranges: TextRange[] = [];
  for (const keyword of keywords) {
    let start = lower.indexOf(keyword);
    while (start >= 0) {
      ranges.push({ start, end: start + keyword.length });
      start = lower.indexOf(keyword, start + 1);
    }
  }
  return ranges.sort((a, b) => a.start - b.start);
}

function filterCatalog(query: string) {
  const include: string[] = [];
  const exclude: string[] = [];
  const keywords: string[] = [];
  for (const token of query.split(/\s+/).filter(Boolean)) {
    if (token.startsWith("-#")) exclude.push(token.slice(2).toLowerCase());
    else if (token.startsWith("#") || token.startsWith("+#")) include.push(token.replace(/^\+?#/, "").toLowerCase());
    else keywords.push(token.toLowerCase());
  }
  return catalog.servers.filter((server) => {
    const tags = server.tags.map((tag) => tag.toLowerCase());
    if (!include.every((tag) => tags.some((candidate) => candidate.startsWith(tag)))) return false;
    if (exclude.some((tag) => tags.some((candidate) => candidate.startsWith(tag)))) return false;
    return keywords.every((keyword) => {
      const haystack = `${server.displayName} ${server.subTitle}`.toLowerCase();
      return haystack.includes(keyword);
    });
  });
}

function canonicalHotkey(input: string): string {  const parts = input
    .split("+")
    .map((part) => part.trim())
    .filter(Boolean);
  if (parts.length < 2) {
    throw {
      code: "validation",
      message: `'${input}' needs a modifier and a key`,
      retryable: false,
    };
  }
  const modifiers = parts.slice(0, -1).map((part) => {
    const lower = part.toLowerCase();
    if (["ctrl", "control"].includes(lower)) return "Ctrl";
    if (lower === "shift") return "Shift";
    if (["alt", "option"].includes(lower)) return "Alt";
    if (["super", "win", "windows", "meta", "cmd", "command"].includes(lower)) return "Super";
    throw { code: "validation", message: `unknown modifier '${part}'`, retryable: false };
  });
  const key = parts[parts.length - 1];
  if (key.length !== 1 && !/^f\d{1,2}$/i.test(key) && !/^[a-z]+$/i.test(key)) {
    throw { code: "validation", message: `unknown key '${key}'`, retryable: false };
  }
  return [...new Set(modifiers), key.length === 1 ? key.toUpperCase() : key].join("+");
}

export async function mockInvoke<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  switch (command) {
    case "bootstrap":
      return bootstrap() as T;
    case "get_config":
      return structuredClone(config) as T;
    case "update_config": {
      config = sanitize(args?.config as Configuration);
      persist();
      return structuredClone(config) as T;
    }
    case "get_themes":
      return mockPresets as T;
    case "resolve_theme": {
      const theme = resolveTheme(args?.theme as ThemeConfig);
      return { theme, cssVariables: [] } as ThemeValidation as T;
    }
    case "get_locales":
      return [locales, currentLocale, "en-US"] as T;
    case "get_locality":
      return mockLocality() as T;
    case "update_tab_workspace": {
      tabWorkspace = {
        layout: String(args?.layout ?? "auto"),
        gridMode: Boolean(args?.gridMode),
        showIcons: Boolean(args?.showIcons),
        showClose: Boolean(args?.showClose),
        showReconnect: Boolean(args?.showReconnect),
      };
      saveTabWorkspace();
      return mockLocality() as T;
    }
    case "open_session_window":
      return "sessions" as T;
    case "set_window_title":
      return undefined as T;
    case "save_window_placement":
    case "reset_main_window":
    case "set_server_view":
    case "ui_ready":
    case "flash_window":
    case "show_main_window":
      return undefined as T;
    case "record_connection":
      recordMockConnection(String(args?.serverId ?? ""));
      return undefined as T;
    case "get_connection_records":
      return records as ConnectionRecords as T;
    case "catalog_snapshot":
      return {
        servers: catalog.servers,
        folders: catalog.folders,
        credentials: catalog.credentials,
        protocols: mockProtocols(),
        tags: catalogTags(catalog.servers),
        connectionRecords: { schemaVersion: 1, records: {}, quickConnect: [] },
      } as CatalogSnapshot as T;
    case "data_source_list":
      return mockSources.map((source) => ({ ...source })) as T;
    case "data_source_statuses":
      return [
        localSourceStatus(),
        ...mockSources.map((source) => mockSourceStatus(source)),
      ] as T;
    case "change_local_database_path": {
      config.sqliteDatabasePath =
        typeof args?.path === "string" && String(args.path).trim()
          ? String(args.path).trim()
          : null;
      persist();
      return {
        available: true,
        path: config.sqliteDatabasePath ?? "hremote.db",
        writable: true,
        schemaVersion: 3,
        encryptionVerified: true,
        serverCount: catalog.servers.length,
        credentialCount: catalog.credentials.length,
        error: null,
      } as DatabaseInfo as T;
    }
    case "data_source_save": {
      const saved = mockSourceConfig(args?.config as DataSourceConfig);
      const index = mockSources.findIndex((source) => source.id === saved.id);
      if (index >= 0) {
        mockSources[index] = saved;
      } else {
        mockSources.push(saved);
      }
      seedSourceServer(saved);
      return saved as T;
    }
    case "data_source_delete": {
      const id = String(args?.id ?? "");
      mockSources = mockSources.filter((source) => source.id !== id);
      catalog.servers = catalog.servers.filter((server) => server.sourceId !== id);
      saveCatalog();
      return true as T;
    }
    case "data_source_test": {
      const probe = mockSourceConfig(args?.config as DataSourceConfig);
      return mockSourceStatus(probe) as T;
    }
    case "data_source_refresh": {
      const id = String(args?.id ?? "");
      const source = mockSources.find((candidate) => candidate.id === id);
      if (!source) {
        throw new Error("data source not found");
      }
      seedSourceServer(source);
      return mockSourceStatus(source) as T;
    }
    case "data_source_refresh_all":
      return [
        localSourceStatus(),
        ...mockSources.map((source) => mockSourceStatus(source)),
      ] as T;
    case "i18n_report":
      return {
        locale: currentLocale,
        missing: [],
        locales,
      } as T;
    case "check_for_update": {
      const feed = config.general.updateFeedUrl;
      const base = {
        enabled: !config.general.doNotCheckForUpdates,
        feedConfigured: Boolean(feed),
        current: "0.1.0",
        checkedAt: Math.floor(Date.now() / 1000),
      };
      if (!feed) {
        return {
          ...base,
          available: false,
          version: null,
          prerelease: false,
          breaking: false,
          notes: "",
          url: null,
          error: "No update feed is configured.",
        } as T;
      }
      return {
        ...base,
        available: true,
        version: "0.2.0",
        prerelease: false,
        breaking: false,
        notes: "Faster sessions and UI polish.",
        url: "https://example.invalid/h-remote-0.2.0.exe",
        error: null,
      } as T;
    }
    case "update_check_status":
      return {
        enabled: !config.general.doNotCheckForUpdates,
        feedConfigured: Boolean(config.general.updateFeedUrl),
        current: "0.1.0",
        checkedAt: config.engagement.lastUpdateCheckAt,
        available: false,
        version: null,
        prerelease: false,
        breaking: false,
        notes: "",
        url: null,
        error: null,
      } as T;
    case "rating_decision": {
      const decision = String(args?.decision ?? "later");
      config.engagement.lastRatingPromptAt = Math.floor(Date.now() / 1000);
      if (decision === "never") {
        config.engagement.doNotShowRating = true;
      }
      persist();
      return config as T;
    }
    case "legacy_database":
      return null as T;
    case "import_legacy_servers":
      return { imported: 0, warnings: [] } as T;
    case "detect_legacy_profile":
      return null as T;
    case "migrate_legacy_settings":
      return { source: "1Remote", importedKeys: [] } as T;
    case "dismiss_legacy_migration": {
      config.general.skipLegacyMigration = true;
      persist();
      return config as T;
    }
    case "new_server": {
      const protocol = String(args?.protocol ?? "rdp") as Server["protocol"];
      return {
        ...baseServer(`mock-${Date.now()}`, protocol, "New server", "", [], []),
        address: "",
        port: protocol === "vnc" ? 5900 : protocol === "ssh" || protocol === "sftp" ? 22 : protocol === "ftp" ? 21 : 3389,
        username: "",
        password: "",
      } as Server as T;
    }
    case "get_locality":
      return mockLocality() as T;
    case "catalog_view": {
      const query = String(args?.query ?? "");
      const source = String(args?.source ?? "");
      const servers = filterCatalog(query).filter((server) =>
        !source || source === "all" ? true : (server.sourceId ?? "local") === source,
      );
      const keywords = query
        .split(/\s+/)
        .filter((token) => token && !token.includes("#"))
        .map((token) => token.toLowerCase());
      const tree = catalogTree(
        servers,
        catalog.folders,
        String(args?.sort ?? "insertion"),
        (args?.customOrder as string[]) ?? [],
        (args?.folderOrder as string[]) ?? [],
      );
      const hits = servers.map((server) => ({
        id: server.id,
        nameRanges: rangesOf(server.displayName, keywords),
        subtitleRanges: rangesOf(server.subTitle, keywords),
      }));
      return {
        tree,
        hits,
        tags: catalogTags(servers),
        total: servers.length,
      } as CatalogView as T;
    }
    case "save_server": {
      const server = args?.server as Server;
      if ((server.sourceId ?? "local") !== "local") {
        throw new Error("read-only data source");
      }
      const index = catalog.servers.findIndex((candidate) => candidate.id === server.id);
      if (index >= 0) catalog.servers[index] = server;
      else catalog.servers.push(server);
      saveCatalog();
      return server as T;
    }
    case "save_servers": {
      const servers = (args?.servers as Server[]) ?? [];
      if (servers.some((server) => (server.sourceId ?? "local") !== "local")) {
        throw new Error("read-only data source");
      }
      for (const server of servers) {
        const index = catalog.servers.findIndex((candidate) => candidate.id === server.id);
        if (index >= 0) catalog.servers[index] = server;
        else catalog.servers.push(server);
      }
      saveCatalog();
      return servers as T;
    }
    case "delete_server": {
      if (
        catalog.servers.some(
          (server) => server.id === args?.id && (server.sourceId ?? "local") !== "local",
        )
      ) {
        throw new Error("read-only data source");
      }
      const before = catalog.servers.length;
      catalog.servers = catalog.servers.filter((server) => server.id !== args?.id);
      saveCatalog();
      return (catalog.servers.length !== before) as T;
    }
    case "list_credentials":
      return catalog.credentials as T;
    case "save_credential": {
      const credential = args?.credential as Credential;
      const index = catalog.credentials.findIndex((candidate) => candidate.id === credential.id);
      if (index >= 0) catalog.credentials[index] = credential;
      else catalog.credentials.push(credential);
      saveCatalog();
      return credential as T;
    }
    case "delete_credential": {
      const before = catalog.credentials.length;
      catalog.credentials = catalog.credentials.filter((credential) => credential.id !== args?.id);
      saveCatalog();
      return (catalog.credentials.length !== before) as T;
    }
    case "create_credential": {
      const credential: Credential = {
        id: `cred-${Date.now()}`,
        name: String(args?.name ?? ""),
        address: null,
        port: null,
        username: String(args?.username ?? ""),
        password: String(args?.password ?? ""),
        privateKeyPath: null,
        databaseId: null,
        hash: "mock",
        isEditable: true,
      };
      catalog.credentials.push(credential);
      saveCatalog();
      return credential as T;
    }
    case "create_folder": {
      const path = (args?.path as string[]) ?? [];
      const key = path.join("\u001f");
      const existing = catalog.folders.find((folder) => folder.path.join("\u001f") === key);
      if (existing) return existing as T;
      const folder: Folder = { id: `folder-${Date.now()}-${catalog.folders.length}`, path };
      catalog.folders.push(folder);
      saveCatalog();
      return folder as T;
    }
    case "ensure_folder": {
      const path = (args?.path as string[]) ?? [];
      let folder: Folder | undefined;
      for (let index = 1; index <= path.length; index += 1) {
        folder = (await mockInvoke("create_folder", { path: path.slice(0, index) })) as Folder;
      }
      return folder as T;
    }
    case "rename_folder": {
      const folderId = String(args?.folderId ?? "");
      const folder = catalog.folders.find((candidate) => candidate.id === folderId);
      if (!folder) throw { code: "not_found", message: "folder not found", retryable: false };
      const oldPath = [...folder.path];
      const newPath = [...oldPath.slice(0, -1), String(args?.newName ?? "").trim()];
      folder.path = newPath;
      for (const server of catalog.servers) {
        if (oldPath.every((segment, index) => server.treeNodes[index] === segment)) {
          server.treeNodes = [...newPath, ...server.treeNodes.slice(oldPath.length)];
          server.subTitle = server.subTitle;
        }
      }
      saveCatalog();
      return 1 as T;
    }
    case "delete_folder": {
      const path = (args?.path as string[]) ?? [];
      const matches = catalog.folders.filter((folder) =>
        path.every((segment, index) => folder.path[index] === segment),
      );
      catalog.folders = catalog.folders.filter((folder) => !matches.includes(folder));
      let moved = 0;
      for (const server of catalog.servers) {
        if (path.every((segment, index) => server.treeNodes[index] === segment)) {
          server.treeNodes = server.treeNodes.slice(path.length);
          moved += 1;
        }
      }
      saveCatalog();
      return [matches.length, moved] as T;
    }
    case "move_folder": {
      const folder = catalog.folders.find((candidate) => candidate.id === args?.folderId);
      if (!folder) throw { code: "not_found", message: "folder not found", retryable: false };
      const oldPath = [...folder.path];
      folder.path = [...((args?.parent as string[]) ?? []), folder.path[folder.path.length - 1]];
      const newPath = folder.path;
      for (const server of catalog.servers) {
        if (oldPath.every((segment, index) => server.treeNodes[index] === segment)) {
          server.treeNodes = [...newPath, ...server.treeNodes.slice(oldPath.length)];
        }
      }
      saveCatalog();
      return 1 as T;
    }
    case "duplicate_folder": {
      const folder = catalog.folders.find((candidate) => candidate.id === args?.folderId);
      if (!folder) throw { code: "not_found", message: "folder not found", retryable: false };
      const copy: Folder = {
        id: `folder-${Date.now()}`,
        path: [...folder.path.slice(0, -1), `${folder.path[folder.path.length - 1]} (copy)`],
      };
      catalog.folders.push(copy);
      saveCatalog();
      return copy as T;
    }
    case "move_server_to_folder": {
      const server = catalog.servers.find((candidate) => candidate.id === args?.serverId);
      if (!server) throw { code: "not_found", message: "server not found", retryable: false };
      server.treeNodes = (args?.path as string[]) ?? [];
      saveCatalog();
      return server as T;
    }
    case "get_list_view":
      return listView as T;
    case "update_list_view":
      listView = args?.view as ListViewLocality;
      saveListTree();
      return listView as T;
    case "get_tree_view":
      return treeView as T;
    case "update_tree_view":
      treeView = args?.view as TreeViewLocality;
      saveListTree();
      return treeView as T;
    case "set_custom_order":
      listView.customOrder = (args?.ids as string[]) ?? [];
      saveListTree();
      return listView as T;
    case "export_servers_json": {
      const ids = (args?.ids as string[]) ?? [];
      const selected = ids.length
        ? catalog.servers.filter((server) => ids.includes(server.id))
        : catalog.servers;
      return JSON.stringify(
        { schemaVersion: 1, exportedAt: Date.now(), servers: selected },
        null,
        2,
      ) as T;
    }
    case "export_rdp_server": {
      const server = catalog.servers.find((candidate) => candidate.id === args?.id);
      return {
        content: `full address:s:${server?.address ?? ""}:${server?.port ?? 3389}`,
        passwordProtected: false,
        warning: "Mock export does not protect the password.",
      } as T;
    }
    case "import_json_content": {
      const parsed = JSON.parse(String(args?.content ?? "{}")) as
        | { servers?: Server[] }
        | Array<Record<string, unknown>>;
      const servers: Server[] = Array.isArray(parsed)
        ? parsed
            .filter((item) => typeof item.Protocol === "string")
            .map((item) => {
              const protocol = String(item.Protocol).toLowerCase();
              const mapped = protocol === "remoteapp" ? "rdp" : protocol;
              const server = baseServer(
                `mock-import-${Date.now()}-${catalog.servers.length}-${Math.random().toString(36).slice(2, 7)}`,
                mapped as Server["protocol"],
                String(item.DisplayName ?? item.Address ?? "Imported"),
                `${String(item.Address ?? "")}:${String(item.Port ?? "")}`,
                Array.isArray(item.Tags) ? item.Tags.map(String) : [],
                Array.isArray(item.TreeNodes) ? item.TreeNodes.map(String) : [],
              );
              return {
                ...server,
                address: String(item.Address ?? ""),
                port: Number(item.Port ?? 0) || undefined,
                username: String(item.UserName ?? ""),
                password: String(item.Password ?? ""),
                note: String(item.Note ?? ""),
              } as Server;
            })
        : (parsed.servers ?? []);
      for (const server of servers) {
        server.id = `mock-${Date.now()}-${catalog.servers.length}`;
        catalog.servers.push(server);
      }
      saveCatalog();
      return { imported: servers.length, warnings: [] } as ImportOutcome as T;
    }
    case "import_csv_content":
    case "import_rdp_content":
    case "import_database_file":
      return {
        imported: 0,
        warnings: ["Mock backend does not parse this format; use the desktop app."],
      } as ImportOutcome as T;
    case "create_shortcut_for_servers":
    case "create_shortcut_for_tag":
      return {
        path: "C:\\Users\\Public\\Desktop\\H-Remote shortcut.lnk",
        iconPath: null,
      } as ShortcutOutcome as T;
    case "save_icon_to_library":
      return `C:\\H-Remote\\.icons\\library\\${Date.now()}.png` as T;
    case "list_icon_library":
      return [] as T;
    case "delete_icon_from_library":
      return true as T;
    case "fetch_icon_from_url":
      throw { code: "unsupported", message: "URL icons require the desktop app", retryable: false };
    case "test_script":
      return { success: true, code: 0, stdout: "", stderr: "" } as T;
    case "connect_server": {
      const server = catalog.servers.find((candidate) => candidate.id === args?.id);
      if (!server) {
        throw { code: "not_found", message: "server not found", retryable: false };
      }
      if (server.protocol === "sftp" || server.protocol === "ftp") {
        const id = `mock-files-${Date.now()}-${(mockSessionSeq += 1)}`;
        const home = server.startupPath || "/";
        mockFileSessions.set(id, { path: home, home });
        const session: SessionSnapshot = {
          id,
          serverId: server.id,
          serverName: server.displayName,
          backend: "fileTransfer",
          state: "connected",
          reason: null,
          message: null,
          attempt: 1,
          maxAttempts: 1,
          width: 0,
          height: 0,
          frameCount: 0,
          startedAt: Date.now(),
          connectedAt: Date.now(),
          endedAt: null,
          frameRing: null,
        };
        mockSessions = [...mockSessions, session];
        recordMockConnection(server.id);
        return session as T;
      }
      const viewport = (args?.viewport as [number, number] | undefined) ?? [1280, 720];
      const session: SessionSnapshot = {
        id: `mock-session-${Date.now()}-${(mockSessionSeq += 1)}`,
        serverId: server.id,
        serverName: server.displayName,
        backend: mockBackendFor(server.protocol),
        state: "connecting",
        reason: null,
        message: null,
        attempt: 1,
        maxAttempts: 5,
        width: viewport[0],
        height: viewport[1],
        frameCount: 0,
        startedAt: Date.now(),
        connectedAt: null,
        endedAt: null,
        frameRing: null,
      };
      mockSessions = [...mockSessions, session];
      later(() => {
        updateSession(session.id, {
          state: "connected",
          connectedAt: Date.now(),
          frameCount: 1,
        });
        mockEmit("session:event", {
          kind: "frame",
          sessionId: session.id,
          seq: 1,
          width: session.width,
          height: session.height,
        } satisfies SessionEvent);
      }, 200);
      recordMockConnection(server.id);
      return session as T;
    }
    case "quick_connect": {
      const address = String(args?.address ?? "").trim();
      if (!address) {
        throw { code: "validation", message: "an address is required", retryable: false };
      }
      const port = Number(args?.port ?? 3389) || 3389;
      const username = String(args?.username ?? "");
      const session: SessionSnapshot = {
        id: `mock-session-${Date.now()}-${(mockSessionSeq += 1)}`,
        serverId: `quick-${address}-${port}`,
        serverName: username ? `${address} (${username})` : address,
        backend: mockBackendFor(String(args?.protocol ?? "rdp")),
        state: "connecting",
        reason: null,
        message: null,
        attempt: 1,
        maxAttempts: 5,
        width: 1280,
        height: 720,
        frameCount: 0,
        startedAt: Date.now(),
        connectedAt: null,
        endedAt: null,
        frameRing: null,
      };
      mockSessions = [...mockSessions, session];
      later(() => {
        updateSession(session.id, {
          state: "connected",
          connectedAt: Date.now(),
          frameCount: 1,
        });
        mockEmit("session:event", {
          kind: "frame",
          sessionId: session.id,
          seq: 1,
          width: session.width,
          height: session.height,
        } satisfies SessionEvent);
      }, 200);
      records.quickConnect = [
        {
          protocol: String(args?.protocol ?? "RDP").toUpperCase(),
          address,
          port,
          username,
          lastUsedAt: Math.floor(Date.now() / 1000),
        },
        ...records.quickConnect.filter(
          (entry) =>
            !(
              entry.address === address &&
              entry.port === port &&
              entry.username === username
            ),
        ),
      ].slice(0, 20);
      saveRecords();
      if (Boolean(args?.save)) {
        const server: Server = {
          ...baseServer(
            `mock-${Date.now()}-${mockSessionSeq}`,
            "rdp",
            session.serverName,
            `${address}:${port}`,
            [],
            [],
          ),
          address,
          port,
          username,
          password: String(args?.password ?? ""),
        };
        catalog.servers.push(server);
        saveCatalog();
      }
      if (!Boolean(args?.save)) {
        // Ad-hoc sessions still appear in recent order by pseudo id.
        recordMockConnection(session.serverId);
      }
      return { session, saved: Boolean(args?.save) } as T;
    }
    case "quick_connect_history":
      return [...records.quickConnect] as T;
    case "show_launcher":
    case "focus_server_in_catalog":
    case "hide_launcher":
    case "resize_launcher":
      return undefined as T;
    case "check_server_availability": {
      const server = catalog.servers.find((candidate) => candidate.id === args?.id);
      if (!server?.address) {
        return [] as T;
      }
      return [
        {
          name: "Main",
          address: server.address,
          port: server.port ?? 0,
          reachable: true,
          latencyMs: 12,
        },
        ...(server.alternateCredentials ?? []).map((alternate) => ({
          name: alternate.name,
          address: alternate.address ?? server.address!,
          port: alternate.port ?? server.port ?? 0,
          reachable: true,
          latencyMs: 20,
        })),
      ] as T;
    }
    case "verify_sensitive_action":
      return { verified: true, supported: true, message: null } as T;
    case "verification_status":
      return 30 as T;
    case "file_list": {
      const id = String(args?.sessionId ?? "");
      const session = mockFileSessions.get(id);
      if (!session) {
        throw { code: "not_found", message: "file session not found", retryable: false };
      }
      const path = String(args?.path ?? session.path);
      session.path = path;
      const entries = MOCK_FILE_ENTRIES.map((entry) => ({
        name: entry.name,
        path: `${path.replace(/\/$/, "")}/${entry.name}`,
        isDir: entry.isDir,
        size: entry.size,
        modified: entry.isDir ? null : Math.floor(Date.now() / 1000) - 3600,
      }));
      return { path, home: session.home, entries } as T;
    }
    case "file_rename":
    case "file_delete":
    case "file_mkdir":
      return undefined as T;
    case "file_upload":
    case "file_upload_dialog":
    case "file_download":
    case "file_download_dialog":
      return [] as T;
    case "file_transfers":
      return [] as T;
    case "file_cancel_transfer":
      return false as T;
    case "session_list":
      return [...mockSessions] as T;
    case "session_snapshot": {
      const session = mockSessions.find((candidate) => candidate.id === args?.id);
      if (!session) {
        throw { code: "not_found", message: "session not found", retryable: false };
      }
      return session as T;
    }
    case "session_cancel": {
      const id = String(args?.id ?? "");
      updateSession(id, {
        state: "disconnected",
        reason: "localCancel",
        message: "Disconnected locally",
        endedAt: Date.now(),
      });
      return undefined as T;
    }
    case "session_reconnect": {
      const id = String(args?.id ?? "");
      const session = updateSession(id, {
        state: "reconnecting",
        reason: null,
        message: null,
        endedAt: null,
        attempt: 1,
        frameCount: 0,
      });
      if (!session) {
        throw { code: "not_found", message: "session not found", retryable: false };
      }
      later(() => {
        updateSession(id, {
          state: "connected",
          connectedAt: Date.now(),
          frameCount: 1,
        });
        mockEmit("session:event", {
          kind: "frame",
          sessionId: id,
          seq: 2,
          width: session.width,
          height: session.height,
        } satisfies SessionEvent);
      }, 200);
      return session as T;
    }
    case "session_remove": {
      const before = mockSessions.length;
      mockSessions = mockSessions.filter((session) => session.id !== args?.id);
      return (mockSessions.length !== before) as T;
    }
    case "session_resize": {
      updateSession(String(args?.id ?? ""), {
        width: Number(args?.width ?? 0),
        height: Number(args?.height ?? 0),
      });
      return undefined as T;
    }
    case "session_send_ctrl_alt_del":
    case "session_send_special":
    case "session_vnc_key":
    case "session_vnc_pointer":
    case "session_vnc_refresh":
    case "session_vnc_text":
    case "session_rdp_key":
    case "session_rdp_pointer_move":
    case "session_rdp_pointer_button":
    case "session_rdp_wheel":
      return undefined as T;
    case "get_runners":
      return defaultRunners() as T;
    case "save_runner": {
      const runners = defaultRunners();
      const runner = args?.runner as Runner;
      const index = runners.runners.findIndex((item) => item.id === runner.id);
      if (index >= 0) runners.runners[index] = runner;
      else runners.runners.push(runner);
      return runners as T;
    }
    case "delete_runner":
    case "set_default_runner":
      return defaultRunners() as T;
    case "session_frame_ring":
      return null as T;
    case "session_frame_image": {
      const session = mockSessions.find((candidate) => candidate.id === args?.id);
      return (session
        ? {
            seq: Date.now(),
            width: 1280,
            height: 720,
            dataUrl: mockSessionFrame(session.serverName),
          }
        : null) as T;
    }
    case "recent_sessions":
      return mockRecentIds(20).map((id) => ({
        serverId: id,
        lastConnectedAt: records.records[id]?.lastConnectedAt ?? 0,
        connectCount: records.records[id]?.connectCount ?? 0,
      })) as RecentSessionInfo[] as T;
    case "get_tags":
      return { schemaVersion: 1, pinned: [], order: [] } as T;
    case "set_pinned_tags":
      return { schemaVersion: 1, pinned: args?.tags ?? [], order: [] } as T;
    case "set_tag_order":
      return { schemaVersion: 1, pinned: [], order: args?.order ?? [] } as T;
    case "open_log_folder":
    case "open_config_folder":
    case "open_url":
    case "install_startup":
    case "uninstall_startup":
      return undefined as T;
    case "read_log_tail":
      return "[mock] H-Remote log\n[mock] log output is simulated in the browser" as T;
    case "report_ui_error":
    case "save_error_report":
      return { path: "C:\\H-Remote\\.logs\\crash-mock.md", markdown: String(args?.markdown ?? "") } as CrashReport as T;
    case "list_crash_reports":
      return [] as CrashReportInfo[] as T;
    case "read_crash_report":
      return "# H-Remote crash report\n\nMock report" as T;
    case "trim_working_set":
      return "trimmed" as TrimOutcome as T;
    case "validate_hotkey":
      return canonicalHotkey(String(args?.hotkey ?? "")) as T;
    case "app_info":
      return {
        name: "H-Remote",
        version: "0.1.0",
        targetOs: "windows",
        targetArch: "x86_64",
        debug: true,
      } as T;
    case "capabilities":
      return caps() as T;
    case "startup_checkpoints":
      return bootstrap().startup as T;
    case "vault_status":
      return {
        ready: true,
        keyStore: "osKeyring",
        saltPath: "C:\\H-Remote\\.locality\\.encryption_salt",
        message: null,
      } as VaultStatus as T;
    case "database_status":
      return bootstrap().database as T;
    case "set_log_level": {
      config.general.logLevel = args?.level as Configuration["general"]["logLevel"];
      persist();
      return structuredClone(config) as T;
    }
    case "complete_guidance": {
      const selection = args?.selection as GuidanceSelection;
      config.general.guidanceCompleted = true;
      config.general.confirmBeforeClosingSession = selection.confirmBeforeClosingSession;
      config.general.startMinimized = selection.startMinimized;
      config.general.runAtStartup = selection.runAtStartup;
      if (selection.language) {
        config.general.language = selection.language;
        currentLocale = selection.language === "system" ? "en-US" : selection.language;
      }
      if (selection.customTheme) {
        config.theme = selection.customTheme;
      } else if (selection.themeName) {
        const preset = mockPresets.find(
          (candidate) => candidate.name.toLowerCase() === selection.themeName!.toLowerCase(),
        );
        if (preset) {
          config.theme = {
            ...config.theme,
            themeName: preset.name,
            isCustom: false,
            colors: { ...preset.colors },
          };
        }
      }
      config = sanitize(config);
      persist();
      const result: GuidanceResult = {
        config: structuredClone(config),
        restartRequired: false,
        migrated: null,
        shortcutPath: selection.createShortcut
          ? "C:\\Users\\Public\\Desktop\\H-Remote.lnk"
          : null,
        warnings: [],
      };
      return result as T;
    }
    case "set_profile_mode":
      return {
        mode: args?.mode as ProfileMode,
        markerPath: "C:\\H-Remote\\FORCE_INTO_PORTABLE_MODE",
        restartRequired: true,
      } as T;
    case "detect_legacy_profile":
      return null as T;
    case "migrate_legacy_settings":
      throw { code: "not_found", message: "not found: legacy profile", retryable: false };
    case "dismiss_legacy_migration":
      config.general.skipLegacyMigration = true;
      persist();
      return structuredClone(config) as T;
    case "install_desktop_shortcut":
      return "C:\\Users\\Public\\Desktop\\H-Remote.lnk" as T;
    case "uninstall_desktop_shortcut":
      return true as T;
    case "startup_status":
      return false as T;
    case "handle_cli":
      return { actions: [], errors: [] } as CliInvocationSummary as T;
    default:
      throw { code: "unsupported", message: `mock backend: ${command}`, retryable: false };
  }
}

declare global {
  interface Window {
    __HREMOTE_MOCK_RESET__?: () => void;
    __HREMOTE_MOCK_EMIT__?: (event: string, payload?: unknown) => void;
  }
}

if (typeof window !== "undefined") {
  window.__HREMOTE_MOCK_RESET__ = resetMockState;
  window.__HREMOTE_MOCK_EMIT__ = mockEmit;
}
