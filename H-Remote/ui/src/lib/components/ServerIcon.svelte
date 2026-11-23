<script lang="ts">
  import type { Server } from "../ipc/contracts";
  import { toCssColor } from "../theme/apply";

  interface Props {
    server: Server;
    size?: number;
  }
  let { server, size = 34 }: Props = $props();

  const initials = $derived(
    server.displayName
      .split(/\s+/)
      .filter(Boolean)
      .slice(0, 2)
      .map((part) => part[0]?.toUpperCase())
      .join("") || "?",
  );
  const background = $derived(
    server.colorHex ? toCssColor(server.colorHex) : "var(--hr-accent-mid)",
  );
</script>

{#if server.iconBase64}
  <img
    class="icon"
    style:width="{size}px"
    style:height="{size}px"
    src="data:image/png;base64,{server.iconBase64}"
    alt=""
    loading="lazy"
    decoding="async"
  />
{:else}
  <span
    class="fallback"
    style:width="{size}px"
    style:height="{size}px"
    style:background
    aria-hidden="true"
  >{initials}</span>
{/if}

<style>
  .icon {
    border-radius: 9px;
    object-fit: cover;
    flex: none;
  }

  .fallback {
    border-radius: 9px;
    display: grid;
    place-items: center;
    color: var(--hr-accent-text);
    font-weight: 650;
    font-size: calc(var(--icon-size, 34px) * 0.34);
    flex: none;
    letter-spacing: 0.02em;
  }
</style>
