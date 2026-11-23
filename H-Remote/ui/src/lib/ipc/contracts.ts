// Typed IPC contract. Mirrors the `#[serde(rename_all = "camelCase")]` DTOs in
// `src-tauri/src/dto.rs` and the shared `hr-*` crates. Keep this file in sync
// whenever a Rust DTO changes; tests assert the bootstrap fixture shape.

export type ProfileMode = "portable" | "appData";
export type CloseButtonAction = "exit" | "minimizeToTray";
export type LogLevel = "off" | "error" | "warn" | "info" | "debug" | "trace";
export type MatcherKind = "direct" | "initials" | "discrete" | "pinyin" | "pinyinInitials";
export type ServerView = "card" | "list" | "tree";
export type ServerSort = "insertion" | "protocol" | "name" | "address" | "custom";
export type ShortcutFlavor = "lnk" | "desktopEntry" | "commandScript";
export type KeyStoreKind = "osKeyring" | "passphraseFile" | "memory";
export type TrimOutcome = "trimmed" | "unsupported" | "failed";
export type LegacyKind = "premoteM" | "oneRemote";

export interface IpcError {
  code: string;
  message: string;
  retryable: boolean;
}

export interface AppInfo {
  name: string;
  version: string;
  targetOs: string;
  targetArch: string;
  debug: boolean;
}

export interface PathsDto {
  profileMode: ProfileMode;
  baseDir: string;
  dataDir: string;
  localityDir: string;
  logsDir: string;
  configFile: string;
  dataSourcesFile: string;
  databaseFile: string;
  logFile: string;
  markerFile: string | null;
}

export interface PathCheck {
  key: string;
  path: string;
  writable: boolean;
  error: string | null;
}

export interface PreflightReport {
  checks: PathCheck[];
  ok: boolean;
}

export interface DatabaseInfo {
  available: boolean;
  path: string;
  writable: boolean | null;
  schemaVersion: number | null;
  encryptionVerified: boolean | null;
  serverCount: number | null;
  credentialCount: number | null;
  error: string | null;
}

export type ProtocolCategory =
  | "remoteDesktop"
  | "terminal"
  | "fileTransfer"
  | "application"
  | "local"
  | "utility";

export type SessionKind =
  | "remoteDesktop"
  | "vnc"
  | "terminal"
  | "fileTransfer"
  | "localProcess"
  | "placeholder";

export interface ProtocolCapabilities {
  sessionKind: SessionKind;
  supportsEmbeddedSession: boolean;
  supportsMultipleInstances: boolean;
  supportsFileTransfer: boolean;
  supportsPrivateKey: boolean;
  requiresOs: "windows" | "linux" | "macOs" | null;
}

export interface ProtocolDescriptor {
  id: string;
  displayName: string;
  subtitleTemplate: string;
  category: ProtocolCategory;
  listOrder: number;
  capabilities: ProtocolCapabilities;
  helpUrl: string | null;
}

export type ServerProtocol =
  | "rdp"
  | "remoteApp"
  | "vnc"
  | "ssh"
  | "telnet"
  | "serial"
  | "sftp"
  | "ftp"
  | "localApp"
  | "custom"
  | "dummy";

export interface TextRange {
  start: number;
  end: number;
}

export interface SearchHit {
  id: string;
  nameRanges: TextRange[];
  subtitleRanges: TextRange[];
}

export interface Folder {
  id: string;
  path: string[];
}

export interface Tag {
  name: string;
  isPinned: boolean;
  customOrder: number;
  itemsCount: number;
}

export interface AppArgument {
  type:
    | "normal"
    | "int"
    | "float"
    | "file"
    | "secret"
    | "flag"
    | "selection"
    | "const";
  name: string;
  key: string;
  isNullable: boolean;
  addBlankAfterKey: boolean;
  addBlankAfterValue: boolean;
  value: string;
  description: string;
  selections: Record<string, string>;
}

export interface AlternateCredential {
  name: string;
  address: string | null;
  port: number | null;
  username: string;
  password: string;
  privateKey: string | null;
}

