import type {
  BootstrapDto,
  Configuration,
  GuidanceResult,
  GuidanceSelection,
  IpcError,
  ResolvedTheme,
  ThemeConfig,
  UpdateStatus,
} from "../ipc/contracts";
import { CommandError, commands } from "../ipc/client";
import { CleanupRegistry, subscribe } from "../ipc/events";
import { i18n } from "../i18n/index.svelte";
import { applyTheme } from "../theme/apply";
import { busy } from "./busy.svelte";

export type Route = "servers" | "settings" | "about";
export type NoticeKind = "info" | "warning" | "error";

export interface Notice {
  kind: NoticeKind;
  text: string;
}

export interface ErrorReportState {
  message: string;
  details: string;
  markdown: string;
  path: string | null;
}

class AppStore {
  loading = $state(true);
  bootstrap = $state<BootstrapDto | null>(null);
  config = $state<Configuration | null>(null);
  theme = $state<ResolvedTheme | null>(null);
  route = $state<Route>("servers");
  settingsSection = $state("general");
  guidanceOpen = $state(false);
  fatalError = $state<string | null>(null);
  errorReport = $state<ErrorReportState | null>(null);
  notice = $state<Notice | null>(null);
  pendingConnection = $state<string | null>(null);
  updateStatus = $state<UpdateStatus | null>(null);
  updateDialogOpen = $state(false);
  ratingOpen = $state(false);
  legacyOpen = $state(false);
  private updateNotified = false;
  private dismissedErrors = new Set<string>();

  async init(registry: CleanupRegistry): Promise<void> {
    this.registerEvents(registry);
    try {
      const bootstrap = await commands.bootstrap();
      this.bootstrap = bootstrap;
      this.config = bootstrap.config;
      this.theme = bootstrap.theme;
      applyTheme(bootstrap.theme);
      i18n.setLocale(bootstrap.locale || bootstrap.systemLocale);
      this.guidanceOpen = bootstrap.firstRun;
      if (bootstrap.recoveredFiles.length > 0) {
        this.showNotice(
          `Recovered ${bootstrap.recoveredFiles.length} corrupt profile file(s); defaults were used.`,
          "warning",
        );
      }
      if (bootstrap.hotkeyError) {
        this.showNotice(bootstrap.hotkeyError, "warning");
      }
      this.ratingOpen = bootstrap.showRatingPrompt && !bootstrap.firstRun;
      this.legacyOpen =
        Boolean(bootstrap.legacy) && !bootstrap.config.general.skipLegacyMigration;
      void this.maybeCheckForUpdates();
      await commands.uiReady();
    } catch (error) {
      this.fatalError = messageOf(error);
    } finally {
      this.loading = false;
    }
  }

  async applyConfig(next: Configuration): Promise<void> {
    this.config = next;
    try {
      const validation = await commands.resolveTheme(next.theme);
      this.theme = validation.theme;
      applyTheme(validation.theme);
    } catch (error) {
      this.showNotice(messageOf(error), "error");
    }
    const locale =
      next.general.language === "system"
        ? (this.bootstrap?.systemLocale ?? "en-US")
        : next.general.language;
    i18n.setLocale(locale);
  }

  async patchConfig(
    mutate: (draft: Configuration) => void,
  ): Promise<Configuration | null> {
    if (!this.config) {
      return null;
    }
    const draft = JSON.parse(JSON.stringify(this.config)) as Configuration;
    mutate(draft);
    try {
      const updated = await commands.updateConfig(draft);
      await this.applyConfig(updated);
      return updated;
    } catch (error) {
      await this.reportError(error);
      return null;
    }
  }

  async applyThemeConfig(theme: ThemeConfig): Promise<ResolvedTheme | null> {
    try {
      const validation = await commands.resolveTheme(theme);
      applyTheme(validation.theme);
      return validation.theme;
    } catch (error) {
      this.showNotice(messageOf(error), "error");
      return null;
    }
  }

