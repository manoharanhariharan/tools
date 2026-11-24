<script lang="ts">
  import type { Configuration, LocaleInfo, MatcherKind } from "../../ipc/contracts";
  import { commands } from "../../ipc/client";
  import { i18n } from "../../i18n/index.svelte";
  import { busy } from "../../state/busy.svelte";
  import { app } from "../../state/app.svelte";
  import Button from "../../components/Button.svelte";
  import Icon from "../../components/Icon.svelte";
  import SelectField from "../../components/SelectField.svelte";
  import Toggle from "../../components/Toggle.svelte";

  interface Props {
    config: Configuration;
    locales: LocaleInfo[];
    systemLocale: string;
    onpatch: (mutate: (draft: Configuration) => void) => Promise<Configuration | null>;
  }

  let { config, locales, systemLocale, onpatch }: Props = $props();

  const languageOptions = $derived([
    { value: "system", label: `${i18n.t("common.systemDefault")} (${systemLocale})` },
    ...locales.map((locale) => ({
      value: locale.id,
      label: `${locale.nativeName} — ${locale.englishName}`,
    })),
  ]);

  const logLevels = [
    { value: "off", label: "logs.level.off" },
    { value: "error", label: "logs.level.error" },
    { value: "warn", label: "logs.level.warn" },
    { value: "info", label: "logs.level.info" },
    { value: "debug", label: "logs.level.debug" },
    { value: "trace", label: "logs.level.trace" },
  ];

  async function patch(mutate: (draft: Configuration) => void) {
    await onpatch(mutate);
  }

  async function openLogs() {
    try {
      await busy.track(i18n.t("common.loading"), () => commands.openLogFolder());
    } catch (error) {
      await app.reportError(error);
    }
  }

  async function resetWindow() {
    try {
      await commands.resetMainWindow();
      app.showNotice(i18n.t("settings.windowReset"), "info");
    } catch (error) {
      await app.reportError(error);
    }
  }

  async function installShortcut() {
    try {
      const path = await busy.track(i18n.t("common.loading"), () => commands.installShortcut());
      app.showNotice(path, "info");
    } catch (error) {
      await app.reportError(error);
    }
  }

  async function toggleStartup(enabled: boolean) {
    await patch((draft) => {
      draft.general.runAtStartup = enabled;
    });
  }

  const matcherOrder: MatcherKind[] = [
    "direct",
    "initials",
    "discrete",
    "pinyin",
    "pinyinInitials",
  ];

  async function setMatcher(kind: MatcherKind, enabled: boolean) {
    await patch((draft) => {
      const selected = new Set(draft.keywordMatch.matchers);
      if (enabled) selected.add(kind);
      else selected.delete(kind);
      selected.add("direct");
      draft.keywordMatch.matchers = matcherOrder.filter((candidate) =>
        selected.has(candidate),
      );
    });
  }
</script>

