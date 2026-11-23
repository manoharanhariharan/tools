<script lang="ts">
  import type {
    BootstrapDto,
    GuidanceResult,
    GuidanceSelection,
    ProfileMode,
  } from "../../ipc/contracts";
  import { i18n } from "../../i18n/index.svelte";
  import { toCssColor, applyTheme } from "../../theme/apply";
  import Button from "../../components/Button.svelte";
  import Icon from "../../components/Icon.svelte";
  import SelectField from "../../components/SelectField.svelte";
  import Toggle from "../../components/Toggle.svelte";
  import "./guidance.css";

  interface Props {
    bootstrap: BootstrapDto;
    onfinish: (selection: GuidanceSelection) => Promise<GuidanceResult | null>;
  }

  let { bootstrap, onfinish }: Props = $props();

  const steps = $derived.by(() => {
    const list = ["welcome", "profile", "language", "theme", "startup", "shortcut", "confirm"];
    if (bootstrap.legacy) {
      list.push("migrate");
    }
    list.push("finish");
    return list;
  });

  let index = $state(0);
  let submitting = $state(false);
  let error = $state<string | null>(null);

  // Initial values come from the bootstrap snapshot; later changes are user input.
  // svelte-ignore state_referenced_locally
  let selection = $state<{
    profileMode: ProfileMode;
    language: string;
    themeName: string;
    customTheme: GuidanceSelection["customTheme"];
    runAtStartup: boolean;
    startMinimized: boolean;
    createShortcut: boolean;
    confirmBeforeClosingSession: boolean;
    importLegacySettings: boolean;
  }>({
    profileMode: bootstrap.paths.profileMode,
    language: bootstrap.config.general.language,
    themeName: bootstrap.config.theme.themeName,
    customTheme: null,
    runAtStartup: bootstrap.config.general.runAtStartup,
    startMinimized: bootstrap.config.general.startMinimized,
    createShortcut: true,
    confirmBeforeClosingSession: bootstrap.config.general.confirmBeforeClosingSession,
    importLegacySettings: Boolean(bootstrap.legacy),
  });

  const step = $derived(steps[index] ?? "welcome");
  const isLast = $derived(index === steps.length - 1);
  const profileLocked = $derived(bootstrap.markerLocked);

  const languageOptions = $derived([
    { value: "system", label: `${i18n.t("common.systemDefault")} (${bootstrap.systemLocale})` },
    ...bootstrap.locales.map((locale) => ({
      value: locale.id,
      label: `${locale.nativeName} — ${locale.englishName}`,
    })),
  ]);

  const selectedPreset = $derived(
    bootstrap.themes.find((preset) => preset.name === selection.themeName),
  );

  function previewTheme(name: string) {
    selection.themeName = name;
    const preset = bootstrap.themes.find((candidate) => candidate.name === name);
    if (!preset) return;
    applyTheme({
      id: preset.name,
      displayName: preset.displayName,
      colors: preset.colors,
      fontFamily: bootstrap.config.theme.fontFamily,
      fontSize: bootstrap.config.theme.fontSize,
    });
  }

  function headingFor(item: string): string {
    const keys: Record<string, string> = {
      welcome: "guidance.welcome.title",
      profile: "guidance.profile.title",
      language: "guidance.language.title",
      theme: "guidance.theme.title",
      startup: "guidance.startup.title",
      shortcut: "guidance.shortcut.title",
      confirm: "guidance.confirm.title",
      migrate: "guidance.migrate.title",
      finish: "guidance.finish.title",
    };
    return i18n.t(keys[item] ?? "guidance.title");
  }

  function bodyFor(item: string): string {
    const keys: Record<string, string> = {
      welcome: "guidance.welcome.body",
      profile: "guidance.profile.body",
      language: "guidance.language.body",
      theme: "guidance.theme.body",
      startup: "guidance.startup.body",
      shortcut: "guidance.shortcut.body",
      confirm: "guidance.confirm.body",
      migrate: "guidance.migrate.body",
      finish: "guidance.finish.body",
    };
    const key = keys[item];
    return key ? i18n.t(key) : "";
  }

  function stepLabel(item: string): string {
    const keys: Record<string, string> = {
      welcome: "guidance.welcome.title",
      profile: "guidance.profile.title",
      language: "guidance.language.title",
      theme: "guidance.theme.title",
      startup: "guidance.startup.title",
      shortcut: "guidance.shortcut.title",
      confirm: "guidance.confirm.title",
      migrate: "guidance.migrate.title",
      finish: "guidance.finish.title",
    };
    return i18n.t(keys[item] ?? "guidance.title");
  }

  function languageLabel(language: string): string {
    if (language === "system") {
      return `${i18n.t("common.systemDefault")} (${bootstrap.systemLocale})`;
    }
    return bootstrap.locales.find((locale) => locale.id === language)?.nativeName ?? language;
  }

  function chooseProfile(mode: ProfileMode) {
    if (!profileLocked) {
      selection.profileMode = mode;
    }
  }

  function next() {
    if (index < steps.length - 1) {
      index += 1;
    }
  }

  function back() {
    if (index > 0) {
      index -= 1;
    }
  }

  async function finish() {
    if (submitting) return;
    submitting = true;
    error = null;
    try {
      const result = await onfinish(selection);      if (!result) {
        error = "Could not save the setup choices. Check the log folder for details.";
      }
    } finally {
      submitting = false;
    }
  }

  const profileCurrent = $derived(
    bootstrap.paths.profileMode === "portable"
      ? i18n.t("profile.portable")
      : i18n.t("profile.appdata"),
  );
