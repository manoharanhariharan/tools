<script lang="ts">
  import { i18n } from "../i18n/index.svelte";
  import { dialogs } from "../state/dialogs.svelte";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";
  import Modal from "./Modal.svelte";
  import TextField from "./TextField.svelte";

  let inputValue = $state("");
  let reveal = $state(false);

  const current = $derived(dialogs.current);

  $effect(() => {
    const request = dialogs.current;
    if (request?.kind === "input") {
      inputValue = request.value;
    } else if (request?.kind === "password") {
      inputValue = "";
      reveal = false;
    }
  });

  function dismiss() {
    const request = dialogs.current;
    if (request?.kind === "message") {
      dialogs.close(request.buttons[0]?.value ?? null);
    } else {
      dialogs.close(null);
    }
  }

  function submit() {
    const request = dialogs.current;
    if (!request) return;
    if (request.kind === "message") {
      dialogs.close(request.buttons[0]?.value ?? "ok");
    } else {
      dialogs.close(inputValue);
    }
  }
</script>

{#if current}
  <Modal
    open={true}
    title={current.title}
    description={current.kind === "message" ? undefined : current.message || undefined}
    width="sm"
    dismissible={current.kind !== "password"}
    onclose={dismiss}
  >
    {#if current.kind === "message"}
      <div class="message message-{current.variant}">
        <Icon
          name={current.variant === "error" ? "error" : current.variant === "warning" ? "warning" : "info"}
          size={22}
        />
        <p>{current.message}</p>
      </div>
    {:else if current.kind === "input"}
      <TextField
        id="dialog-input"
        label={current.label}
        value={inputValue}
        placeholder={current.placeholder}
        oninput={(value) => (inputValue = value)}
        onkeydown={(event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            submit();
          }
        }}
      />
    {:else}
      <div class="password-field">
        <label for="dialog-password">{current.label}</label>
        <div class="password-row">
          <input
            id="dialog-password"
            type={reveal ? "text" : "password"}
            bind:value={inputValue}
            autocomplete="current-password"
            onkeydown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                submit();
              }
            }}
          />
          <button
            type="button"
            class="reveal"
            aria-label={reveal ? "Hide password" : "Show password"}
            aria-pressed={reveal}
            onclick={() => (reveal = !reveal)}
          >
            <Icon name="key" size={16} />
          </button>
        </div>
      </div>
    {/if}

    {#snippet footer()}
      {#if current.kind === "message"}
        {#each current.buttons as button (button.value)}
          <Button
            variant={button.variant ?? "secondary"}
            onclick={() => dialogs.close(button.value)}
          >
            {button.label}
          </Button>
        {/each}
      {:else}
        <Button variant="ghost" onclick={() => dialogs.close(null)}>{i18n.t("common.cancel")}</Button>
        <Button variant="primary" onclick={submit}>
          {current.kind === "input" ? current.submitLabel : i18n.t("common.ok")}
        </Button>
      {/if}
    {/snippet}
  </Modal>
{/if}

<style>
  .message {
    display: flex;
    gap: 12px;
    align-items: flex-start;
    padding: 6px 0 10px;
  }

  .message p {
    line-height: 1.45;
    white-space: pre-wrap;
  }

  .message-info {
    color: var(--hr-info);
  }

  .message-warning {
    color: var(--hr-warning);
  }

  .message-error {
    color: var(--hr-danger);
  }

  .message p {
    color: var(--hr-background-text);
  }

  .password-field {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 4px 0 8px;
  }

  label {
    font-size: var(--hr-font-size-small);
    font-weight: 500;
  }

  .password-row {
    display: flex;
    gap: 8px;
  }

  .password-row input {
    flex: 1;
    min-height: 34px;
    border-radius: var(--hr-radius);
    border: 1px solid var(--hr-border);
    background: color-mix(in srgb, var(--hr-primary-dark) 55%, var(--hr-background));
    padding: 0 11px;
    color: var(--hr-background-text);
  }

  .reveal {
    width: 36px;
    border-radius: var(--hr-radius);
    border: 1px solid var(--hr-border);
    background: transparent;
    color: inherit;
    display: grid;
    place-items: center;
  }

  .reveal:hover {
    border-color: var(--hr-accent-mid);
  }
</style>