export interface Server {
  protocol: ServerProtocol;
  id: string;
  classVersion: number;
  displayName: string;
  subTitle: string;
  iconBase64: string | null;
  colorHex: string;
  tags: string[];
  treeNodes: string[];
  note: string;
  commandBeforeConnected: string;
  hideCommandBeforeConnectedWindow: boolean;
  commandAfterDisconnected: string;
  selectedRunnerName: string | null;
  alwaysOpenInNewTabWindow: boolean;
  sessionId: string | null;
  /** Owning data source; `local` is the writable catalog. */
  sourceId: string;

  address?: string;
  port?: number;
  alternateCredentials?: AlternateCredential[];
  isPingBeforeConnect?: boolean;
  isAutoAlternateAddressSwitching?: boolean;
  username?: string;
  password?: string;
  privateKey?: string | null;
  askPasswordWhenConnect?: boolean;
  inheritedCredentialName?: string | null;

  // RDP / RemoteApp
  domain?: string;
  isAdministrativePurposes?: boolean;
  gatewayMode?: string;
  gatewayHostName?: string;
  gatewayUserName?: string;
  gatewayPassword?: string;
  gatewayLogonMethod?: string;
  gatewayBypassForLocalAddress?: boolean;
  rdpFullScreenFlag?: string;
  isConnWithFullScreen?: boolean;
  isFullScreenWithConnectionBar?: boolean;
  isPinTheConnectionBarByDefault?: boolean;
  rdpWindowResizeMode?: string;
  rdpWidth?: number;
  rdpHeight?: number;
  isScaleFactorFollowSystem?: boolean;
  scaleFactorCustomValue?: number;
  displayPerformance?: string;
  useMultimon?: boolean;
  enableClipboard?: boolean;
  enableDiskDrives?: boolean;
  enableRedirectDrivesPlugIn?: boolean;
  enableRedirectCameras?: boolean;
  enableKeyCombinations?: boolean;
  audioRedirectionMode?: string;
  audioQualityMode?: string;
  enableAudioCapture?: boolean;
  enablePorts?: boolean;
  enablePrinters?: boolean;
  enableSmartCardsAndWinHello?: boolean;
  mstscModeEnabled?: boolean;
  rdpFileAdditionalSettings?: string;
  rdpControlAdditionalSettings?: Array<{ name: string; value: string; valueType: string }>;
  remoteApplicationName?: string;
  remoteApplicationProgram?: string;

  // VNC / SSH / Telnet / Serial
  vncWindowResizeMode?: string;
  sshVersion?: string;
  startupAutoCommand?: string;
  externalSessionConfigPath?: string | null;
  externalKittySessionConfigPath?: string | null;
  openSftpOnConnected?: boolean;
  serialPort?: string;
  bitRate?: number;
  dataBits?: number;
  stopBits?: number;
  parity?: string;
  flowControl?: string;
  startupPath?: string;

  // Local application
  exePath?: string;
  appProtocolDisplayName?: string;
  argumentList?: AppArgument[];
  runWithHosting?: boolean;
  arguments?: string;

  // Custom
  customProtocolName?: string;
  extraFields?: unknown;

  [key: string]: unknown;
}

export interface CatalogFolder {
  path: string[];
  name: string;
  folders: CatalogFolder[];
  servers: Server[];
}

export interface CatalogTree {
  folders: CatalogFolder[];
  servers: Server[];
}

export interface CatalogSnapshot {
  servers: Server[];
  folders: Folder[];
  credentials: Credential[];
  protocols: ProtocolDescriptor[];
  tags: Tag[];
  connectionRecords: ConnectionRecords;
}

export interface CatalogView {
  tree: CatalogTree;
  hits: SearchHit[];
  tags: Tag[];
  total: number;
}

export type DataSourceKind = "sqlite" | "mysql" | "postgres";

export type SourceState =
  | "ok"
  | "notConnectedYet"
  | "lostConnection"
  | "accessDenied"
  | "encryptKeyError"
  | "otherError";

export interface DataSourceConfig {
  id: string;
  name: string;
  kind: DataSourceKind;
  path: string | null;
  host: string;
  port: number;
  database: string;
  username: string;
  /** Always empty: stored passwords never travel to the UI. */
  password: string;
  readOnly: boolean;
  enabled: boolean;
}

