<script lang="ts">
  import type { Configuration, ResolvedTheme, ThemeColors, ThemeConfig, ThemePreset } from "../../ipc/contracts";
  import { i18n } from "../../i18n/index.svelte";
  import { applyTheme, toCssColor } from "../../theme/apply";
  import { app } from "../../state/app.svelte";
  import Button from "../../components/Button.svelte";

  interface Props {
    config: Configuration;
    theme: ResolvedTheme | null;
    themes: ThemePreset[];
    onpatch: (mutate: (draft: Configuration) => void) => Promise<Configuration | null>;
  }

  let { config, theme, themes, onpatch }: Props = $props();

  // svelte-ignore state_referenced_locally
  let colors = $state<ThemeColors>({ ...config.theme.colors });
  // svelte-ignore state_referenced_locally
  let fontFamily = $state(config.theme.fontFamily);
  // svelte-ignore state_referenced_locally
  let fontSize = $state(config.theme.fontSize);
  let dirty = $state(false);

  $effect(() => {
    const persisted = config.theme;
    colors = { ...persisted.colors };
    fontFamily = persisted.fontFamily;
    fontSize = persisted.fontSize;
    dirty = false;
  });

  const colorFields: Array<{ key: keyof ThemeColors; label: string }> = [
    { key: "primaryMid", label: "appearance.primaryMid" },
    { key: "primaryLight", label: "appearance.primaryLight" },
    { key: "primaryDark", label: "appearance.primaryDark" },
    { key: "primaryText", label: "appearance.primaryText" },
    { key: "accentMid", label: "appearance.accentMid" },
    { key: "accentLight", label: "appearance.accentLight" },
    { key: "accentDark", label: "appearance.accentDark" },
    { key: "accentText", label: "appearance.accentText" },
    { key: "background", label: "appearance.background" },
    { key: "backgroundText", label: "appearance.backgroundText" },
  ];

  function preview() {
    applyTheme({
      id: "Custom",
      displayName: "Custom",
      colors,
      fontFamily,
      fontSize,
    });
  }

  function updateColor(key: keyof ThemeColors, cssHex: string) {
    const argb = `#FF${cssHex.replace("#", "").toUpperCase()}`;
    colors = { ...colors, [key]: argb };
    dirty = true;
    preview();
  }

  function updateFontFamily(value: string) {
    fontFamily = value;
    dirty = true;
    preview();
  }

  function updateFontSize(value: number) {
    fontSize = Math.min(20, Math.max(10, value));
    dirty = true;
    preview();
  }

  async function selectPreset(preset: ThemePreset) {
    const updated = await onpatch((draft) => {
      draft.theme = {
        ...draft.theme,
        themeName: preset.name,
        isCustom: false,
        colors: { ...preset.colors },
      };
    });
    if (updated) {
      app.showNotice(i18n.t("appearance.presetApplied"), "info");
    }
  }

  async function applyCustom() {
    const custom: ThemeConfig = {
      themeName: "Custom",
      isCustom: true,
      colors: { ...colors },
      fontFamily: fontFamily.trim() || "system-ui",
      fontSize,
    };
    const updated = await onpatch((draft) => {
      draft.theme = custom;
    });
    if (updated) {
      dirty = false;
    }
  }

  function colorValue(value: string): string {
    return toCssColor(value).slice(0, 7);
  }
</script>