<div class="hr-stack">
  <section class="hr-card">
    <div class="hr-card-title"><h2>{i18n.t("settings.general")}</h2></div>
    <div class="form-grid">
      <SelectField
        id="settings-language"
        label={i18n.t("general.language")}
        value={config.general.language}
        options={languageOptions}
        onchange={(value) =>
          patch((draft) => {
            draft.general.language = value;
          })}
      />
      <SelectField
        id="settings-log-level"
        label={i18n.t("general.logLevel")}
        value={config.general.logLevel}
        options={logLevels.map((level) => ({ value: level.value, label: i18n.t(level.label) }))}
        onchange={(value) =>
          patch((draft) => {
            draft.general.logLevel = value as Configuration["general"]["logLevel"];
          })}
      />
    </div>

    <hr class="hr-divider" />

    <Toggle
      id="settings-run-at-startup"
      label={i18n.t("general.startWithOs")}
      description={i18n.t("general.startWithOsHelp")}
      checked={config.general.runAtStartup}
      onchange={toggleStartup}
    />
    <Toggle
      id="settings-start-minimized"
      label={i18n.t("general.startMinimized")}
      checked={config.general.startMinimized}
      onchange={(checked) =>
        patch((draft) => {
          draft.general.startMinimized = checked;
        })}
    />
    <Toggle
      id="settings-confirm-close"
      label={i18n.t("general.confirmCloseSession")}
      checked={config.general.confirmBeforeClosingSession}
      onchange={(checked) =>
        patch((draft) => {
          draft.general.confirmBeforeClosingSession = checked;
        })}
    />
    <Toggle
      id="settings-session-icon"
      label={i18n.t("general.sessionIcon")}
      checked={config.general.showSessionIconInTitle}
      onchange={(checked) =>
        patch((draft) => {
          draft.general.showSessionIconInTitle = checked;
        })}
    />
    <Toggle
      id="settings-show-note"
      label={i18n.t("general.noteInList")}
      checked={config.general.showNoteInList}
      onchange={(checked) =>
        patch((draft) => {
          draft.general.showNoteInList = checked;
        })}
    />
    <Toggle
      id="settings-copy-port"
      label={i18n.t("general.copyPort")}
      checked={config.general.copyPortWithAddress}
      onchange={(checked) =>
        patch((draft) => {
          draft.general.copyPortWithAddress = checked;
        })}
    />
    <Toggle
      id="settings-focus-local"
      label={i18n.t("general.focusLocalOnLeave")}
      checked={config.general.moveFocusToLocalOnMouseLeave}
      onchange={(checked) =>
        patch((draft) => {
          draft.general.moveFocusToLocalOnMouseLeave = checked;
        })}
    />
    <Toggle
      id="settings-no-update-check"
      label={i18n.t("general.noUpdateCheck")}
      checked={config.general.doNotCheckForUpdates}
      onchange={(checked) =>
        patch((draft) => {
          draft.general.doNotCheckForUpdates = checked;
        })}
    />
  </section>

  <section class="hr-card">
    <div class="hr-card-title">
      <h2>Updates</h2>
      <Button size="sm" variant="secondary" onclick={() => void app.checkForUpdates()}>
        Check now
      </Button>
    </div>
    <p class="hr-small hr-muted">
      Automatic checks read a JSON manifest; nothing is downloaded or installed silently.
    </p>
    <Toggle
      id="settings-prerelease"
      label="Include pre-release versions"
      checked={config.general.includePrereleaseUpdates}
      onchange={(checked) =>
        patch((draft) => {
          draft.general.includePrereleaseUpdates = checked;
        })}
    />
    <div class="updates-row">
      <label class="updates-field">
        <span>Check every (hours, 0 disables)</span>
        <input
          type="number"
          min="0"
          max="168"
          value={config.general.updateCheckPeriodHours}
          onchange={(event) =>
            patch((draft) => {
              draft.general.updateCheckPeriodHours = Math.max(
                0,
                Math.min(168, Number(event.currentTarget.value) || 0),
              );
            })}
        />
      </label>
      <label class="updates-field grow">
        <span>Update feed URL</span>
        <input
          value={config.general.updateFeedUrl ?? ""}
          placeholder="https://example.invalid/h-remote/latest.json"
          onchange={(event) =>
            patch((draft) => {
              draft.general.updateFeedUrl = event.currentTarget.value.trim() || null;
            })}
        />
      </label>
    </div>
    {#if app.updateStatus}
      {@const status = app.updateStatus}
      {#if status.error}
        <p class="updates-error" role="status">{status.error}</p>
      {:else if status.available}
        <p class="updates-ok">
          Version {status.version} is available.
          <button type="button" class="link" onclick={() => app.openUpdateDialog()}>Details</button>
        </p>
      {:else}
        <p class="hr-small hr-muted">Up to date ({status.current}).</p>
      {/if}
    {/if}
  </section>

  <section class="hr-card">
    <div class="hr-card-title"><h2>Search matching</h2></div>
    <p class="hr-small hr-muted">
      Direct matching is always active. Extra providers widen matching but cost more work on large
      catalogs.
    </p>
    <Toggle
      id="settings-match-initials"
      label="Initials (e.g. “prd” → Production)"
      checked={config.keywordMatch.matchers.includes("initials")}
      onchange={(checked) => void setMatcher("initials", checked)}
    />
    <Toggle
      id="settings-match-discrete"
      label="Discrete words"
      checked={config.keywordMatch.matchers.includes("discrete")}
      onchange={(checked) => void setMatcher("discrete", checked)}
    />
    <Toggle
      id="settings-match-pinyin"
      label="Pinyin"
      checked={config.keywordMatch.matchers.includes("pinyin")}
      onchange={(checked) => void setMatcher("pinyin", checked)}
    />
    <Toggle
      id="settings-match-pinyin-initials"
      label="Pinyin initials"
      checked={config.keywordMatch.matchers.includes("pinyinInitials")}
      onchange={(checked) => void setMatcher("pinyinInitials", checked)}
    />
    <Toggle
      id="settings-match-auto-locale"
      label="Enable pinyin automatically for Chinese locales"
      checked={config.keywordMatch.autoLocale}
      onchange={(checked) =>
        patch((draft) => {
          draft.keywordMatch.autoLocale = checked;
        })}
    />
  </section>

  <section class="hr-card">
    <div class="hr-card-title"><h2>{i18n.t("general.tabHeader")}</h2></div>
    <Toggle
      id="settings-tab-icon"
      label={i18n.t("general.tabIcon")}
      checked={config.general.tabHeader.showIcon}
      onchange={(checked) =>
        patch((draft) => {
          draft.general.tabHeader.showIcon = checked;
        })}
    />
    <Toggle
      id="settings-tab-close"
      label={i18n.t("general.tabClose")}
      checked={config.general.tabHeader.showCloseButton}
      onchange={(checked) =>
        patch((draft) => {
          draft.general.tabHeader.showCloseButton = checked;
        })}
    />
    <Toggle
      id="settings-tab-reconnect"
      label={i18n.t("general.tabReconnect")}
      checked={config.general.tabHeader.showReconnectButton}
      onchange={(checked) =>
        patch((draft) => {
          draft.general.tabHeader.showReconnectButton = checked;
        })}
    />
    <Toggle
      id="settings-tab-close-left"
      label={i18n.t("general.tabCloseLeft")}
      checked={config.general.tabHeader.closeButtonOnLeft}
      onchange={(checked) =>
        patch((draft) => {
          draft.general.tabHeader.closeButtonOnLeft = checked;
        })}
    />
  </section>

  <section class="hr-card">
    <div class="hr-card-title"><h2>{i18n.t("settings.section.help")}</h2></div>
    <div class="hr-row">
      <Button variant="secondary" onclick={openLogs}>
        <Icon name="folder" size={15} />
        {i18n.t("settings.openLogFolder")}
      </Button>
      <Button variant="secondary" onclick={resetWindow}>
        <Icon name="refresh" size={15} />
        {i18n.t("settings.resetWindow")}
      </Button>
      <Button variant="secondary" onclick={installShortcut}>
        <Icon name="external" size={15} />
        {i18n.t("guidance.shortcut.create")}
      </Button>
    </div>
    <p class="hr-small hr-muted paths">
      {i18n.t("about.configuration")}: <code>{app.bootstrap?.paths.configFile}</code>
    </p>
  </section>
</div>

<style>
  .form-grid {
    display: grid;
    gap: 14px;
    grid-template-columns: repeat(auto-fit, minmax(230px, 1fr));
    margin-bottom: 8px;
  }

  .paths {
    margin-top: 14px;
  }

  .updates-row {
    display: flex;
    gap: 10px;
    align-items: flex-end;
    margin-top: 8px;
    flex-wrap: wrap;
  }

  .updates-field {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 10px;
    min-width: 0;
  }

  .updates-field.grow {
    flex: 1;
    min-width: 240px;
  }

  .updates-field input {
    min-height: 31px;
    border-radius: var(--hr-radius);
    border: 1px solid var(--hr-border);
    background: color-mix(in srgb, var(--hr-primary-dark) 55%, var(--hr-background));
    color: inherit;
    padding: 6px 8px;
    font: inherit;
    font-size: 11px;
  }

  .updates-error {
    color: var(--hr-danger);
    font-size: 11px;
  }

  .updates-ok {
    color: #49b675;
    font-size: 11px;
  }

  .link {
    border: 0;
    background: transparent;
    color: var(--hr-accent-light);
    text-decoration: underline;
    font: inherit;
    font-size: 11px;
    cursor: pointer;
  }
</style>
