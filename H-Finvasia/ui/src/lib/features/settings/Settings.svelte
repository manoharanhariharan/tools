<script lang="ts">
  import type { AppInfo, SessionInfo, StatusDto } from "../../api";
  import Diagnostics from "../diagnostics/Diagnostics.svelte";

  interface Props {
    status: StatusDto | null;
    info: AppInfo | null;
    session: SessionInfo | null;
    busy: boolean;
    onlive: () => void;
    onorderdefaults: () => void;
    onappearance: () => void;
  }
  let { status, info, session, busy, onlive, onorderdefaults, onappearance }: Props = $props();
</script>

<div class="stack">
  <div class="section">
    <h3>Workspace</h3>
    <div class="settings-actions">
      <button onclick={onorderdefaults}>
        <span class="settings-icon">#</span>
        <span>
          <strong>Order defaults</strong>
          <small>Lots, product, validity and default order type</small>
        </span>
      </button>
      <button onclick={onappearance}>
        <span class="settings-icon">◐</span>
        <span>
          <strong>Appearance</strong>
          <small>Themes, accents and live colour changes</small>
        </span>
      </button>
    </div>
  </div>

  <Diagnostics {status} {info} {session} {busy} {onlive} />
</div>
