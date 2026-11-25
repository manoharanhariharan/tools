<script lang="ts">
  import { untrack } from "svelte";
  import Modal from "../../components/Modal.svelte";
  import Button from "../../components/Button.svelte";
  import { sessions } from "../../state/sessions.svelte";

  const presets = [
    { width: 1024, height: 768 },
    { width: 1280, height: 720 },
    { width: 1366, height: 768 },
    { width: 1600, height: 900 },
    { width: 1920, height: 1080 },
    { width: 2560, height: 1440 },
    { width: 3840, height: 2160 },
  ];

  const session = $derived(
    sessions.sessions.find((candidate) => candidate.id === sessions.displaySessionId) ?? null,
  );

  let width = $state(1920);
  let height = $state(1080);
  let monitors = $state(1);
  let initializedFor = "";

  $effect(() => {
    const id = sessions.displaySessionId;
    if (!id) {
      initializedFor = "";
      return;
    }
    if (initializedFor === id) {
      return;
    }
    initializedFor = id;
    untrack(() => {
      const current = sessions.sessions.find((candidate) => candidate.id === id);
      const currentOverride = sessions.displayOverrides[id];
      width = currentOverride?.width ?? Math.max(640, current?.width ?? 1920);
      height = currentOverride?.height ?? Math.max(480, current?.height ?? 1080);
      monitors = currentOverride?.monitors ?? 1;
    });
  });

  const totalWidth = $derived(width * monitors);
  const maxMonitors = $derived(Math.max(1, Math.min(4, Math.floor(8192 / width))));

  async function apply(): Promise<void> {
    if (!session) return;
    await sessions.applyDisplay(session.id, width, height, monitors);
    sessions.closeDisplay();
  }

  async function fit(): Promise<void> {
    if (!session) return;
    await sessions.resetDisplay(session.id);
    sessions.closeDisplay();
  }

  function clampWidth(value: number): void {
    width = Math.min(3840, Math.max(640, Math.round(value) || 1920));
  }

  function clampHeight(value: number): void {
    height = Math.min(2160, Math.max(480, Math.round(value) || 1080));
  }
</script>

<Modal
  open={Boolean(session)}
  title="Session display"
  description={session?.serverName ?? ""}
  width="md"
  onclose={() => sessions.closeDisplay()}
>
  {#if session}
    <div class="hr-stack">
      <div class="presets">
        {#each presets as preset (preset.width)}
          <button
            type="button"
            class:active={width === preset.width && height === preset.height}
            onclick={() => {
              width = preset.width;
              height = preset.height;
            }}
          >
            {preset.width}×{preset.height}
          </button>
        {/each}
      </div>

      <div class="row">
        <label class="field">
          <span>Width</span>
          <input
            type="number"
            min="640"
            max="3840"
            value={width}
            onchange={(event) => clampWidth(Number(event.currentTarget.value))}
          />
        </label>
        <label class="field">
          <span>Height</span>
          <input
            type="number"
            min="480"
            max="2160"
            value={height}
            onchange={(event) => clampHeight(Number(event.currentTarget.value))}
          />
        </label>
        <label class="field">
          <span>Virtual monitors</span>
          <select
            value={String(monitors)}
            onchange={(event) => (monitors = Number(event.currentTarget.value))}
          >
            <option value="1">1</option>
            <option value="2" disabled={maxMonitors < 2}>2 (wide desktop)</option>
            <option value="3" disabled={maxMonitors < 3}>3 (wide desktop)</option>
            <option value="4" disabled={maxMonitors < 4}>4 (wide desktop)</option>
          </select>
        </label>
      </div>

      <p class="hr-small hr-muted">
        Fixed mode suspends automatic resizing. Canvas:
        <strong>{totalWidth}×{height}</strong>. “Fit to window” keeps the current remote pixel
        size and only scales the rendering.
      </p>
      {#if monitors > 1}
        <p class="hr-small note">
          The embedded engine exposes these as one wide remote desktop (Windows sees a single
          ultrawide monitor), not true per-monitor desktops. For real multi-monitor layout,
          enable “Launch with external mstsc” on the server.
        </p>
        {#if maxMonitors < monitors}
          <p class="hr-small note">
            This width is capped at {maxMonitors} wide screen(s) by the RDP 8192 px limit.
          </p>
        {/if}
      {/if}
    </div>
  {/if}

  {#snippet footer()}
    <Button variant="ghost" onclick={() => void fit()}>Fit to window</Button>
    <Button variant="secondary" onclick={() => sessions.closeDisplay()}>Cancel</Button>
    <Button variant="primary" onclick={() => void apply()}>Apply</Button>
  {/snippet}
</Modal>

<style>
  .presets {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }

  .presets button {
    border: 1px solid var(--hr-border);
    background: transparent;
    color: inherit;
    border-radius: 999px;
    padding: 4px 9px;
    font-size: 10px;
  }

  .presets button.active {
    border-color: var(--hr-accent-mid);
    background: color-mix(in srgb, var(--hr-accent-mid) 18%, transparent);
  }

  .row {
    display: flex;
    gap: 10px;
    align-items: flex-end;
    flex-wrap: wrap;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 10px;
    flex: 1;
    min-width: 100px;
  }

  input,
  select {
    min-height: 31px;
    border-radius: var(--hr-radius);
    border: 1px solid var(--hr-border);
    background: color-mix(in srgb, var(--hr-primary-dark) 55%, var(--hr-background));
    color: inherit;
    padding: 6px 8px;
    font: inherit;
    font-size: 11px;
  }

  .note {
    color: var(--hr-warning);
  }
</style>