  /** Automatic check honouring the opt-out and configured period (P9-10). */
  async maybeCheckForUpdates(): Promise<void> {
    const config = this.config;
    if (!config) {
      return;
    }
    if (config.general.doNotCheckForUpdates || !config.general.updateFeedUrl) {
      return;
    }
    if (config.general.updateCheckPeriodHours <= 0) {
      return;
    }
    const now = Math.floor(Date.now() / 1000);
    const last = config.engagement.lastUpdateCheckAt ?? 0;
    if (now - last < config.general.updateCheckPeriodHours * 3600) {
      return;
    }
    await this.checkForUpdates(false);
  }

  async checkForUpdates(showDialog = true): Promise<UpdateStatus | null> {
    try {
      const status = await busy.track("", () => commands.checkForUpdate());
      this.updateStatus = status;
      if (status.available && showDialog) {
        this.updateDialogOpen = true;
        this.updateNotified = true;
      }
      return status;
    } catch (error) {
      await this.reportError(error);
      return null;
    }
  }

  openUpdateDialog(): void {
    if (this.updateStatus?.available) {
      this.updateDialogOpen = true;
    }
  }

  dismissUpdate(): void {
    this.updateDialogOpen = false;
  }

  async decideRating(decision: "rated" | "later" | "never"): Promise<void> {
    try {
      const updated = await commands.ratingDecision(decision);
      if (updated) {
        await this.applyConfig(updated);
      }
    } catch (error) {
      await this.reportError(error);
    } finally {
      this.ratingOpen = false;
    }
  }

  async dismissLegacyMigration(): Promise<void> {
    this.legacyOpen = false;
    try {
      const updated = await commands.dismissLegacyMigration();
      if (updated) {
        await this.applyConfig(updated);
      }
    } catch (error) {
      await this.reportError(error);
    }
  }

  async importLegacySettings(): Promise<void> {
    try {
      const result = await commands.migrateLegacySettings();
      this.showNotice(`Imported ${result.importedKeys.length} setting(s).`, "info");
      this.legacyOpen = false;
    } catch (error) {
      await this.reportError(error);
    }
  }

  async importLegacyServers(): Promise<void> {
    try {
      const result = await commands.importLegacyServers();
      this.showNotice(`${result.imported} server(s) imported.`, "info");
      this.legacyOpen = false;
    } catch (error) {
      await this.reportError(error);
    }
  }

  async finishGuidance(selection: GuidanceSelection): Promise<GuidanceResult | null> {    try {
      const result = await busy.track("", () => commands.completeGuidance(selection));
      await this.applyConfig(result.config);
      this.guidanceOpen = false;
      if (result.restartRequired) {
        this.showNotice("Profile mode changed. Restart H-Remote to apply it.", "warning");
      }
      for (const warning of result.warnings) {
        this.showNotice(warning, "warning");
      }
      return result;
    } catch (error) {
      await this.reportError(error);
      return null;
    }
  }

  showNotice(text: string, kind: NoticeKind = "info"): void {
    this.notice = { kind, text };
  }

  dismissNotice(): void {
    this.notice = null;
  }

  async ensureVerified(action: string): Promise<boolean> {
    try {
      const result = await commands.verifySensitive(action);
      if (result.verified) {
        return true;
      }
      if (!result.supported) {
        const { dialogs } = await import("./dialogs.svelte");
        return dialogs.confirm({
          title: "Verification unavailable",
          message:
            result.message ??
            "Secondary verification is unavailable on this platform. Continue anyway?",
        });
      }
      if (result.message) {
        this.showNotice(result.message, "warning");
      }
      return false;
    } catch (reason) {
      await this.reportError(reason);
      return false;
    }
  }

