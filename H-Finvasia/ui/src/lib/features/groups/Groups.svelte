<script lang="ts">
  import { onMount } from "svelte";
  import { api, type GroupSummary } from "../../api";
  import EmptyState from "../../components/EmptyState.svelte";

  interface Props {
    watchCount: number;
  }
  let { watchCount }: Props = $props();

  let groups = $state<GroupSummary[]>([]);
  let name = $state("");
  let error = $state<string | null>(null);
  let notice = $state<string | null>(null);
  let busy = $state(false);

  async function refresh() {
    try {
      groups = await api.groupsList();
    } catch (e) {
      error = String(e);
    }
  }

  async function saveCurrent() {
    if (name.trim() === "") return;
    busy = true;
    error = null;
    notice = null;
    try {
      await api.groupSave(name.trim());
      notice = `Saved "${name.trim()}" with ${watchCount} instruments.`;
      name = "";
      await refresh();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function load(group: string) {
    busy = true;
    error = null;
    notice = null;
    try {
      const plan = await api.groupLoad(group);
      if (plan.truncated) {
        notice = `Loaded "${group}" but ${plan.rejected.length} instrument(s) exceeded the 100 budget.`;
      } else {
        notice = `Loaded "${group}".`;
      }
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function remove(group: string) {
    busy = true;
    error = null;
    try {
      await api.groupDelete(group);
      await refresh();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  onMount(() => {
    void refresh();
  });
</script>

<section class="panel">
  <header class="panel-head">
    <h2>Watch groups</h2>
    <span class="muted">{groups.length} saved</span>
    <span class="spacer"></span>
    <button disabled={busy} onclick={refresh}>Refresh</button>
  </header>

  {#if error}
    <div class="banner"><span>{error}</span></div>
  {/if}
  {#if notice}
    <div class="banner" style="border-color: rgba(61,199,126,.35); background: rgba(61,199,126,.08); color: #9fe3bf">
      <span>{notice}</span>
      <button class="ghost" onclick={() => (notice = null)}>Dismiss</button>
    </div>
  {/if}

  <div class="chain-toolbar">
    <div class="field" style="flex: 1; min-width: 180px">
      <label for="group-name">Save the current watch list as</label>
      <input
        id="group-name"
        bind:value={name}
        placeholder="e.g. Nifty weekly"
        onkeydown={(e) => {
          if (e.key === "Enter") void saveCurrent();
        }}
      />
    </div>
    <button class="primary" disabled={busy || name.trim() === "" || watchCount === 0} onclick={saveCurrent}>
      Save ({watchCount})
    </button>
  </div>

  {#if groups.length === 0}
    <EmptyState
      title="No saved groups"
      hint="Build a watch list, then save it as a named group. Groups are local files — no broker calls."
    />
  {:else}
    <div class="grid-wrap">
      <table class="grid">
        <thead>
          <tr>
            <th class="l">Group</th>
            <th>Instruments</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          {#each groups as group (group.name)}
            <tr>
              <td class="l mono">{group.name}</td>
              <td class="num mono">{group.count}</td>
              <td class="num">
                <button disabled={busy} onclick={() => load(group.name)}>Load</button>
                <button class="icon" title="Delete group" onclick={() => remove(group.name)}>×</button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}

  <p class="hint" style="padding: 10px 14px; border-top: 1px solid var(--border)">
    Loading a group replaces the watch list and re-subscribes within the 100-instrument budget.
    Groups are stored in the app config folder as JSON.
  </p>
</section>
