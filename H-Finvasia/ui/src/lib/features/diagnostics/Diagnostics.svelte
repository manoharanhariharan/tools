<script lang="ts">
  import Badge from "../../components/Badge.svelte";
  import type { AppInfo, SessionInfo, StatusDto } from "../../api";

  interface Props {
    status: StatusDto | null;
    info: AppInfo | null;
    session: SessionInfo | null;
    busy: boolean;
    onlive: () => void;
  }
  let { status, info, session, busy, onlive }: Props = $props();

  const connected = $derived(status?.connected ?? false);
  const sessionReady = $derived(
    session !== null && (session.has_stored_token || session.has_credentials),
  );

  const sourceLabel = $derived(
    session === null
      ? "checking?"
      : session.source === "env"
        ? "token from FV_ACCESS_TOKEN"
        : session.source === "stored"
          ? "persisted session (MongoDB)"
          : session.source === "login"
            ? "will log in with TOTP"
            : "no credentials",
  );
</script>

<div class="stack">
  <div class="section">
    <h3>Feed</h3>
    <div class="row" style="margin-bottom: 10px">
      <Badge
        label={connected ? `connected (${status?.source})` : "disconnected"}
        tone={connected ? "ok" : "warn"}
        dot
      />
      <Badge label={`backend: ${status?.backend ?? "?"}`} tone={status?.backend === "mongo" ? "ok" : "warn"} />
      <Badge label={`${status?.instruments?.toLocaleString() ?? 0} instruments`} />
      <Badge label={`${status?.watched ?? 0} watching`} />
    </div>
    <p class="hint" style="margin-top: 10px">
      The feed starts with the live session below. There is no bundled replay switch in the UI;
      demo captures remain available to the headless CLI and CI.
    </p>
  </div>

  <div class="section">
    <h3>Live feed</h3>
    <div class="row" style="margin-bottom: 10px">
      <Badge
        label={sourceLabel}
        tone={sessionReady ? "ok" : "err"}
        dot
      />
    </div>
    <dl class="kv" style="margin-bottom: 10px">
      <dt>User id</dt>
      <dd>{session?.uid || "?"}</dd>
      <dt>Account id</dt>
      <dd>{session?.actid || "?"}</dd>
      <dt>Stored session</dt>
      <dd>{session?.has_stored_token ? `yes (${session.stored_date ?? "date unknown"})` : "no"}</dd>
      <dt>Credential file</dt>
      <dd>{session?.has_credentials ? "found" : "not found"}</dd>
    </dl>
    <div class="row">
      <button class="primary" disabled={busy || !sessionReady} onclick={onlive}>
        {busy ? "Connecting?" : "Connect live"}
      </button>
    </div>
    <p class="hint" style="margin-top: 10px">
      Uses the session already persisted in <code>STOCK.CONFIGS_GLOBAL</code> when it still
      validates, otherwise logs in with the TOTP secret from <code>fv-cred.json</code>.
      The stored token is only read, never overwritten, so other tools keep their session.
    </p>
  </div>

  <div class="section">
    <h3>Runtime</h3>
    <dl class="kv">
      <dt>Build</dt>
      <dd>{status?.build ?? "?"}</dd>
      <dt>Version</dt>
      <dd>{info?.version ?? "?"}</dd>
      <dt>Source</dt>
      <dd>{status?.source ?? "?"}</dd>
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
      Order entry, order/trade/position books and holdings are the next slice. They need the
      write-side REST routes and are intentionally absent rather than faked.
    </p>
  </div>
</div>