</script>

<div class="wizard-layer">
  <div class="wizard" role="dialog" aria-modal="true" aria-labelledby="wizard-title">
    <aside class="rail">
      <div class="brand">
        <span class="glyph" aria-hidden="true">H</span>
        <div>
          <strong>H-Remote</strong>
          <span> {i18n.t("guidance.subtitle")}</span>
        </div>
      </div>
      <ol class="rail-steps">
        {#each steps as item, itemIndex (item)}
          <li class:current={itemIndex === index} class:done={itemIndex < index}>
            <span class="bullet" aria-hidden="true">
              {#if itemIndex < index}<Icon name="check" size={12} />{:else}{itemIndex + 1}{/if}
            </span>
            <span>{stepLabel(item)}</span>
          </li>
        {/each}
      </ol>
      <p class="progress">
        {i18n.t_args("guidance.stepOf", { current: index + 1, total: steps.length })}
      </p>
    </aside>

    <section class="stage">
      <header>
        <div>
          <h1 id="wizard-title">{headingFor(step)}</h1>
          <p>{bodyFor(step)}</p>
        </div>
        <span class="hr-pill">{i18n.t("guidance.title")}</span>
      </header>

      <div class="stage-body hr-scroll">
        {#if step === "welcome"}
          <div class="welcome-grid">
            <article class="tile">
              <Icon name="servers" size={22} />
              <h3>Catalog</h3>
              <p>Remote desktops, terminals and file transfers in one searchable list.</p>
            </article>
            <article class="tile">
              <Icon name="key" size={22} />
              <h3>Secure by default</h3>
              <p>OS keyring, encrypted vault and no plaintext secrets on disk.</p>
            </article>
            <article class="tile">
              <Icon name="palette" size={22} />
              <h3>Yours</h3>
              <p>Nine themes, custom colours, 14 languages and portable profiles.</p>
            </article>
          </div>
        {:else if step === "profile"}
          <div class="choices">
            <button
              type="button"
              class="choice"
              class:selected={selection.profileMode === "portable"}
              disabled={profileLocked && bootstrap.paths.profileMode !== "portable"}
              onclick={() => chooseProfile("portable")}
            >
              <strong>{i18n.t("guidance.profile.portable")}</strong>
              <span>{i18n.t("guidance.profile.portableHelp")}</span>
              <code>{bootstrap.paths.baseDir}</code>
            </button>
            <button
              type="button"
              class="choice"
              class:selected={selection.profileMode === "appData"}
              disabled={profileLocked && bootstrap.paths.profileMode !== "appData"}
              onclick={() => chooseProfile("appData")}
            >
              <strong>{i18n.t("guidance.profile.appdata")}</strong>
              <span>{i18n.t("guidance.profile.appdataHelp")}</span>
              <code>{bootstrap.paths.dataDir}</code>
            </button>
          </div>
          {#if profileLocked}
            <p class="hint">
              {i18n.t_args("guidance.profile.locked", { mode: profileCurrent })}
            </p>
          {:else if selection.profileMode !== bootstrap.paths.profileMode}
            <p class="hint warning">Changing profile mode requires a restart.</p>
          {/if}
        {:else if step === "language"}
          <SelectField
            id="guidance-language"
            label={i18n.t("general.language")}
            value={selection.language}
            options={languageOptions}
            hint={i18n.t("guidance.language.body")}
            onchange={(value) => {
              selection.language = value;
              i18n.setLocale(value === "system" ? bootstrap.systemLocale : value);
            }}
          />
        {:else if step === "theme"}
          <div class="theme-grid">
            {#each bootstrap.themes as preset (preset.name)}
              <button
                type="button"
                class="theme-card"
                class:selected={selection.themeName === preset.name}
                onclick={() => previewTheme(preset.name)}
              >
                <span
                  class="swatch"
                  style:background={toCssColor(preset.colors.background)}
                  style:border-color={toCssColor(preset.colors.primaryText)}
                >
                  <span style:background={toCssColor(preset.colors.primaryMid)}></span>
                  <span style:background={toCssColor(preset.colors.accentMid)}></span>
                </span>
                <span class="theme-name">{preset.displayName}</span>
                {#if selection.themeName === preset.name}
                  <Icon name="check" size={15} />
                {/if}
              </button>
            {/each}
          </div>
          {#if selectedPreset}
            <div class="theme-preview">
              <span class="hr-small hr-muted">Preview</span>
              <div class="preview-surface" style:background={toCssColor(selectedPreset.colors.background)} style:color={toCssColor(selectedPreset.colors.backgroundText)}>
                <div class="preview-bar" style:background={toCssColor(selectedPreset.colors.primaryMid)} style:color={toCssColor(selectedPreset.colors.primaryText)}>
                  H-Remote
                </div>
                <div class="preview-body">
                  <span class="preview-accent" style:background={toCssColor(selectedPreset.colors.accentMid)}></span>
                  <span>Server catalog</span>
                </div>
              </div>
            </div>
          {/if}
        {:else if step === "startup"}
          <div class="stack">
            <Toggle
              id="guidance-run-at-startup"
              label={i18n.t("guidance.startup.enable")}
              description={i18n.t("general.startWithOsHelp")}
              checked={selection.runAtStartup}
              onchange={(checked) => (selection.runAtStartup = checked)}
            />
            <Toggle
              id="guidance-start-minimized"
              label={i18n.t("guidance.startup.minimized")}
              description={i18n.t("general.startMinimized")}
              checked={selection.startMinimized}
              onchange={(checked) => (selection.startMinimized = checked)}
            />
          </div>
        {:else if step === "shortcut"}
          <Toggle
            id="guidance-create-shortcut"
            label={i18n.t("guidance.shortcut.create")}
            description={i18n.t("guidance.shortcut.body")}
            checked={selection.createShortcut}
            onchange={(checked) => (selection.createShortcut = checked)}
          />
        {:else if step === "confirm"}
          <Toggle
            id="guidance-confirm-close"
            label={i18n.t("guidance.confirm.body")}
            checked={selection.confirmBeforeClosingSession}
            onchange={(checked) => (selection.confirmBeforeClosingSession = checked)}
          />
        {:else if step === "migrate"}
          <div class="migrate">
            <Icon name="info" size={20} />
            <div>
              <p>
                {i18n.t_args("guidance.migrate.body", {
                  path: bootstrap.legacy?.configFile ?? bootstrap.legacy?.path ?? "",
                })}
              </p>
              <Toggle
                id="guidance-import-legacy"
                label={i18n.t("guidance.migrate.accept")}
                checked={selection.importLegacySettings}
                onchange={(checked) => (selection.importLegacySettings = checked)}
              />
            </div>
          </div>
        {:else}
          <div class="summary">
            <div><span>{i18n.t("general.language")}</span><strong>{languageLabel(selection.language)}</strong></div>
            <div><span>{i18n.t("common.theme")}</span><strong>{selection.themeName}</strong></div>
            <div><span>{i18n.t("about.profile")}</span><strong>{selection.profileMode === "portable" ? i18n.t("profile.portable") : i18n.t("profile.appdata")}</strong></div>
            <div><span>{i18n.t("general.startWithOs")}</span><strong>{selection.runAtStartup ? i18n.t("common.enabled") : i18n.t("common.disabled")}</strong></div>
            <div><span>{i18n.t("guidance.shortcut.create")}</span><strong>{selection.createShortcut ? i18n.t("common.enabled") : i18n.t("common.disabled")}</strong></div>
          </div>
          <p class="hint">{i18n.t("guidance.finish.body")}</p>
        {/if}

        {#if error}
          <p class="error" role="alert">{error}</p>
        {/if}
      </div>

      <footer>
        <Button variant="ghost" disabled={index === 0 || submitting} onclick={back}>
          <Icon name="chevron-left" size={15} />
          {i18n.t("common.back")}
        </Button>
        <div class="spacer"></div>
        {#if isLast}
          <Button variant="primary" disabled={submitting} onclick={finish}>
            {submitting ? i18n.t("common.loading") : i18n.t("common.finish")}
          </Button>
        {:else}
          <Button variant="primary" onclick={next}>
            {i18n.t("common.next")}
            <Icon name="chevron-right" size={15} />
          </Button>
        {/if}
      </footer>
    </section>
  </div>
</div>
