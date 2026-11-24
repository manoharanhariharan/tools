<script lang="ts">
  import { onMount } from "svelte";
  import { i18n } from "./lib/i18n/index.svelte";
  import { CleanupRegistry } from "./lib/ipc/events";
  import { commands } from "./lib/ipc/client";
  import { isTauriRuntime } from "./lib/ipc/runtime";
  import { app } from "./lib/state/app.svelte";
  import BusyOverlay from "./lib/components/BusyOverlay.svelte";
  import DialogHost from "./lib/components/DialogHost.svelte";
  import NoticeBar from "./lib/components/NoticeBar.svelte";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import TitleBar from "./lib/components/TitleBar.svelte";
  import AboutPage from "./lib/features/about/AboutPage.svelte";
  import LauncherPanel from "./lib/features/launcher/LauncherPanel.svelte";
  import SessionWorkspace from "./lib/features/session/SessionWorkspace.svelte";
  import AvailabilityDialog from "./lib/features/session/AvailabilityDialog.svelte";
  import SessionDisplayDialog from "./lib/features/session/SessionDisplayDialog.svelte";
  import UpdateDialog from "./lib/features/updates/UpdateDialog.svelte";
  import RatingDialog from "./lib/features/updates/RatingDialog.svelte";
  import LegacyMigrationDialog from "./lib/features/updates/LegacyMigrationDialog.svelte";
  import { sessions } from "./lib/state/sessions.svelte";

  const params = new URLSearchParams(window.location.search);
  const isLauncherWindow = params.get("window") === "launcher";
  const isSessionWindow = params.get("window") === "sessions";
  import CatalogPage from "./lib/features/catalog/CatalogPage.svelte";
  import ErrorReportDialog from "./lib/features/errors/ErrorReportDialog.svelte";
  import GuidanceWizard from "./lib/features/guidance/GuidanceWizard.svelte";
  import SettingsPage from "./lib/features/settings/SettingsPage.svelte";

  onMount(() => {
    const instance = new CleanupRegistry();
    void app.init(instance);
    if (isSessionWindow || !isTauriRuntime) {
      void sessions.load();
    }
    return () => {
      sessions.dispose();
      instance.dispose();
    };
  });

  function retry() {
    if (typeof window !== "undefined") {
      window.location.reload();
    }
  }

  function openLogs() {
    void commands.openLogFolder().catch((error) => app.reportError(error));
  }
</script>

{#if app.loading}
  <div class="boot" role="status" aria-live="polite">
    <div class="boot-mark" aria-hidden="true"><span>H</span></div>
    <h1>H-Remote</h1>
    <p>{i18n.t("common.loading")}</p>
  </div>
{:else if app.fatalError}
  <div class="fatal">
    <div class="fatal-card">
      <h1>{i18n.t("error.title")}</h1>
      <p>{app.fatalError}</p>
      <div class="hr-row">
        <button type="button" class="retry" onclick={retry}>{i18n.t("common.retry")}</button>
        <button type="button" class="logs" onclick={openLogs}>{i18n.t("error.openLogs")}</button>
      </div>
    </div>
  </div>
{:else if isLauncherWindow}
  <LauncherPanel />
{:else if isSessionWindow}
  {#if app.bootstrap && app.config}
    <SessionWorkspace standalone />
    <SessionDisplayDialog />
    <BusyOverlay />
    <DialogHost />
    <ErrorReportDialog />
    <NoticeBar />
  {/if}
{:else if app.bootstrap && app.config}
  <div class="hr-shell">
    <TitleBar
      title={i18n.t("app.name")}
      subtitle={i18n.t("app.tagline")}
      version={app.bootstrap.app.version}
    />

    <div class="hr-content">
      <Sidebar
        route={app.route}
        appInfo={app.bootstrap.app}
        paths={app.bootstrap.paths}
        onnavigate={(route) => app.setRoute(route)}
      />

      <main class="hr-main" class:catalog-main={app.route === "servers"} id="main-content">
        {#if app.route === "settings"}
          <SettingsPage />
        {:else if app.route === "about"}
          <AboutPage />
        {:else}
          <CatalogPage />
        {/if}
      </main>
    </div>
  </div>

  {#if app.guidanceOpen}
    <GuidanceWizard
      bootstrap={app.bootstrap}
      onfinish={(selection) => app.finishGuidance(selection)}
    />
  {/if}

  {#if !isTauriRuntime && sessions.activeId}
    <SessionWorkspace />
  {/if}

  {#if sessions.availability}
    <AvailabilityDialog entries={sessions.availability} />
  {/if}

  <SessionDisplayDialog />

  {#if !app.guidanceOpen}
    <UpdateDialog />
    <RatingDialog />
    <LegacyMigrationDialog />
  {/if}

  <BusyOverlay />
  <DialogHost />
  <ErrorReportDialog />
  <NoticeBar />
{/if}

<style>
  .boot,
  .fatal {
    height: 100%;
    display: grid;
    place-items: center;
    padding: 32px;
    background:
      radial-gradient(
        circle at 30% 20%,
        color-mix(in srgb, var(--hr-accent-dark) 30%, transparent),
        transparent 55%
      ),
      var(--hr-background);
  }

  .boot {
    align-content: center;
    text-align: center;
    gap: 10px;
  }

  .boot-mark {
    width: 76px;
    height: 76px;
    margin: 0 auto 6px;
    border-radius: 22px;
    display: grid;
    place-items: center;
    background: linear-gradient(145deg, var(--hr-accent-light), var(--hr-accent-dark));
    box-shadow: 0 20px 60px color-mix(in srgb, var(--hr-accent-mid) 35%, transparent);
  }

  .boot-mark span {
    font-size: 34px;
    font-weight: 700;
    color: var(--hr-accent-text);
  }

  .boot h1 {
    font-size: 24px;
    letter-spacing: 0.02em;
  }

  .boot p {
    color: color-mix(in srgb, var(--hr-background-text) 62%, transparent);
  }

  .fatal-card {
    width: min(560px, 100%);
    padding: 26px;
    border-radius: var(--hr-radius-large);
    border: 1px solid var(--hr-border-strong);
    background: color-mix(in srgb, var(--hr-primary-mid) 80%, var(--hr-background));
    box-shadow: var(--hr-shadow);
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .fatal-card p {
    color: var(--hr-danger);
    line-height: 1.45;
    word-break: break-word;
  }

  .catalog-main {
    padding: 0;
    overflow: hidden;
    display: flex;
  }

  .catalog-main :global(.catalog-page) {
    flex: 1;
  }

  .retry,
  .logs {
    min-height: 36px;
    padding: 0 16px;
    border-radius: var(--hr-radius);
    border: 1px solid var(--hr-border);
    background: transparent;
    color: inherit;
  }

  .retry {
    background: var(--hr-accent-mid);
    border-color: var(--hr-accent-mid);
    color: var(--hr-accent-text);
  }
</style>
