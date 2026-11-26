<script lang="ts">
  import Badge from "../../components/Badge.svelte";
  import type { AppInfo, StatusDto } from "../../api";

  interface Props {
    status: StatusDto | null;
    info: AppInfo | null;
    busy: boolean;
    notice: string | null;
    ondemo: () => void;
    onstop: () => void;
    onlive: (uid: string, actid: string) => void;
    onnotice: (message: string | null) => void;
  }
  let { status, info, busy, notice, ondemo, onstop, onlive, onnotice }: Props = $props();

  let uid = $state("");
  let actid = $state("");

  const connected = $derived(status?.connected ?? false);
  const tokenMissing = $derived(info ? !info.has_token : false);
</script>

<div class="stack">
  {#if notice}
    <div class="banner">
      <span>{notice}</span>
      <button class="ghost" onclick={() => onnotice(null)}>Dismiss</button>
    </div>
  {/if}

  <div class="section">
    <h3>Feed</h3>
    <div class="row" style="margin-bottom: 10px">
      <Badge
        label={connected ? `connected (${status?.source})` : "disconnected"}
        tone={connected ? "ok" : "warn"}
        dot
      />
      <Badge label={`backend: ${status?.backend ?? "…"}`} tone={status?.backend === "mongo" ? "ok" : "warn"} />
      <Badge label={`${status?.instruments?.toLocaleString() ?? 0} instruments`} />
      <Badge label={`${status?.watched ?? 0} watching`} />
    </div>
    <div class="row">
      <button class="primary" disabled={busy} onclick={ondemo}>
        {busy ? "Starting…" : "Start demo feed (replay)"}
      </button>
      <button disabled={busy || !connected} onclick={onstop}>Stop feed</button>
    </div>
    <p class="hint" style="margin-top: 10px">
      The demo feed replays a bundled capture at ~120 frames/s so you can see the grid and
      snap quote update without credentials or market hours.
    </p>
  </div>

  <div class="section">
    <h3>Live feed</h3>
    <div class="row" style="margin-bottom: 10px">
      <Badge
        label={tokenMissing ? "FV_ACCESS_TOKEN not set" : "token present"}
        tone={tokenMissing ? "warn" : "ok"}
        dot
      />
    </div>
    <div class="row" style="margin-bottom: 10px">
      <div class="field" style="flex: 1">
        <label for="uid">User id</label>
        <input id="uid" bind:value={uid} placeholder="AB1234" autocomplete="off" />
      </div>
      <div class="field" style="flex: 1">
        <label for="actid">Account id</label>
        <input id="actid" bind:value={actid} placeholder="AB1234" autocomplete="off" />
      </div>
    </div>
    <button disabled={busy || tokenMissing || !uid.trim() || !actid.trim()} onclick={() => onlive(uid, actid)}>
      Connect live
    </button>
    <p class="hint" style="margin-top: 10px">
      Requires an OAuth access token in <code>FV_ACCESS_TOKEN</code> and at least one instrument in
      the watch list. Set the token before launching the app.
    </p>
  </div>

  <div class="section">
    <h3>Runtime</h3>
    <dl class="kv">
      <dt>Build</dt>
      <dd>{status?.build ?? "—"}</dd>
      <dt>Source</dt>
      <dd>{status?.source ?? "—"}</dd>
      <dt>Frames ingested</dt>
      <dd>{status?.frames_total?.toLocaleString() ?? 0}</dd>
      <dt>Decode errors</dt>
      <dd>{status?.frames_error ?? 0}</dd>
      <dt>Frames / second</dt>
      <dd>{status?.frames_per_sec ?? 0}</dd>
      <dt>Watch limit</dt>
      <dd>{status?.watch_limit ?? 0}</dd>
    </dl>
    {#if status?.last_error}
      <p class="hint down" style="margin-top: 10px; word-break: break-word">{status.last_error}</p>
    {/if}
  </div>

  <div class="section">
    <h3>Not implemented yet</h3>
    <p class="hint">
      Order entry, order/trade/position books and holdings need the REST client plus an authorised
      session. They are the next slice (Phase 2b) and are intentionally absent rather than faked.
    </p>
  </div>
</div>
