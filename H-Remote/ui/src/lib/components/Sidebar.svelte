<script lang="ts">
  import type { AppInfo, PathsDto } from "../ipc/contracts";
  import { i18n } from "../i18n/index.svelte";
  import type { Route } from "../state/app.svelte";
  import Icon from "./Icon.svelte";

  interface Props {
    route: Route;
    appInfo: AppInfo | null;
    paths: PathsDto | null;
    onnavigate: (route: Route) => void;
  }

  let { route, appInfo, paths, onnavigate }: Props = $props();

  const items: Array<{ route: Route; icon: "servers" | "settings" | "about"; label: string }> = [
    { route: "servers", icon: "servers", label: "nav.servers" },
    { route: "settings", icon: "settings", label: "nav.settings" },
    { route: "about", icon: "about", label: "nav.about" },
  ];
</script>

<nav class="sidebar" aria-label={i18n.t("nav.servers")}>
  <div class="identity">
    <span class="glyph" aria-hidden="true">H</span>
    <div class="identity-text">
      <strong>H-Remote</strong>
      <span>{i18n.t("app.tagline")}</span>
    </div>
  </div>

  <div class="nav">
    {#each items as item (item.route)}
      <button
        type="button"
        class="nav-item"
        class:active={route === item.route}
        aria-current={route === item.route ? "page" : undefined}
        data-testid={`nav-${item.route}`}
        onclick={() => onnavigate(item.route)}
      >
        <Icon name={item.icon} size={18} />
        <span class="nav-label">{i18n.t(item.label)}</span>
      </button>
    {/each}
  </div>

  <div class="footer">
    {#if appInfo}
      <span class="version">v{appInfo.version}</span>
    {/if}
    {#if paths}
      <span class="profile" title={paths.baseDir}>
        {paths.profileMode === "portable" ? i18n.t("profile.portable") : i18n.t("profile.appdata")}
      </span>
    {/if}
  </div>
</nav>

<style>
  .sidebar {
    width: var(--hr-sidebar-width);
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 18px;
    padding: 16px 12px 14px;
    background: color-mix(in srgb, var(--hr-primary-mid) 88%, var(--hr-background));
    border-right: 1px solid var(--hr-border);
  }

  .identity {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 2px 8px 14px;
    border-bottom: 1px solid var(--hr-border);
  }

  .glyph {
    width: 34px;
    height: 34px;
    border-radius: 10px;
    display: grid;
    place-items: center;
    font-weight: 700;
    font-size: 16px;
    color: var(--hr-accent-text);
    background: linear-gradient(145deg, var(--hr-accent-light), var(--hr-accent-dark));
    flex: none;
  }

  .identity-text {
    display: flex;
    flex-direction: column;
    min-width: 0;
    line-height: 1.25;
  }

  .identity-text strong {
    font-size: var(--hr-font-size-subtitle);
  }

  .identity-text span {
    font-size: 10px;
    color: color-mix(in srgb, var(--hr-primary-text) 55%, transparent);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .nav {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 10px;
    border: 0;
    background: transparent;
    color: color-mix(in srgb, var(--hr-primary-text) 78%, transparent);
    border-radius: var(--hr-radius);
    padding: 9px 10px;
    text-align: left;
    transition:
      background var(--hr-transition),
      color var(--hr-transition);
  }

  .nav-item:hover {
    background: color-mix(in srgb, var(--hr-primary-text) 9%, transparent);
    color: var(--hr-primary-text);
  }

  .nav-item.active {
    background: color-mix(in srgb, var(--hr-accent-mid) 20%, transparent);
    color: var(--hr-accent-light);
    font-weight: 550;
  }

  .nav-label {
    white-space: nowrap;
  }

  .footer {
    margin-top: auto;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
    padding: 10px 8px 0;
    border-top: 1px solid var(--hr-border);
    font-size: 10px;
    color: color-mix(in srgb, var(--hr-primary-text) 55%, transparent);
  }

  .profile {
    border: 1px solid var(--hr-border);
    border-radius: 999px;
    padding: 1px 7px;
  }

  @media (max-width: 860px) {
    .identity-text,
    .nav-label,
    .footer {
      display: none;
    }

    .nav-item {
      justify-content: center;
    }
  }
</style>
