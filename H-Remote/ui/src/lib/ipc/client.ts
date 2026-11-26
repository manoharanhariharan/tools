import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import type { IpcError } from "./contracts";
import { mockInvoke } from "./mock";
import { useMockBackend } from "./runtime";

export class CommandError extends Error {
  readonly code: string;
  readonly retryable: boolean;

  constructor(error: IpcError) {
    super(error.message);
    this.name = "CommandError";
    this.code = error.code;
    this.retryable = error.retryable;
  }
}

function normalizeError(reason: unknown): CommandError {
  if (reason instanceof CommandError) {
    return reason;
  }
  if (reason && typeof reason === "object" && "message" in reason) {
    const candidate = reason as Partial<IpcError>;
    return new CommandError({
      code: candidate.code ?? "internal",
      message: String(candidate.message),
      retryable: candidate.retryable ?? false,
    });
  }
  return new CommandError({
    code: "internal",
    message: typeof reason === "string" ? reason : "Unexpected backend error",
    retryable: false,
  });
}

export async function invokeCommand<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    if (useMockBackend) {
      return await mockInvoke<T>(command, args);
    }
    return await tauriInvoke<T>(command, args);
  } catch (reason) {
    throw normalizeError(reason);
  }
}

export const commands = {
  bootstrap: () => invokeCommand<import("./contracts").BootstrapDto>("bootstrap"),
  updateConfig: (config: import("./contracts").Configuration) =>
    invokeCommand<import("./contracts").Configuration>("update_config", { config }),
  resolveTheme: (theme: import("./contracts").ThemeConfig) =>
    invokeCommand<import("./contracts").ThemeValidation>("resolve_theme", { theme }),
  getLocales: () =>
    invokeCommand<[import("./contracts").LocaleInfo[], string, string]>("get_locales"),
  getThemes: () => invokeCommand<import("./contracts").ThemePreset[]>("get_themes"),
  completeGuidance: (selection: import("./contracts").GuidanceSelection) =>
    invokeCommand<import("./contracts").GuidanceResult>("complete_guidance", { selection }),
  reportUiError: (message: string, details: string) =>
    invokeCommand<import("./contracts").CrashReport>("report_ui_error", { message, details }),
  reportSessionFailure: (id: string) =>
    invokeCommand<import("./contracts").CrashReport>("report_session_failure", { id }),
  reportConnectionFailure: (serverId: string, message: string) =>
    invokeCommand<import("./contracts").CrashReport>("report_connection_failure", {
      serverId,
      message,
    }),
  saveErrorReport: (markdown: string) =>
    invokeCommand<string>("save_error_report", { markdown }),
  validateHotkey: (hotkey: string) => invokeCommand<string>("validate_hotkey", { hotkey }),
  openLogFolder: () => invokeCommand<void>("open_log_folder"),
  openConfigFolder: () => invokeCommand<void>("open_config_folder"),
  openUrl: (url: string) => invokeCommand<void>("open_url", { url }),
  detectLegacyProfile: () =>
    invokeCommand<import("./contracts").LegacyProfile | null>("detect_legacy_profile"),
  migrateLegacySettings: () =>
    invokeCommand<{ source: string; importedKeys: string[] }>("migrate_legacy_settings"),
  dismissLegacyMigration: () =>
    invokeCommand<import("./contracts").Configuration>("dismiss_legacy_migration"),
  resetMainWindow: () => invokeCommand<void>("reset_main_window"),
  uiReady: () => invokeCommand<void>("ui_ready"),
  installShortcut: () => invokeCommand<string>("install_desktop_shortcut"),
  uninstallShortcut: () => invokeCommand<boolean>("uninstall_desktop_shortcut"),
  installStartup: () => invokeCommand<void>("install_startup"),
  uninstallStartup: () => invokeCommand<void>("uninstall_startup"),
  startupStatus: () => invokeCommand<boolean>("startup_status"),
  readLogTail: (lines = 200) => invokeCommand<string>("read_log_tail", { lines }),
  listCrashReports: () => invokeCommand<import("./contracts").CrashReportInfo[]>("list_crash_reports"),
  readCrashReport: (fileName: string) =>
    invokeCommand<string>("read_crash_report", { fileName }),
  setLogLevel: (level: import("./contracts").LogLevel) =>
    invokeCommand<import("./contracts").Configuration>("set_log_level", { level }),
  i18nReport: () => invokeCommand<import("./contracts").I18nReport>("i18n_report"),
  databaseStatus: () => invokeCommand<import("./contracts").DatabaseInfo>("database_status"),
  repairCatalogSecrets: () =>
    invokeCommand<import("./contracts").SecretRepairReport>("repair_catalog_secrets"),
  checkForUpdate: () =>
    invokeCommand<import("./contracts").UpdateStatus>("check_for_update"),
  updateCheckStatus: () =>
    invokeCommand<import("./contracts").UpdateStatus>("update_check_status"),
  ratingDecision: (decision: "rated" | "later" | "never") =>
    invokeCommand<import("./contracts").Configuration>("rating_decision", { decision }),
  legacyDatabase: () => invokeCommand<string | null>("legacy_database"),
  importLegacyServers: () =>
    invokeCommand<import("./contracts").ImportOutcome>("import_legacy_servers"),
  changeLocalDatabasePath: (path: string | null) =>
    invokeCommand<import("./contracts").DatabaseInfo>("change_local_database_path", { path }),
  dataSourceList: () =>
    invokeCommand<import("./contracts").DataSourceConfig[]>("data_source_list"),
  dataSourceStatuses: () =>
    invokeCommand<import("./contracts").SourceStatus[]>("data_source_statuses"),
  dataSourceSave: (config: import("./contracts").DataSourceConfig, password?: string | null) =>
    invokeCommand<import("./contracts").DataSourceConfig>("data_source_save", {
      config,
      password: password ?? null,
    }),
  dataSourceDelete: (id: string) => invokeCommand<boolean>("data_source_delete", { id }),
  dataSourceTest: (config: import("./contracts").DataSourceConfig, password?: string | null) =>
    invokeCommand<import("./contracts").SourceStatus>("data_source_test", {
      config,
      password: password ?? null,
    }),
  dataSourceRefresh: (id: string) =>
    invokeCommand<import("./contracts").SourceStatus>("data_source_refresh", { id }),
  dataSourceRefreshAll: () =>
    invokeCommand<import("./contracts").SourceStatus[]>("data_source_refresh_all"),
  vaultStatus: () => invokeCommand<import("./contracts").VaultStatus>("vault_status"),
  startupCheckpoints: () => invokeCommand<import("./contracts").Checkpoint[]>("startup_checkpoints"),
  setPinnedTags: (tags: string[]) =>
    invokeCommand<unknown>("set_pinned_tags", { tags }),
  setTagOrder: (order: string[]) =>
    invokeCommand<unknown>("set_tag_order", { order }),
  catalogSnapshot: () => invokeCommand<import("./contracts").CatalogSnapshot>("catalog_snapshot"),
  newServer: (protocol: string) =>
    invokeCommand<import("./contracts").Server>("new_server", { protocol }),
  getLocality: () => invokeCommand<import("./contracts").Locality>("get_locality"),
  setServerView: (view: "card" | "list" | "tree") =>
    invokeCommand<void>("set_server_view", { view }),
  getConnectionRecords: () =>
    invokeCommand<import("./contracts").ConnectionRecords>("get_connection_records"),
  catalogView: (request: {
    query: string;
    providers: string[];
    sort: string;
    customOrder: string[];
    folderOrder: string[];
    source?: string;
  }) =>
    invokeCommand<import("./contracts").CatalogView>("catalog_view", {
      query: request.query,
      providers: request.providers,
      sort: request.sort,
      customOrder: request.customOrder,
      folderOrder: request.folderOrder,
      source: request.source ?? null,
    }),
  saveServer: (server: import("./contracts").Server) =>
    invokeCommand<import("./contracts").Server>("save_server", { server }),
  saveServers: (servers: import("./contracts").Server[]) =>
    invokeCommand<import("./contracts").Server[]>("save_servers", { servers }),
  deleteServer: (id: string) => invokeCommand<boolean>("delete_server", { id }),
  listCredentials: () => invokeCommand<import("./contracts").Credential[]>("list_credentials"),
  saveCredential: (credential: import("./contracts").Credential) =>
    invokeCommand<import("./contracts").Credential>("save_credential", { credential }),
  deleteCredential: (id: string) => invokeCommand<boolean>("delete_credential", { id }),
  createCredential: (name: string, username: string, password: string) =>
    invokeCommand<import("./contracts").Credential>("create_credential", {
      name,
      username,
      password,
    }),
  createFolder: (path: string[]) =>
    invokeCommand<import("./contracts").Folder>("create_folder", { path }),
  ensureFolder: (path: string[]) =>
    invokeCommand<import("./contracts").Folder>("ensure_folder", { path }),
  renameFolder: (folderId: string, newName: string) =>
    invokeCommand<number>("rename_folder", { folderId, newName }),
  deleteFolder: (path: string[]) =>
    invokeCommand<[number, number]>("delete_folder", { path }),
  moveFolder: (folderId: string, parent: string[]) =>
    invokeCommand<number>("move_folder", { folderId, parent }),
  setFolderColor: (folderId: string, colorHex: string) =>
    invokeCommand<import("./contracts").Folder>("set_folder_color", { folderId, colorHex }),
  duplicateFolder: (folderId: string) =>
    invokeCommand<import("./contracts").Folder>("duplicate_folder", { folderId }),
  moveServerToFolder: (serverId: string, path: string[]) =>
    invokeCommand<import("./contracts").Server>("move_server_to_folder", { serverId, path }),
  getListView: () => invokeCommand<import("./contracts").ListViewLocality>("get_list_view"),
  updateListView: (view: import("./contracts").ListViewLocality) =>
    invokeCommand<import("./contracts").ListViewLocality>("update_list_view", { view }),
  getTreeView: () => invokeCommand<import("./contracts").TreeViewLocality>("get_tree_view"),
  updateTreeView: (view: import("./contracts").TreeViewLocality) =>
    invokeCommand<import("./contracts").TreeViewLocality>("update_tree_view", { view }),
  setCustomOrder: (ids: string[]) =>
    invokeCommand<import("./contracts").ListViewLocality>("set_custom_order", { ids }),
  exportServersJson: (ids: string[]) =>
    invokeCommand<string>("export_servers_json", { ids }),
  exportRdpServer: (id: string) =>
    invokeCommand<import("./contracts").RdpExport>("export_rdp_server", { id }),
  importJson: (content: string, target = "local") =>
    invokeCommand<import("./contracts").ImportOutcome>("import_json_content", {
      content,
      target,
    }),
  importCsv: (content: string, target = "local") =>
    invokeCommand<import("./contracts").ImportOutcome>("import_csv_content", {
      content,
      target,
    }),
  importRdp: (
    content: string,
    displayName: string,
    password: string | null = null,
    target = "local",
  ) =>
    invokeCommand<import("./contracts").ImportOutcome>("import_rdp_content", {
      content,
      displayName,
      password,
      target,
    }),
  importDatabase: (path: string, target = "local") =>
    invokeCommand<import("./contracts").ImportOutcome>("import_database_file", {
      path,
      target,
    }),
  createShortcutForServers: (ids: string[], name: string, startMinimized: boolean) =>
    invokeCommand<import("./contracts").ShortcutOutcome>("create_shortcut_for_servers", {
      ids,
      name,
      startMinimized,
    }),
  createShortcutForTag: (tag: string, name: string, startMinimized: boolean) =>
    invokeCommand<import("./contracts").ShortcutOutcome>("create_shortcut_for_tag", {
      tag,
      name,
      startMinimized,
    }),
  saveIconLibrary: (base64Value: string) =>
    invokeCommand<string>("save_icon_to_library", { base64Value }),
  listIconLibrary: () =>
    invokeCommand<import("./contracts").IconLibraryEntry[]>("list_icon_library"),
  deleteIconLibrary: (path: string) =>
    invokeCommand<boolean>("delete_icon_from_library", { path }),
  fetchIcon: (url: string) => invokeCommand<string>("fetch_icon_from_url", { url }),
  testScript: (script: string, hideWindow: boolean, serverId?: string | null) =>
    invokeCommand<import("./contracts").ScriptResult>("test_script", {
      script,
      hideWindow,
      serverId: serverId ?? null,
    }),
  connectServer: (
    id: string,
    viewport?: [number, number],
    deviceScale?: number,
    credentials?: {
      username?: string;
      password?: string;
      privateKey?: string;
      alternateName?: string;
    },
  ) =>
    invokeCommand<import("./contracts").SessionSnapshot>("connect_server", {
      id,
      viewport,
      deviceScale,
      username: credentials?.username ?? null,
      password: credentials?.password ?? null,
      privateKey: credentials?.privateKey ?? null,
      alternateName: credentials?.alternateName ?? null,
    }),
  sessionList: () =>
    invokeCommand<import("./contracts").SessionSnapshot[]>("session_list"),
  sessionSnapshot: (id: string) =>
    invokeCommand<import("./contracts").SessionSnapshot>("session_snapshot", { id }),
  sessionCancel: (id: string) =>
    invokeCommand<void>("session_cancel", { id }),
  sessionReconnect: (id: string) =>
    invokeCommand<import("./contracts").SessionSnapshot>("session_reconnect", { id }),
  sessionRemove: (id: string) =>
    invokeCommand<boolean>("session_remove", { id }),
  sessionResize: (id: string, width: number, height: number, scalePercent: number) =>
    invokeCommand<void>("session_resize", { id, width, height, scalePercent }),
  sessionCtrlAltDel: (id: string) =>
    invokeCommand<void>("session_send_ctrl_alt_del", { id }),
  sessionSendSpecial: (id: string, special: string) =>
    invokeCommand<void>("session_send_special", { id, special }),
  sessionVncKey: (id: string, keycode: number, down: boolean) =>
    invokeCommand<void>("session_vnc_key", { id, keycode, down }),
  sessionVncPointer: (id: string, x: number, y: number, buttons: number) =>
    invokeCommand<void>("session_vnc_pointer", { id, x, y, buttons }),
  sessionVncRefresh: (id: string) =>
    invokeCommand<void>("session_vnc_refresh", { id }),
  sessionVncText: (id: string, text: string) =>
    invokeCommand<void>("session_vnc_text", { id, text }),
  sessionRdpKey: (id: string, code: string, key: string, down: boolean) =>
    invokeCommand<void>("session_rdp_key", { id, code, key, down }),
  sessionRdpPointerMove: (id: string, x: number, y: number) =>
    invokeCommand<void>("session_rdp_pointer_move", { id, x, y }),
  sessionRdpPointerButton: (
    id: string,
    x: number,
    y: number,
    button: number,
    down: boolean,
  ) =>
    invokeCommand<void>("session_rdp_pointer_button", { id, x, y, button, down }),
  sessionRdpWheel: (
    id: string,
    x: number,
    y: number,
    vertical: boolean,
    units: number,
  ) => invokeCommand<void>("session_rdp_wheel", { id, x, y, vertical, units }),
  getRunners: () =>
    invokeCommand<import("./contracts").RunnersConfig>("get_runners"),
  saveRunner: (runner: import("./contracts").Runner) =>
    invokeCommand<import("./contracts").RunnersConfig>("save_runner", { runner }),
  deleteRunner: (id: string) =>
    invokeCommand<import("./contracts").RunnersConfig>("delete_runner", { id }),
  setDefaultRunner: (protocol: string, runnerId: string) =>
    invokeCommand<import("./contracts").RunnersConfig>("set_default_runner", {
      protocol,
      runnerId,
    }),
  sessionFrameRing: (id: string) =>
    invokeCommand<string | null>("session_frame_ring", { id }),
  sessionFrameImage: (id: string, since?: number | null) =>
    invokeCommand<import("./contracts").SessionFrameImage | null>(
      "session_frame_image",
      { id, since },
    ),
  openSessionWindow: (sessionId: string, placement?: unknown) =>
    invokeCommand<string>("open_session_window", { sessionId, placement }),
  setWindowTitle: (title: string) =>
    invokeCommand<void>("set_window_title", { title }),
  updateTabWorkspace: (request: {
    layout: string;
    gridMode: boolean;
    showIcons: boolean;
    showClose: boolean;
    showReconnect: boolean;
  }) =>
    invokeCommand<import("./contracts").Locality>("update_tab_workspace", request),
  showLauncher: () => invokeCommand<void>("show_launcher"),
  focusServerInCatalog: (serverId: string) =>
    invokeCommand<void>("focus_server_in_catalog", { serverId }),
  hideLauncher: () => invokeCommand<void>("hide_launcher"),
  showMainWindow: () => invokeCommand<void>("show_main_window"),
  resizeLauncher: (height: number) =>
    invokeCommand<void>("resize_launcher", { height }),
  quickConnect: (request: {
    protocol: string;
    address: string;
    port?: number;
    username: string;
    password?: string;
    save: boolean;
    matchCredential: boolean;
  }) =>
    invokeCommand<import("./contracts").QuickConnectOutcome>("quick_connect", request),
  quickConnectHistory: () =>
    invokeCommand<import("./contracts").QuickConnectEntry[]>("quick_connect_history"),
  checkServerAvailability: (id: string) =>
    invokeCommand<import("./contracts").AvailabilityEntry[]>("check_server_availability", {
      id,
    }),
  verifySensitive: (action: string, force = false) =>
    invokeCommand<import("./contracts").VerificationResult>("verify_sensitive_action", {
      action,
      force,
    }),
  verificationStatus: () => invokeCommand<number>("verification_status"),
  fileList: (sessionId: string, path?: string) =>
    invokeCommand<import("./contracts").FileListing>("file_list", { sessionId, path }),
  fileRename: (sessionId: string, from: string, to: string) =>
    invokeCommand<void>("file_rename", { sessionId, from, to }),
  fileDelete: (sessionId: string, path: string, isDir: boolean) =>
    invokeCommand<void>("file_delete", { sessionId, path, isDir }),
  fileMkdir: (sessionId: string, path: string) =>
    invokeCommand<void>("file_mkdir", { sessionId, path }),
  fileUpload: (sessionId: string, localPaths: string[], remoteDir: string) =>
    invokeCommand<import("./contracts").TransferSnapshot[]>("file_upload", {
      sessionId,
      localPaths,
      remoteDir,
    }),
  fileDownload: (sessionId: string, remotePaths: string[], localDir: string) =>
    invokeCommand<import("./contracts").TransferSnapshot[]>("file_download", {
      sessionId,
      remotePaths,
      localDir,
    }),
  fileUploadDialog: (sessionId: string, remoteDir: string) =>
    invokeCommand<import("./contracts").TransferSnapshot[]>("file_upload_dialog", {
      sessionId,
      remoteDir,
    }),
  fileDownloadDialog: (sessionId: string, remotePaths: string[]) =>
    invokeCommand<import("./contracts").TransferSnapshot[]>("file_download_dialog", {
      sessionId,
      remotePaths,
    }),
  fileTransfers: () =>
    invokeCommand<import("./contracts").TransferSnapshot[]>("file_transfers"),
  fileCancelTransfer: (transferId: string) =>
    invokeCommand<boolean>("file_cancel_transfer", { transferId }),
  trimWorkingSet: () => invokeCommand<import("./contracts").TrimOutcome>("trim_working_set"),
  flashWindow: () => invokeCommand<void>("flash_window"),
};