export interface SourceStatus {
  id: string;
  name: string;
  kind: string;
  state: SourceState;
  writable: boolean;
  serverCount: number;
  credentialCount: number;
  error: string | null;
  lastChecked: number;
}

export interface I18nReport {
  locale: string;
  missing: string[];
  locales: LocaleInfo[];
}

export interface Credential {
  id: string;
  name: string;
  address: string | null;
  port: number | null;
  username: string;
  password: string;
  privateKeyPath: string | null;
  databaseId: string | null;
  hash: string;
  isEditable: boolean;
}

export interface ImportOutcome {
  imported: number;
  warnings: string[];
}

export interface RdpExport {
  content: string;
  passwordProtected: boolean;
  warning: string | null;
}

export interface ShortcutOutcome {
  path: string;
  iconPath: string | null;
}

export interface IconLibraryEntry {
  name: string;
  path: string;
  base64: string;
}

export type SessionState =
  | "notInitialized"
  | "initializing"
  | "connecting"
  | "connected"
  | "reconnecting"
  | "disconnecting"
  | "disconnected"
  | "failed";

export type DisconnectReason =
  | "localCancel"
  | "remoteUser"
  | "remoteServer"
  | "idleTimeout"
  | "replacedByAnotherConnection"
  | "logoff"
  | "apiDisconnect"
  | "networkLoss"
  | "authenticationFailed"
  | "connectionFailed"
  | "timeout"
  | "noInfo";

export type SessionBackend =
  | "ironRdp"
  | "mstsc"
  | "vnc"
  | "terminal"
  | "localApp"
  | "remoteApp"
  | "fileTransfer"
  | "synthetic";

export interface SessionSnapshot {
  id: string;
  serverId: string;
  serverName: string;
  backend: SessionBackend;
  state: SessionState;
  reason: DisconnectReason | null;
  message: string | null;
  attempt: number;
  maxAttempts: number;
  width: number;
  height: number;
  frameCount: number;
  startedAt: number;
  connectedAt: number | null;
  endedAt: number | null;
  frameRing: string | null;
}

export interface QuickConnectEntry {
  protocol: string;
  address: string;
  port: number;
  username: string;
  lastUsedAt: number;
}

export interface QuickConnectOutcome {
  session: SessionSnapshot;
  saved: boolean;
}

export interface AvailabilityEntry {
  name: string;
  address: string;
  port: number;
  reachable: boolean;
  latencyMs: number | null;
}

export interface VerificationResult {
  verified: boolean;
  supported: boolean;
  message: string | null;
}

export interface FileEntry {
  name: string;
  path: string;
  isDir: boolean;
  size: number;
  modified: number | null;
}

export interface FileListing {
  path: string;
  home: string;
  entries: FileEntry[];
}

export interface FileSessionInfo {
  id: string;
  serverId: string;
  serverName: string;
  protocol: string;
  address: string;
  currentPath: string;
  homePath: string;
}

export type TransferDirection = "upload" | "download";
export type TransferState = "running" | "completed" | "failed" | "cancelled";

export interface TransferSnapshot {
  id: string;
  sessionId: string;
  direction: TransferDirection;
  remotePath: string;
  localPath: string;
  bytes: number;
  total: number;
  state: TransferState;
  message: string | null;
}

export type FileEvent =
  | { kind: "transfer"; transfer: TransferSnapshot }
  | { kind: "closed"; sessionId: string };

export interface SessionFrameImage {
  seq: number;
  width: number;
  height: number;
  dataUrl: string;
}

export type SessionCursor =
  | { kind: "default" }
  | { kind: "hidden" }
  | { kind: "image"; dataUrl: string; hotspotX: number; hotspotY: number };

export type SessionEvent =
  | { kind: "state"; session: SessionSnapshot }
  | { kind: "frame"; sessionId: string; seq: number; width: number; height: number }
  | { kind: "clipboard"; sessionId: string; text: string }
  | { kind: "cursor"; sessionId: string; cursor: SessionCursor };

