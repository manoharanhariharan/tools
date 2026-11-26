<script lang="ts">
  import { onMount } from "svelte";
  import { api, type GroupSummary } from "../../api";
  import Modal from "../../components/Modal.svelte";

  interface Props {
    instrumentKey: string;
    instrumentLabel: string;
    onclose: () => void;
    onadded: (message: string) => void;
  }
  let { instrumentKey, instrumentLabel, onclose, onadded }: Props = $props();

  let groups = $state<GroupSummary[]>([]);
  let selected = $state("");
  let newName = $state("");
  let busy = $state(false);
  let error = $state<string | null>(null);

  async function load() {
    try {
      const view = await api.containersList();
      groups = view.groups;
      if (!groups.some((group) => group.name === selected)) {
        selected = view.active ?? groups[0]?.name ?? "";
      }
    } catch (e) {
      error = String(e);
    }
  }

  onMount(() => {
    void load();
  });

  async function add() {
    if (busy) return;
    const target = newName.trim();
    busy = true;
    error = null;
    try {
      if (target !== "") {
        const view = await api.containerCreateEmpty(target);
        groups = view.groups;
        await api.containerAdd(target, instrumentKey);
        onadded(`Created "${target}" and added ${instrumentLabel}.`);
      } else {
        if (selected === "") throw new Error("choose a card or type a new name");
        await api.containerAdd(selected, instrumentKey);
        onadded(`Added ${instrumentLabel} to "${selected}".`);
      }
      onclose();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<Modal
  title="Add to a Stocks card"
  subtitle={`${instrumentLabel} can be added to an existing group or a new card.`}
  {onclose}
>
  {#if error}
    <div class="banner"><span>{error}</span></div>
  {/if}

  <div class="group-pick-list" role="radiogroup" aria-label="Target group">
    {#each groups as group (group.name)}
      <label class="group-pick" class:active={selected === group.name && newName.trim() === ""}>
        <input
          type="radio"
          name="target-group"
          value={group.name}
          checked={selected === group.name}
          onchange={() => {
            selected = group.name;
            newName = "";
          }}
        />
        <span class="group-card-dot" style="background: {group.color ?? 'var(--accent-2)'}"></span>
        <span class="group-pick-name">{group.name}</span>
        <span class="muted">{group.count}</span>
      </label>
    {/each}
  </div>

  <div class="field">
    <label for="new-card-name">Or create a new card</label>
    <div class="row">
      <input id="new-card-name" bind:value={newName} placeholder="e.g. Nifty hedges" />
      <button class="primary" disabled={busy || newName.trim() === ""} onclick={add}>
        {busy ? "Adding..." : "Create + add"}
      </button>
    </div>
  </div>

  <div class="modal-actions">
    <button class="ghost" disabled={busy} onclick={onclose}>Cancel</button>
    <button
      class="primary"
      disabled={busy || newName.trim() !== "" || selected === ""}
      onclick={add}
    >
      {busy ? "Adding..." : `Add to ${selected || "card"}`}
    </button>
  </div>
</Modal>
