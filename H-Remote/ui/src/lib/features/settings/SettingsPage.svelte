<script lang="ts">
  import { i18n } from "../../i18n/index.svelte";
  import { app } from "../../state/app.svelte";
  import GeneralSettings from "./GeneralSettings.svelte";
  import AppearanceSettings from "./AppearanceSettings.svelte";
  import LauncherSettings from "./LauncherSettings.svelte";
  import DataSettings from "./DataSettings.svelte";
  import RunnersSettings from "./RunnersSettings.svelte";
  import CredentialsSettings from "./CredentialsSettings.svelte";
  import DataSourcesSettings from "./DataSourcesSettings.svelte";

  const sections = [
    { id: "general", label: "settings.general" },
    { id: "appearance", label: "settings.appearance" },
    { id: "launcher", label: "settings.launcher" },
    { id: "runners", label: "settings.runners" },
    { id: "credentials", label: "settings.credentials" },
    { id: "sources", label: "settings.sources" },
    { id: "data", label: "settings.data" },
  ] as const;

  function select(id: string) {
    app.settingsSection = id;
  }
</script>

<div class="hr-page hr-fade-in">
  <header class="hr-page-header">
    <div>
      <h1>{i18n.t("settings.title")}</h1>
      <p>{i18n.t("settings.saved")} · {app.bootstrap?.paths.profileMode}</p>
    </div>
  </header>

  <div class="settings-frame">
    <div class="tabs" role="tablist" aria-label={i18n.t("settings.title")}>
      {#each sections as section (section.id)}
        <button
          type="button"
          role="tab"
          aria-selected={app.settingsSection === section.id}
          class:active={app.settingsSection === section.id}
          data-testid={`settings-tab-${section.id}`}
          onclick={() => select(section.id)}
        >
          {i18n.t(section.label)}
        </button>
      {/each}
    </div>

    <section class="panel" role="tabpanel">
      {#if app.config && app.bootstrap}
        {#if app.settingsSection === "general"}
          <GeneralSettings
            config={app.config}
            locales={app.bootstrap.locales}
            systemLocale={app.bootstrap.systemLocale}
            onpatch={(mutate) => app.patchConfig(mutate)}
          />
        {:else if app.settingsSection === "appearance"}
          <AppearanceSettings
            config={app.config}
            theme={app.theme}
            themes={app.bootstrap.themes}
            onpatch={(mutate) => app.patchConfig(mutate)}
          />
        {:else if app.settingsSection === "launcher"}
          <LauncherSettings
            config={app.config}
            onpatch={(mutate) => app.patchConfig(mutate)}
          />
        {:else if app.settingsSection === "runners"}
          <RunnersSettings />
        {:else if app.settingsSection === "credentials"}
          <CredentialsSettings />
        {:else if app.settingsSection === "sources"}
          <DataSourcesSettings />
        {:else}
          <DataSettings bootstrap={app.bootstrap} />
        {/if}
      {/if}
    </section>
  </div>
</div>

<style>
  .settings-frame {
    display: grid;
    grid-template-columns: 180px 1fr;
    gap: 18px;
    align-items: start;
  }

  .tabs {
    display: flex;
    flex-direction: column;
    gap: 4px;
    position: sticky;
    top: 0;
  }

  .tabs button {
    text-align: left;
    border: 1px solid transparent;
    background: transparent;
    color: color-mix(in srgb, var(--hr-background-text) 68%, transparent);
    padding: 9px 12px;
    border-radius: var(--hr-radius);
    transition:
      background var(--hr-transition),
      color var(--hr-transition);
  }

  .tabs button:hover {
    background: color-mix(in srgb, var(--hr-primary-text) 8%, transparent);
  }

  .tabs button.active {
    background: color-mix(in srgb, var(--hr-accent-mid) 16%, transparent);
    color: var(--hr-accent-light);
    font-weight: 550;
    border-color: color-mix(in srgb, var(--hr-accent-mid) 35%, transparent);
  }

  .panel {
    min-width: 0;
  }

  @media (max-width: 820px) {
    .settings-frame {
      grid-template-columns: 1fr;
    }

    .tabs {
      flex-direction: row;
      overflow-x: auto;
      padding-bottom: 4px;
    }
  }
</style>