export interface VaultStatus {
  ready: boolean;
  keyStore: KeyStoreKind | null;
  saltPath: string;
  message: string | null;
}

export interface Checkpoint {
  name: string;
  elapsedMs: number;
}

export interface LocaleInfo {
  id: string;
  englishName: string;
  nativeName: string;
  complete: boolean;
}

export interface ThemeColors {
  primaryMid: string;
  primaryLight: string;
  primaryDark: string;
  primaryText: string;
  accentMid: string;
  accentLight: string;
  accentDark: string;
  accentText: string;
  background: string;
  backgroundText: string;
}

export interface ThemeConfig {
  themeName: string;
  isCustom: boolean;
  colors: ThemeColors;
  fontFamily: string;
  fontSize: number;
}

export interface ResolvedTheme {
  id: string;
  displayName: string;
  colors: ThemeColors;
  fontFamily: string;
  fontSize: number;
}

export interface ThemePreset {
  name: string;
  displayName: string;
  colors: ThemeColors;
}

export interface TabHeaderConfig {
  showIcon: boolean;
  showCloseButton: boolean;
  showReconnectButton: boolean;
  closeButtonOnLeft: boolean;
}

export interface GeneralConfig {
  language: string;
  runAtStartup: boolean;
  closeButtonAction: CloseButtonAction;
  confirmBeforeClosingSession: boolean;
  showSessionIconInTitle: boolean;
  tabHeader: TabHeaderConfig;
  showRecentInTray: boolean;
  showNoteInList: boolean;
  logLevel: LogLevel;
  copyPortWithAddress: boolean;
  moveFocusToLocalOnMouseLeave: boolean;
  doNotCheckForUpdates: boolean;
  includePrereleaseUpdates: boolean;
  updateCheckPeriodHours: number;
  updateFeedUrl: string | null;
  startMinimized: boolean;
  guidanceCompleted: boolean;
  skipLegacyMigration: boolean;
}

export interface LauncherConfig {
  enabled: boolean;
  hotkey: string;
  showNote: boolean;
  showCredentials: boolean;
  saveEnteredInfo: boolean;
  matchCredential: boolean;
  recentLimit: number;
  rememberPosition: boolean;
}

export interface KeywordMatchConfig {
  matchers: MatcherKind[];
  debounceMs: number;
  parallel: boolean;
  autoLocale: boolean;
}

export interface EngagementConfig {
  launches: number;
  firstRunAt: number | null;
  lastUpdateCheckAt: number | null;
  lastRatingPromptAt: number | null;
  doNotShowRating: boolean;
}

export interface Runner {
  id: string;
  name: string;
  protocol: string;
  kind: "internal" | "external" | "externalSsh";
  exePath: string;
  arguments: string;
  parameters: Record<string, string>;
  environment: Record<string, string>;
  runWithHosting: boolean;
  sessionConfigPath: string | null;
  installState: string;
  macros: string[];
}

export interface RunnersConfig {
  runners: Runner[];
  defaults: Record<string, string>;
}

export interface Configuration {
  schemaVersion: number;
  general: GeneralConfig;
  launcher: LauncherConfig;
  runners: RunnersConfig;
  keywordMatch: KeywordMatchConfig;
  theme: ThemeConfig;
  engagement: EngagementConfig;
  databaseCheckPeriod: number;
  databaseReconnectPeriod: number;
  sqliteDatabasePath: string | null;
  pinnedTags: string[];
}

export interface Capabilities {
  os: string;
  arch: string;
  osKeyring: boolean;
  windowsHello: boolean;
  registryAutostart: boolean;
  shortcutFlavor: ShortcutFlavor;
  hwndEmbedding: boolean;
  nativeRdpFallback: boolean;
  embeddedRdp: boolean;
  globalHotkey: boolean;
  tray: boolean;
  workingSetTrim: boolean;
  supportsMultipleMonitors: boolean;
}

export interface LegacyProfile {
  kind: LegacyKind;
  path: string;
  configFile: string | null;
}

export interface LegacySettings {
  source: string;
  importedKeys: string[];
}

export interface ProtocolConfigAsset {
  protocol: string;
  value: unknown;
}

