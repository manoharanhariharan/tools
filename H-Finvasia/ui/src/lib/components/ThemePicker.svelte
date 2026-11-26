<script lang="ts">
  import { untrack } from "svelte";
  import Modal from "./Modal.svelte";
  import { ACCENTS, THEMES, applyTheme, type ThemeId } from "../theme";

  interface Props {
    theme: ThemeId;
    accent: string;
    onclose: () => void;
  }
  let { theme, accent, onclose }: Props = $props();

  let selectedTheme = $state<ThemeId>(untrack(() => theme));
  let selectedAccent = $state(untrack(() => accent));

  function chooseAccent(value: string) {
    selectedAccent = value.toUpperCase();
  }

  // No Apply step: every click is persisted and applied to the live DOM at once.
  $effect(() => {
    applyTheme({ theme: selectedTheme, accent: selectedAccent });
  });
</script>

<Modal
  title="Appearance"
  subtitle="Every choice applies instantly and is remembered on this machine."
  onclose={onclose}
  wide
>
  <div class="theme-grid">
    {#each THEMES as option (option.id)}
      <button
        class="theme-card"
        class:active={selectedTheme === option.id}
        type="button"
        onclick={() => (selectedTheme = option.id)}
      >
        <span class="theme-preview" style="background: {option.swatch}">
          <i style="background: {selectedAccent}"></i>
          <i></i>
          <i></i>
        </span>
        <strong>{option.label}</strong>
        <small>{option.description}</small>
      </button>
    {/each}
  </div>

  <div class="theme-section">
    <span class="theme-label">Accent</span>
    <div class="accent-row">
      {#each ACCENTS as option (option.id)}
        <button
          class="accent-swatch"
          class:active={selectedAccent.toUpperCase() === option.accent.toUpperCase()}
          type="button"
          title={option.label}
          style="background: {option.accent}"
          onclick={() => chooseAccent(option.accent)}
        ></button>
      {/each}
      <label class="accent-custom" title="Custom accent">
        <input
          type="color"
          value={selectedAccent}
          oninput={(event) => chooseAccent(event.currentTarget.value)}
        />
        <span>Custom</span>
      </label>
    </div>
  </div>

  <div class="modal-actions">
    <span class="theme-live">Applied live - already saved.</span>
    <button class="primary" onclick={onclose}>Done</button>
  </div>
</Modal>