  async reportError(reason: unknown, details?: string): Promise<void> {
    const message = messageOf(reason);
    // A recurring error (for example input racing with teardown) must not
    // reopen the modal after the user dismissed it.
    if (this.errorReport?.message === message || this.dismissedErrors.has(message)) {
      return;
    }
    const technical =
      details ??
      (reason instanceof Error ? (reason.stack ?? reason.message) : JSON.stringify(reason, null, 2));
    try {
      const report = await commands.reportUiError(message, technical);
      this.errorReport = {
        message,
        details: technical,
        markdown: report.markdown,
        path: report.path,
      };
    } catch {
      this.errorReport = { message, details: technical, markdown: "", path: null };
    }
  }

  dismissError(): void {
    if (this.errorReport) {
      this.dismissedErrors.add(this.errorReport.message);
    }
    this.errorReport = null;
  }

  openSettings(section = "general"): void {
    this.settingsSection = section;
    this.route = "settings";
  }

  setRoute(route: Route): void {
    this.route = route;
  }

  resetForTests(): void {
    this.loading = false;
    this.bootstrap = null;
    this.config = null;
    this.theme = null;
    this.route = "servers";
    this.settingsSection = "general";
    this.guidanceOpen = false;
    this.fatalError = null;
    this.errorReport = null;
    this.notice = null;
    this.pendingConnection = null;
    this.updateStatus = null;
    this.updateDialogOpen = false;
    this.ratingOpen = false;
    this.legacyOpen = false;
    this.updateNotified = false;
    this.dismissedErrors.clear();
    i18n.setLocale("en-US");
  }

  private registerEvents(registry: CleanupRegistry): void {
    subscribe<Configuration>("config:changed", (config) => {
      void this.applyConfig(config);
    }, registry);

    subscribe<string>("ui:navigate", (target) => {
      if (target === "about" || target === "settings" || target === "servers") {
        this.setRoute(target);
      }
    }, registry);

    subscribe<{ kind: string; value?: string }>("activation:connect", (action) => {
      this.pendingConnection = action.value ?? action.kind;
      this.route = "servers";
      void import("./sessions.svelte").then(({ sessions }) =>
        sessions.connectActivation(action),
      );
    }, registry);

    subscribe<string>("hotkey:error", (message) => {
      this.showNotice(message, "error");
    }, registry);

    subscribe<UpdateStatus>("update:available", (status) => {
      this.updateStatus = status;
      if (!this.updateNotified) {
        this.updateNotified = true;
        this.updateDialogOpen = true;
      }
    }, registry);

    subscribe<string>("catalog:edit-server", (serverId) => {
      this.route = "servers";
      void import("./catalog.svelte").then(async ({ catalog }) => {
        if (!catalog.snapshot) {
          await catalog.load();
        }
        const server = catalog.snapshot?.servers.find(
          (candidate) => candidate.id === serverId,
        );
        if (server) {
          catalog.openEdit(server);
        }
      });
    }, registry);

    subscribe<string[]>("integration:warnings", (warnings) => {      for (const warning of warnings) {
        this.showNotice(warning, "warning");
      }
    }, registry);

    subscribe<string[]>("cli:errors", (errors) => {
      for (const error of errors) {
        this.showNotice(error, "warning");
      }
    }, registry);

    if (typeof window !== "undefined") {
      const onError = (event: ErrorEvent) => {
        void this.reportError(event.error ?? event.message, event.error?.stack);
      };
      const onRejection = (event: PromiseRejectionEvent) => {
        void this.reportError(event.reason);
      };
      window.addEventListener("error", onError);
      window.addEventListener("unhandledrejection", onRejection);
      registry.add(() => {
        window.removeEventListener("error", onError);
        window.removeEventListener("unhandledrejection", onRejection);
      });
    }
  }
}

function messageOf(reason: unknown): string {
  if (reason instanceof CommandError) {
    return reason.message;
  }
  if (reason instanceof Error) {
    return reason.message;
  }
  if (reason && typeof reason === "object" && "message" in reason) {
    return String((reason as IpcError).message);
  }
  return String(reason);
}

export const app = new AppStore();
