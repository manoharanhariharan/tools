<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { AvailabilityEntry } from "../../ipc/contracts";
  import { sessions } from "../../state/sessions.svelte";
  import Button from "../../components/Button.svelte";

  interface Props {
    entries: AvailabilityEntry[];
  }
  let { entries }: Props = $props();

  let seconds = $state(5);
  let timer: ReturnType<typeof setInterval> | undefined;
  const reachable = $derived(entries.some((entry) => entry.reachable));

  onMount(() => {
    timer = setInterval(() => {
      seconds -= 1;
      if (seconds <= 0) {
        finish(reachable);
      }
    }, 1000);
  });

  onDestroy(() => {
    if (timer) {
      clearInterval(timer);
    }
  });

  function finish(proceed: boolean): void {
    if (timer) {
      clearInterval(timer);
      timer = undefined;
    }
    sessions.resolveAvailability(proceed);
  }
</script>

<div class="layer" role="dialog" aria-modal="true" aria-label="Server availability">
  <div class="card">
    <h2>{reachable ? "Address reachable" : "Not able to connect"}</h2>
    <p>
      {#if reachable}
        At least one address responded. Connecting in {seconds} s…
      {:else}
        None of the configured addresses responded. Closing in {seconds} s…
      {/if}
    </p>
    <div class="list">
      {#each entries as entry (entry.name + entry.address + entry.port)}
        <div class="entry">
          <span class="dot" class:ok={entry.reachable}></span>
          <span class="name">{entry.name}</span>
          <span class="address">{entry.address}:{entry.port}</span>
          <span class="latency">
            {entry.reachable ? `${entry.latencyMs ?? 0} ms` : "unreachable"}
          </span>
        </div>
      {/each}
    </div>
    <div class="actions">
      {#if reachable}
        <Button variant="primary" onclick={() => finish(true)}>Connect now</Button>
      {:else}
        <Button variant="secondary" onclick={() => finish(true)}>Try anyway</Button>
      {/if}
      <Button variant="ghost" onclick={() => finish(false)}>Cancel</Button>
    </div>
  </div>
</div>

<style>
  .layer {
    position: fixed;
    inset: 0;
    z-index: 970;
    display: grid;
    place-items: center;
    background: rgba(4, 10, 16, 0.62);
    backdrop-filter: blur(2px);
  }

  .card {
    width: min(460px, 90vw);
    padding: 18px;
    border-radius: var(--hr-radius-large);
    border: 1px solid var(--hr-border-strong);
    background: color-mix(in srgb, var(--hr-primary-mid) 96%, var(--hr-background));
    box-shadow: var(--hr-shadow);
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  h2 {
    font-size: var(--hr-font-size-subtitle);
  }

  p {
    font-size: var(--hr-font-size-small);
    color: color-mix(in srgb, var(--hr-background-text) 62%, transparent);
  }

  .list {
    display: flex;
    flex-direction: column;
    gap: 3px;
    border: 1px solid var(--hr-border);
    border-radius: var(--hr-radius);
    padding: 6px;
  }

  .entry {
    display: grid;
    grid-template-columns: 12px 90px 1fr 70px;
    gap: 6px;
    align-items: center;
    font-size: 11px;
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--hr-danger);
  }

  .dot.ok {
    background: #49b675;
  }

  .address {
    color: color-mix(in srgb, var(--hr-background-text) 62%, transparent);
  }

  .latency {
    text-align: right;
    color: color-mix(in srgb, var(--hr-background-text) 55%, transparent);
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 6px;
  }
</style>