<div class="hr-stack">
  <section class="hr-card">
    <div class="hr-card-title">
      <h2>{i18n.t("appearance.preset")}</h2>
      {#if theme}<span class="hr-pill">{theme.displayName}</span>{/if}
    </div>
    <div class="preset-grid">
      {#each themes as preset (preset.name)}
        <button
          type="button"
          class="preset"
          class:active={!config.theme.isCustom && config.theme.themeName === preset.name}
          data-testid={`theme-${preset.name}`}
          onclick={() => selectPreset(preset)}
        >
          <span
            class="swatch"
            style:background={toCssColor(preset.colors.background)}
            style:border-color={toCssColor(preset.colors.primaryText)}
          >
            <span style:background={toCssColor(preset.colors.primaryMid)}></span>
            <span style:background={toCssColor(preset.colors.accentMid)}></span>
            <span style:background={toCssColor(preset.colors.accentLight)}></span>
          </span>
          <span>{preset.displayName}</span>
        </button>
      {/each}
    </div>
  </section>

  <section class="hr-card">
    <div class="hr-card-title">
      <h2>{i18n.t("appearance.custom")}</h2>
      {#if dirty}<span class="hr-pill">{i18n.t("general.unsavedChanges")}</span>{/if}
    </div>
    <p class="hr-small hr-muted">{i18n.t("appearance.customHelp")}</p>

    <div class="color-grid">
      {#each colorFields as field (field.key)}
        <label class="color-field">
          <span>{i18n.t(field.label)}</span>
          <span class="color-input">
            <input
              type="color"
              value={colorValue(colors[field.key])}
              aria-label={i18n.t(field.label)}
              oninput={(event) => updateColor(field.key, event.currentTarget.value)}
            />
            <code>{colors[field.key]}</code>
          </span>
        </label>
      {/each}
    </div>

    <div class="form-grid">
      <label class="field">
        <span>{i18n.t("appearance.fontFamily")}</span>
        <input
          type="text"
          value={fontFamily}
          oninput={(event) => updateFontFamily(event.currentTarget.value)}
        />
      </label>
      <label class="field">
        <span>{i18n.t("appearance.fontSize")}: {fontSize}px</span>
        <input
          type="range"
          min="10"
          max="20"
          step="1"
          value={fontSize}
          oninput={(event) => updateFontSize(Number(event.currentTarget.value))}
        />
      </label>
    </div>

    <div class="hr-row apply-row">
      <Button variant="primary" onclick={applyCustom}>
        {i18n.t("common.apply")}
      </Button>
      <Button
        variant="ghost"
        onclick={() => {
          colors = { ...config.theme.colors };
          fontFamily = config.theme.fontFamily;
          fontSize = config.theme.fontSize;
          dirty = false;
          if (theme) applyTheme(theme);
        }}
      >
        {i18n.t("common.reset")}
      </Button>
    </div>
  </section>
</div>

<style>
  .preset-grid {
    display: grid;
    gap: 10px;
    grid-template-columns: repeat(auto-fill, minmax(148px, 1fr));
  }

  .preset {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 8px;
    padding: 9px;
    border-radius: var(--hr-radius);
    border: 1px solid var(--hr-border);
    background: transparent;
    color: inherit;
    text-align: left;
    font-size: var(--hr-font-size-small);
  }

  .preset:hover {
    border-color: var(--hr-border-strong);
  }

  .preset.active {
    border-color: var(--hr-accent-mid);
    background: color-mix(in srgb, var(--hr-accent-mid) 12%, transparent);
  }

  .swatch {
    height: 34px;
    border-radius: 6px;
    border: 1px solid;
    display: flex;
    overflow: hidden;
  }

  .swatch span {
    flex: 1;
  }

  .color-grid {
    display: grid;
    gap: 10px;
    grid-template-columns: repeat(auto-fit, minmax(190px, 1fr));
    margin: 14px 0;
  }

  .color-field {
    display: flex;
    flex-direction: column;
    gap: 5px;
    font-size: var(--hr-font-size-small);
  }

  .color-input {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .color-input input {
    width: 42px;
    height: 28px;
    padding: 0;
    border: 1px solid var(--hr-border);
    border-radius: 6px;
    background: transparent;
  }

  .color-input code {
    color: color-mix(in srgb, var(--hr-background-text) 62%, transparent);
  }

  .form-grid {
    display: grid;
    gap: 14px;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: var(--hr-font-size-small);
  }

  .field input[type="text"] {
    min-height: 34px;
    border-radius: var(--hr-radius);
    border: 1px solid var(--hr-border);
    background: color-mix(in srgb, var(--hr-primary-dark) 55%, var(--hr-background));
    padding: 0 11px;
    color: inherit;
  }

  .apply-row {
    margin-top: 16px;
  }
</style>
