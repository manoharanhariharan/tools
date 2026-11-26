<script lang="ts">
  import type { AccountSummaryDto, SourceKind } from "../../api";
  import { loadHeaderCollapsed, saveHeaderCollapsed } from "../../layoutPrefs";

  interface Props {
    summary: AccountSummaryDto | null;
    loading: boolean;
    onrefresh: () => void;
    source: SourceKind;
  }
  let { summary, loading, onrefresh, source }: Props = $props();

  let collapsed = $state(loadHeaderCollapsed());

  const funds = $derived(summary?.funds ?? null);
  const session = $derived(summary?.session ?? null);
  const feedLabel = $derived(
    source === "replay" ? "replay" : summary?.connected ? "logged in" : "offline",
  );
  const feedTitle = $derived(
    source === "replay"
      ? "replaying a capture, no broker connection"
      : summary?.connected
        ? "websocket connected"
        : "not connected",
  );

  function toggle() {
    collapsed = !collapsed;
    saveHeaderCollapsed(collapsed);
  }

  function signed(value: string | null | undefined): string {
    if (!value) return "-";
    const n = Number(value);
    if (Number.isFinite(n) && n > 0) return `+${value}`;
    return value;
  }

  function tone(value: string | null | undefined): string {
    const n = Number(value ?? 0);
    if (!Number.isFinite(n) || n === 0) return "flat";
    return n > 0 ? "up" : "down";
  }
</script>

<section class="account-strip" class:collapsed>
  {#if collapsed}
    <div class="acct-line">
      <button
        class="acct-toggle"
        type="button"
        title="Expand account details"
        aria-expanded="false"
        onclick={toggle}>v</button
      >
      <span
        class="badge {source === 'replay' ? 'warn' : summary?.connected ? 'ok' : 'warn'}"
        title={feedTitle}
      >
        <span class="dot"></span>{feedLabel}
      </span>
      <span class="acct-line-item">Account <strong class="mono">{session?.actid || "-"}</strong></span>
      <span class="acct-line-item">Cash <strong class="mono">{funds?.cash ?? "-"}</strong></span>
      <span class="acct-line-item">RPNL
        <strong class="mono {tone(summary?.totals.rpnl)}">{signed(summary?.totals.rpnl)}</strong></span
      >
      <span class="acct-line-item">MTM
        <strong class="mono {tone(summary?.totals.mtm)}">{signed(summary?.totals.mtm)}</strong></span
      >
      <span class="acct-line-item dim">
        {summary?.positions ?? 0} position{(summary?.positions ?? 0) === 1 ? "" : "s"}
      </span>
      <span class="spacer"></span>
      <button class="icon" title="Refresh account" disabled={loading} onclick={onrefresh}>
        {loading ? "..." : "refresh"}
      </button>
    </div>
  {:else}
    <div class="acct-card">
      <div class="acct-head">
        <span class="acct-label">Account details</span>
        <span
          class="badge {source === 'replay' ? 'warn' : summary?.connected ? 'ok' : 'warn'}"
          title={feedTitle}
        >
          <span class="dot"></span>{feedLabel}
        </span>
      </div>
      <div class="acct-value mono">{session?.uid || "-"}</div>
      <div class="acct-sub">
        Account <strong>{session?.actid || "-"}</strong>
        {#if funds?.profile}&middot; {funds.profile}{/if}
      </div>
      <div class="acct-sub dim">
        {#if session?.has_stored_token}
          session stored{session.stored_date ? ` (${session.stored_date})` : ""}
        {:else if session?.has_credentials}
          credentials found, not logged in
        {:else}
          no session
        {/if}
        {#if summary}&middot; {summary.positions} position{summary.positions === 1 ? "" : "s"}{/if}
      </div>
    </div>

    <div class="acct-card">
      <div class="acct-head">
        <span class="acct-label">Funds available</span>
        <button class="icon" title="Refresh account" disabled={loading} onclick={onrefresh}>
          {loading ? "..." : "refresh"}
        </button>
      </div>
      <div class="acct-value mono">{funds?.cash ?? "-"}</div>
      <div class="acct-sub dim">
        Blocked <strong>{funds?.blocked ?? "-"}</strong>
      </div>
      <div class="acct-sub dim">
        Turnover <strong>{funds?.turnover ?? "-"}</strong>
      </div>
    </div>

    <div class="acct-card">
      <div class="acct-head">
        <span class="acct-label">RPNL</span>
      </div>
      <div class="acct-value mono {tone(summary?.totals.rpnl)}">{signed(summary?.totals.rpnl)}</div>
      <div class="acct-sub dim">realised, day</div>
    </div>

    <div class="acct-card">
      <div class="acct-head">
        <span class="acct-label">MTM</span>
      </div>
      <div class="acct-value mono {tone(summary?.totals.mtm)}">{signed(summary?.totals.mtm)}</div>
      <div class="acct-sub dim">unrealised, live</div>
    </div>

    <div class="acct-toggle-col">
      <button
        class="acct-toggle"
        type="button"
        title="Collapse to a single band"
        aria-expanded="true"
        onclick={toggle}>^</button
      >
    </div>
  {/if}
</section>
