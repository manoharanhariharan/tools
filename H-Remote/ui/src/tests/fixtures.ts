import type { BootstrapDto, ResolvedTheme } from "../lib/ipc/contracts";
import {
  defaultConfiguration,
  defaultThemeConfig,
  mockPresets,
} from "../lib/ipc/mock";

export function bootstrapFixture(overrides: Partial<BootstrapDto> = {}): BootstrapDto {
  const config = defaultConfiguration();
  const preset = mockPresets.find((candidate) => candidate.name === "Dark")!;
  const theme: ResolvedTheme = {
    id: preset.name,
    displayName: preset.displayName,
    colors: preset.colors,
    fontFamily: config.theme.fontFamily,
    fontSize: config.theme.fontSize,
  };

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
    firstRun: true,
    config,
    theme,
    themes: mockPresets,
    locales: [
      { id: "en-US", englishName: "English (United States)", nativeName: "English", complete: true },
      { id: "de-DE", englishName: "German", nativeName: "Deutsch", complete: false },
      { id: "fr-FR", englishName: "French", nativeName: "Français", complete: false },
    ],
    locale: "en-US",
    systemLocale: "en-US",
    capabilities: {
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
      tray: true,
      workingSetTrim: true,
      supportsMultipleMonitors: true,
    },
    vault: {
      ready: true,
      keyStore: "osKeyring",
      saltPath: "C:\\H-Remote\\.locality\\.encryption_salt",
      message: null,
    },
    preflight: { ok: true, checks: [] },
    startup: [{ name: "process", elapsedMs: 0 }],
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
    protocols: [],
    recoveredFiles: [],
    markerLocked: false,
    feedbackUrl: "https://example.invalid/issues",
    logLevel: "warn",
    pendingActions: [],
    hotkeyError: null,
    showRatingPrompt: false,
    ...overrides,
  };
}