export interface AuxiliaryAssets {
  dataSourcesPresent: boolean;
  dataSources: unknown;
  protocolConfigs: ProtocolConfigAsset[];
  warnings: string[];
}

export interface BootstrapDto {
  app: AppInfo;
  paths: PathsDto;
  firstRun: boolean;
  config: Configuration;
  theme: ResolvedTheme;
  themes: ThemePreset[];
  locales: LocaleInfo[];
  locale: string;
  systemLocale: string;
  capabilities: Capabilities;
  vault: VaultStatus;
  preflight: PreflightReport;
  startup: Checkpoint[];
  database: DatabaseInfo;
  legacy: LegacyProfile | null;
  auxiliary: AuxiliaryAssets;
  protocols: ProtocolDescriptor[];
  recoveredFiles: string[];
  markerLocked: boolean;
  feedbackUrl: string;
  logLevel: LogLevel;
  pendingActions: CliAction[];
  hotkeyError: string | null;
  showRatingPrompt: boolean;
}

export interface UpdateStatus {
  enabled: boolean;
  feedConfigured: boolean;
  current: string;
  checkedAt: number | null;
  available: boolean;
  version: string | null;
  prerelease: boolean;
  breaking: boolean;
  notes: string;
  url: string | null;
  error: string | null;
}

export interface GuidanceSelection {
  profileMode: ProfileMode | null;
  language: string | null;
  themeName: string | null;
  customTheme: ThemeConfig | null;
  runAtStartup: boolean;
  startMinimized: boolean;
  createShortcut: boolean;
  confirmBeforeClosingSession: boolean;
  importLegacySettings: boolean;
}

export interface GuidanceResult {
  config: Configuration;
  restartRequired: boolean;
  migrated: LegacySettings | null;
  shortcutPath: string | null;
  warnings: string[];
}

export interface ProfileChange {
  mode: ProfileMode;
  markerPath: string | null;
  restartRequired: boolean;
}

export interface RecentSessionInfo {
  serverId: string;
  lastConnectedAt: number;
  connectCount: number;
}

export interface CrashReportInfo {
  path: string;
  fileName: string;
  modifiedAt: number | null;
}

export interface CrashReport {
  path: string;
  markdown: string;
}

export interface ThemeValidation {
  theme: ResolvedTheme;
  cssVariables: [string, string][];
}

export interface Locality {
  schemaVersion: number;
  ui: {
    serverView: ServerView;
    serverSort: ServerSort;
    columnWidths: Record<string, number>;
    mainWindow: unknown | null;
    launcherWindow: unknown | null;
    preferLastDisplay: boolean;
    lastActiveTab: string | null;
    tabLayout: string;
    gridMode: boolean;
    showTabIcons: boolean;
    showTabClose: boolean;
    showTabReconnect: boolean;
  };
  session: {
    fullscreenMonitors: Record<string, number>;
    lastTabTokens: Record<string, string>;
    sessionWindows: Record<string, unknown>;
  };
}

export interface ListViewLocality {
  schemaVersion: number;
  sort: ServerSort;
  groupExpanded: Record<string, boolean>;
  customOrder: string[];
  columnWidths: Record<string, number>;
}

export interface TreeViewLocality {
  schemaVersion: number;
  expanded: string[];
  nodeOrder: string[];
}

export interface ScriptResult {
  success: boolean;
  code: number | null;
  stdout: string;
  stderr: string;
}

export interface ConnectionRecords {
  schemaVersion: number;
  records: Record<string, { lastConnectedAt: number; connectCount: number }>;
  quickConnect: QuickConnectEntry[];
}

export type CliAction =
  | { kind: "activate" }
  | { kind: "startMinimized" }
  | { kind: "install" }
  | { kind: "uninstall" }
  | { kind: "installDesktopShortcut" }
  | { kind: "uninstallDesktopShortcut" }
  | { kind: "installStartup" }
  | { kind: "uninstallStartup" }
  | { kind: "connectById"; value: string }
  | { kind: "connectByTag"; value: string }
  | { kind: "connectByName"; value: string };

export interface CliInvocationSummary {
  actions: CliAction[];
  errors: string[];
}
