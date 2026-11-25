<script lang="ts">
  type IconName =
    | "servers"
    | "settings"
    | "about"
    | "info"
    | "close"
    | "minimize"
    | "maximize"
    | "restore"
    | "check"
    | "warning"
    | "error"
    | "chevron-left"
    | "chevron-right"
    | "arrow-up"
    | "arrow-down"
    | "folder"
    | "external"
    | "refresh"
    | "plus"
    | "search"
    | "palette"
    | "language"
    | "terminal"
    | "key"
    | "spinner";

  interface Props {
    name: IconName;
    size?: number;
    title?: string;
  }

  let { name, size = 18, title }: Props = $props();
</script>

<svg
  class="hr-icon"
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.7"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden={title ? undefined : "true"}
  role={title ? "img" : undefined}
>
  {#if title}<title>{title}</title>{/if}
  {#if name === "servers"}
    <rect x="3" y="4" width="18" height="7" rx="2" />
    <rect x="3" y="13" width="18" height="7" rx="2" />
    <path d="M7 7.5h.01M7 16.5h.01M11 7.5h6M11 16.5h6" />
  {:else if name === "settings"}
    <circle cx="12" cy="12" r="3" />
    <path
      d="M19.4 15a1.7 1.7 0 0 0 .34 1.87l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-1.87-.34 1.7 1.7 0 0 0-1 1.55V21a2 2 0 1 1-4 0v-.09a1.7 1.7 0 0 0-1.11-1.55 1.7 1.7 0 0 0-1.87.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.7 1.7 0 0 0 4.6 15a1.7 1.7 0 0 0-1.55-1H3a2 2 0 1 1 0-4h.09A1.7 1.7 0 0 0 4.6 8.9a1.7 1.7 0 0 0-.34-1.87l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.7 1.7 0 0 0 9 4.6h.08A1.7 1.7 0 0 0 10 3.05V3a2 2 0 1 1 4 0v.09a1.7 1.7 0 0 0 1 1.55 1.7 1.7 0 0 0 1.87-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.7 1.7 0 0 0-.34 1.87V9c.14.6.68 1.02 1.3 1H21a2 2 0 1 1 0 4h-.09a1.7 1.7 0 0 0-1.51 1z"
    />
  {:else if name === "about"}
    <circle cx="12" cy="12" r="9" />
    <path d="M12 11v5M12 8h.01" />
  {:else if name === "info"}
    <circle cx="12" cy="12" r="9" />
    <path d="M12 11v5M12 8h.01" />
  {:else if name === "warning"}
    <path d="M10.3 3.8 2.6 17a2 2 0 0 0 1.7 3h15.4a2 2 0 0 0 1.7-3L13.7 3.8a2 2 0 0 0-3.4 0z" />
    <path d="M12 9v4M12 16h.01" />
  {:else if name === "error"}
    <circle cx="12" cy="12" r="9" />
    <path d="m9 9 6 6M15 9l-6 6" />
  {:else if name === "close"}
    <path d="m6 6 12 12M18 6 6 18" />
  {:else if name === "minimize"}
    <path d="M6 12h12" />
  {:else if name === "maximize"}
    <rect x="5" y="5" width="14" height="14" rx="1.5" />
  {:else if name === "restore"}
    <rect x="8" y="8" width="11" height="11" rx="1.5" />
    <path d="M5 15V6.5A1.5 1.5 0 0 1 6.5 5H15" />
  {:else if name === "check"}
    <path d="m5 12 4.5 4.5L19 7" />
  {:else if name === "chevron-left"}
    <path d="m14 6-6 6 6 6" />
  {:else if name === "chevron-right"}
    <path d="m10 6 6 6-6 6" />
  {:else if name === "arrow-up"}
    <path d="M12 19V5M6 11l6-6 6 6" />
  {:else if name === "arrow-down"}
    <path d="M12 5v14M6 13l6 6 6-6" />
  {:else if name === "folder"}
    <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
  {:else if name === "external"}
    <path d="M14 4h6v6M20 4l-9 9" />
    <path d="M19 14v4a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2h4" />
  {:else if name === "refresh"}
    <path d="M20 12a8 8 0 1 1-2.34-5.66" />
    <path d="M20 4v6h-6" />
  {:else if name === "plus"}
    <path d="M12 5v14M5 12h14" />
  {:else if name === "search"}
    <circle cx="11" cy="11" r="6.5" />
    <path d="m16 16 4.5 4.5" />
  {:else if name === "palette"}
    <path
      d="M12 3a9 9 0 1 0 0 18h1.2a2.3 2.3 0 0 0 1.7-3.85 2.3 2.3 0 0 1 1.7-3.85H18a3 3 0 0 0 3-3A9 9 0 0 0 12 3z"
    />
    <path d="M7.5 10h.01M10 7h.01M14 7h.01M17 10h.01" />
  {:else if name === "language"}
    <circle cx="12" cy="12" r="9" />
    <path d="M3 12h18M12 3c2.5 2.5 3.8 5.6 3.8 9S14.5 18.5 12 21M12 3C9.5 5.5 8.2 8.6 8.2 12s1.3 6.5 3.8 9" />
  {:else if name === "terminal"}
    <rect x="3" y="4" width="18" height="16" rx="2" />
    <path d="m7 9 3 3-3 3M13 15h4" />
  {:else if name === "key"}
    <circle cx="8" cy="15" r="4" />
    <path d="m11 12 8-8M17 6l2 2M15 8l2 2" />
  {:else if name === "spinner"}
    <path d="M12 3a9 9 0 1 0 9 9" />
  {/if}
</svg>

<style>
  .hr-icon {
    flex: none;
    display: block;
  }
</style>
