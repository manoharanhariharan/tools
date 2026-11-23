<script lang="ts">
  import type { Configuration } from "../../ipc/contracts";
  import { commands } from "../../ipc/client";
  import { i18n } from "../../i18n/index.svelte";
  import { app } from "../../state/app.svelte";
  import { busy } from "../../state/busy.svelte";
  import Button from "../../components/Button.svelte";
  import TextField from "../../components/TextField.svelte";
  import Toggle from "../../components/Toggle.svelte";

  interface Props {
    config: Configuration;
    onpatch: (mutate: (draft: Configuration) => void) => Promise<Configuration | null>;
  }

  let { config, onpatch }: Props = $props();

  // svelte-ignore state_referenced_locally
  let hotkey = $state(config.launcher.hotkey);
  let hotkeyError = $state<string | null>(null);

  $effect(() => {
    hotkey = config.launcher.hotkey;
  });

  async function applyHotkey() {
    hotkeyError = null;
    try {
      const canonical = await busy.track(i18n.t("common.loading"), () =>
        commands.validateHotkey(hotkey),
      );
      hotkey = canonical;
      const updated = await onpatch((draft) => {
        draft.launcher.hotkey = canonical;
      });
      if (updated) {
        app.showNotice(i18n.t("launcher.hotkeySaved"), "info");
      }
    } catch (error) {
      hotkeyError =
        error instanceof Error ? error.message : i18n.t_args("launcher.hotkeyInvalid", { hotkey });
    }
  }

  function updateRecentLimit(value: string) {
    const parsed = Number.parseInt(value, 10);
    if (!Number.isFinite(parsed)) return;
    const limit = Math.min(20, Math.max(0, parsed));
    void onpatch((draft) => {
      draft.launcher.recentLimit = limit;
    });
  }
</script>

<div class="hr-stack">
  <section class="hr-card">
    <div class="hr-card-title"><h2>{i18n.t("launcher.title")}</h2></div>
    <Toggle
      id="launcher-enabled"
      label={i18n.t("launcher.enabled")}
      description={i18n.t("launcher.hotkeyHelp")}
      checked={config.launcher.enabled}
      onchange={(checked) =>
        onpatch((draft) => {
          draft.launcher.enabled = checked;
        })}
    />

    <div class="hotkey-row">
      <TextField
        id="launcher-hotkey"
        label={i18n.t("launcher.hotkey")}
        value={hotkey}
        placeholder="Alt+M"
        error={hotkeyError}
        hint={i18n.t("launcher.hotkeyHelp")}
        oninput={(value) => {
          hotkey = value;
          hotkeyError = null;
        }}
        onkeydown={(event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            void applyHotkey();
          }
        }}
      />
      <Button variant="secondary" disabled={!config.launcher.enabled} onclick={applyHotkey}>
        {i18n.t("common.apply")}
      </Button>
    </div>
  </section>

  <section class="hr-card">
    <div class="hr-card-title"><h2>{i18n.t("launcher.title")}</h2></div>
    <Toggle
      id="launcher-show-note"
      label={i18n.t("launcher.showNote")}
      checked={config.launcher.showNote}
      onchange={(checked) =>
        onpatch((draft) => {
          draft.launcher.showNote = checked;
        })}
    />
    <Toggle
      id="launcher-show-credentials"
      label={i18n.t("launcher.showCredentials")}
      checked={config.launcher.showCredentials}
      onchange={(checked) =>
        onpatch((draft) => {
          draft.launcher.showCredentials = checked;
        })}
    />
    <Toggle
      id="launcher-save-info"
      label={i18n.t("launcher.saveInfo")}
      checked={config.launcher.saveEnteredInfo}
      onchange={(checked) =>
        onpatch((draft) => {
          draft.launcher.saveEnteredInfo = checked;
        })}
    />
    <Toggle
      id="launcher-match-credential"
      label={i18n.t("launcher.matchCredential")}
      checked={config.launcher.matchCredential}
      onchange={(checked) =>
        onpatch((draft) => {
          draft.launcher.matchCredential = checked;
        })}
    />
    <Toggle
      id="launcher-remember-position"
      label={i18n.t("launcher.position.remember")}
      checked={config.launcher.rememberPosition}
      onchange={(checked) =>
        onpatch((draft) => {
          draft.launcher.rememberPosition = checked;
        })}
    />
    <label class="limit">
      <span>{i18n.t("launcher.recentLimit")}</span>
      <input
        type="number"
        min="0"
        max="20"
        value={config.launcher.recentLimit}
        onchange={(event) => updateRecentLimit(event.currentTarget.value)}
      />
    </label>
    <p class="hr-small hr-muted">
      The hotkey opens a borderless launcher above the current app. Tab switches to quick
      connect; Esc hides it.
    </p>
  </section>
</div>

<style>
  .hotkey-row {
    display: flex;
    align-items: flex-end;
    gap: 10px;
    max-width: 520px;
    margin-top: 12px;
  }

  .hotkey-row :global(.hr-field) {
    flex: 1;
  }

  .limit {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 9px 0;
  }

  .limit input {
    width: 84px;
    min-height: 32px;
    border-radius: var(--hr-radius);
    border: 1px solid var(--hr-border);
    background: color-mix(in srgb, var(--hr-primary-dark) 55%, var(--hr-background));
    color: inherit;
    padding: 0 9px;
  }
</style>
