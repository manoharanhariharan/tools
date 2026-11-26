<script lang="ts">
  import type {
    AlternateCredential,
    AppArgument,
    IconLibraryEntry,
    Server,
  } from "../../ipc/contracts";
  import { commands } from "../../ipc/client";
  import { i18n } from "../../i18n/index.svelte";
  import { app } from "../../state/app.svelte";
  import { catalog } from "../../state/catalog.svelte";
  import { dialogs } from "../../state/dialogs.svelte";
  import { toCssColor } from "../../theme/apply";
  import Button from "../../components/Button.svelte";
  import Icon from "../../components/Icon.svelte";
  import MarkdownView from "../../components/MarkdownView.svelte";
  import PasswordField from "../../components/PasswordField.svelte";
  import ServerIcon from "../../components/ServerIcon.svelte";
  import Toggle from "../../components/Toggle.svelte";
  import {
    STOCK_ICONS,
    stockIconDataUrl,
    stockIconPngBase64,
    type StockIcon,
  } from "./stockIcons";

  let section = $state("general");
  let notePreview = $state(false);
  let iconLibrary = $state<IconLibraryEntry[]>([]);
  let iconUrl = $state("");
  let libraryOpen = $state(false);
  let stockOpen = $state(false);

  const editor = $derived(catalog.editor);
  const draft = $derived(editor?.draft);
  const bulk = $derived(editor?.mode === "bulk");
  const isNew = $derived(editor?.mode === "add" || editor?.mode === "duplicate");
  const readOnly = $derived(draft ? catalog.isReadOnly(draft) : false);
  let extraFieldsText = $state("{}");
  let extraFieldsError = $state<string | null>(null);
  let extraFieldsDraftId = "";

  $effect(() => {
    if (draft && draft.id !== extraFieldsDraftId) {
      extraFieldsDraftId = draft.id;
      extraFieldsText = JSON.stringify(draft.extraFields ?? {}, null, 2);
      extraFieldsError = null;
    }
  });

  function updateExtraFields(text: string): void {
    extraFieldsText = text;
    try {
      const parsed = JSON.parse(text.trim() || "{}") as unknown;
      if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
        throw new Error("expected a JSON object");
      }
      update("extraFields", parsed);
      extraFieldsError = null;
    } catch (error) {
      extraFieldsError = error instanceof Error ? error.message : String(error);
    }
  }
  const protocol = $derived(
    catalog.snapshot?.protocols.find((candidate) => candidate.id.toLowerCase() === draft?.protocol.toLowerCase()),
  );
  const folders = $derived(
    (catalog.snapshot?.folders ?? [])
      .map((folder) => folder.path.join("/"))
      .filter((path, index, all) => all.indexOf(path) === index)
      .sort(),
  );
  const isAddressProtocol = $derived(
    ["rdp", "remoteApp", "vnc", "ssh", "telnet", "sftp", "ftp", "custom"].includes(draft?.protocol ?? ""),
  );
  const hasCredentials = $derived(
    ["rdp", "remoteApp", "vnc", "ssh", "telnet", "sftp", "ftp", "custom"].includes(draft?.protocol ?? ""),
  );
  const runnerOptions = $derived(
    (app.config?.runners.runners ?? []).filter(
      (runner) =>
        runner.protocol.toUpperCase() ===
        (draft?.protocol ?? "").toUpperCase(),
    ),
  );

  function value(field: string, fallback = ""): string {
    if (!draft) return "";
    if (bulk && !editor?.dirty.has(field)) return "";
    const raw = (draft as Record<string, unknown>)[field];
    return raw === null || raw === undefined ? fallback : String(raw);
  }

  function bool(field: keyof Server | string): boolean {
    return Boolean((draft as Record<string, unknown>)?.[field]);
  }

  function update(field: string, next: unknown): void {
    catalog.updateDraft(field as keyof Server, next as never);
  }

  function updateString(field: string, next: string): void {
    update(field, next);
  }

  function updateBool(field: string, next: boolean): void {
    update(field, next);
  }

  function folderValue(): string {
    return draft?.treeNodes?.join("/") ?? "";
  }

  function setFolder(value: string): void {
    catalog.updateDraft(
      "treeNodes",
      value ? value.split("/") : [],
    );
  }

  function tagsValue(): string {
    if (bulk && !editor?.dirty.has("tags")) return "";
    return draft?.tags.join(", ") ?? "";
  }

  function setTags(value: string): void {
    catalog.updateDraft(
      "tags",
      [...new Set(
        value
          .split(",")
          .map((tag) => tag.trim().replace(/^#/, "").toLowerCase())
          .filter(Boolean),
      )].sort(),
    );
  }

  function setColor(hex: string): void {
    updateString("colorHex", `#FF${hex.replace("#", "").toUpperCase()}`);
  }

  function colorForPicker(): string {
    return draft?.colorHex ? toCssColor(draft.colorHex).slice(0, 7) : "#007acc";
  }

  function fileToBase64(file: File): Promise<string> {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(String(reader.result).split(",")[1] ?? "");
      reader.onerror = () => reject(reader.error);
      reader.readAsDataURL(file);
    });
  }

  async function uploadIcon(event: Event): Promise<void> {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    if (file.size > 512 * 1024) {
      app.showNotice("Icon is larger than 512 KiB.", "warning");
      return;
    }
    try {
      const base64 = await fileToBase64(file);
      updateString("iconBase64", base64);
    } catch (reason) {
      await app.reportError(reason);
    } finally {
      input.value = "";
    }
  }

  function generateIcon(): void {
    const canvas = document.createElement("canvas");
    canvas.width = 64;
    canvas.height = 64;
    const context = canvas.getContext("2d");
    if (!context) return;
    const colors = ["#E83D61", "#007ACC", "#5FA291", "#FF7247", "#A82159", "#5FA291"];
    context.fillStyle = draft?.colorHex ? toCssColor(draft.colorHex) : colors[(draft?.displayName.length ?? 0) % colors.length];
    context.fillRect(0, 0, 64, 64);
    context.fillStyle = "rgba(255,255,255,0.15)";
    context.fillRect(0, 42, 64, 22);
    context.fillStyle = "#fff";
    context.font = "bold 28px system-ui";
    context.textAlign = "center";
    context.textBaseline = "middle";
    context.fillText((draft?.displayName?.[0] ?? "?").toUpperCase(), 32, 32);
    const base64 = canvas.toDataURL("image/png").split(",")[1] ?? "";
    updateString("iconBase64", base64);
  }

  async function openLibrary(): Promise<void> {
    iconLibrary = await commands.listIconLibrary();
    stockOpen = false;
    libraryOpen = true;
  }

  function toggleStockIcons(): void {
    libraryOpen = false;
    stockOpen = !stockOpen;
  }

  async function applyStockIcon(icon: StockIcon): Promise<void> {
    try {
      updateString("iconBase64", await stockIconPngBase64(icon));
      stockOpen = false;
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async function fetchIcon(): Promise<void> {
    if (!iconUrl.trim()) return;
    try {
      const dataUrl = await commands.fetchIcon(iconUrl.trim());
      updateString("iconBase64", dataUrl.split(",")[1] ?? "");
      iconUrl = "";
      app.showNotice("Icon downloaded", "info");
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async function saveIconToLibrary(): Promise<void> {
    if (!draft?.iconBase64) return;
    try {
      await commands.saveIconLibrary(draft.iconBase64);
      await openLibrary();
      app.showNotice("Icon saved to library", "info");
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  function addAlternate(): void {
    const list: AlternateCredential[] = [
      ...(draft?.alternateCredentials ?? []),
      { name: "", address: null, port: null, username: "", password: "", privateKey: null },
    ];
    update("alternateCredentials", list);
  }

  function updateAlternate(index: number, field: keyof AlternateCredential, next: unknown): void {
    const list = [...(draft?.alternateCredentials ?? [])];
    list[index] = { ...list[index], [field]: next };
    update("alternateCredentials", list);
  }

  function removeAlternate(index: number): void {
    update(
      "alternateCredentials",
      (draft?.alternateCredentials ?? []).filter((_, candidate) => candidate !== index),
    );
  }

  function addArgument(): void {
    const list: AppArgument[] = [
      ...(draft?.argumentList ?? []),
      {
        type: "normal",
        name: "",
        key: "",
        isNullable: false,
        addBlankAfterKey: true,
        addBlankAfterValue: false,
        value: "",
        description: "",
        selections: {},
      },
    ];
    update("argumentList", list);
  }

  function updateArgument(index: number, field: keyof AppArgument, next: unknown): void {
    const list = [...(draft?.argumentList ?? [])];
    list[index] = { ...list[index], [field]: next };
    update("argumentList", list);
  }

  function removeArgument(index: number): void {
    update(
      "argumentList",
      (draft?.argumentList ?? []).filter((_, candidate) => candidate !== index),
    );
  }

  function moveArgument(index: number, delta: number): void {
    const list = [...(draft?.argumentList ?? [])];
    const target = index + delta;
    if (target < 0 || target >= list.length) return;
    [list[index], list[target]] = [list[target], list[index]];
    update("argumentList", list);
  }

  function addTagFromSuggestion(tag: string): void {
    const current = new Set(draft?.tags ?? []);
    current.add(tag);
    catalog.updateDraft("tags", [...current].sort());
  }

  async function createCredential(): Promise<void> {
    const name = await dialogs.input({
      title: "New credential",
      label: "Credential name",
      submitLabel: "Next",
    });
    if (!name?.trim()) return;
    const username = await dialogs.input({
      title: "New credential",
      label: "Username",
      value: draft?.username ?? "",
      submitLabel: "Next",
    });
    if (username === null) return;
    const password = await dialogs.password({
      title: "New credential",
      label: "Password",
    });
    if (password === null) return;
    try {
      const credential = await commands.createCredential(name.trim(), username, password);
      if (catalog.snapshot) {
        catalog.snapshot.credentials = [...catalog.snapshot.credentials, credential];
      }
      update("inheritedCredentialName", credential.name);
      app.showNotice(`Credential '${credential.name}' created`, "info");
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async function testScript(kind: "before" | "after"): Promise<void> {    const script = kind === "before" ? draft?.commandBeforeConnected : draft?.commandAfterDisconnected;
    if (!script) return;
    try {
      const result = await commands.testScript(
        script,
        kind === "before" ? Boolean(draft?.hideCommandBeforeConnectedWindow) : true,
        draft?.id,
      );
      app.showNotice(
        result.success ? `Script exit ${result.code ?? 0}` : `Script failed (${result.code}): ${result.stderr}`,
        result.success ? "info" : "warning",
      );
    } catch (reason) {
      await app.reportError(reason);
    }
  }

  async function save(): Promise<void> {
    if (!draft) return;
    if (!draft.displayName.trim()) {
      app.showNotice("Display name is required.", "warning");
      return;
    }
    const ok = await catalog.saveEditor();
    if (ok) {
      app.showNotice("Server saved", "info");
    }
  }

  function title(): string {
    switch (editor?.mode) {
      case "add": return "Add server";
      case "duplicate": return "Duplicate server";
      case "bulk": return `Edit ${editor?.servers.length ?? 0} servers`;
      default: return "Edit server";
    }
  }

  function protocolValue(id: string): string {
    switch (id.toLowerCase()) {
      case "rdp": return "rdp";
      case "remoteapp": return "remoteApp";
      case "vnc": return "vnc";
      case "ssh": return "ssh";
      case "telnet": return "telnet";
      case "serial": return "serial";
      case "sftp": return "sftp";
      case "ftp": return "ftp";
      case "localapp": return "localApp";
      case "custom": return "custom";
      default: return "dummy";
    }
  }
</script>

{#if editor && draft}
  <aside class="editor" aria-label={title()}>
    <header>
      <div>
        <h2>{title()}</h2>
        <p>{draft.displayName || "Untitled"}</p>
      </div>
      <button type="button" class="close" aria-label="Close editor" onclick={() => catalog.closeEditor()}>
        <Icon name="close" size={16} />
      </button>
    </header>

    {#if bulk}
      <div class="bulk-hint">
        Only changed fields are applied. Empty fields mean “different values”.
      </div>
    {/if}

    {#if readOnly}
      <div class="readonly-hint" role="status">
        <strong>Read-only source</strong>
        <span>
          This server comes from {catalog.sourceLabel(draft.sourceId)} and can be connected to or
          duplicated, but not edited or deleted here.
        </span>
      </div>
    {/if}

    <nav class="tabs">
      {#each [
        ["general", "General"],
        ["connection", "Connection"],
        ["display", "Display"],
        ["advanced", "Advanced"],
        ["scripts", "Scripts"],
      ] as [id, label] (id)}
        <button type="button" class:active={section === id} onclick={() => (section = id)}>
          {label}
        </button>
      {/each}
    </nav>

    <div class="body">
      {#if section === "general"}
        <label class="field">
          <span>Display name</span>
          <input
            list="server-name-suggestions"
            value={value("displayName")}
            placeholder={bulk ? "— different values —" : "Server name"}
            oninput={(event) => updateString("displayName", event.currentTarget.value)}
          />
          <datalist id="server-name-suggestions">
            {#each catalog.snapshot?.servers ?? [] as candidate (candidate.id)}
              {#if candidate.id !== draft.id}
                <option value={candidate.displayName}></option>
              {/if}
            {/each}
          </datalist>
        </label>

        <div class="row">
          <label class="field">
            <span>Protocol</span>
            <select
              value={draft.protocol}
              disabled={!isNew || bulk}
              onchange={(event) => void catalog.changeEditorProtocol(event.currentTarget.value)}
            >
              {#each catalog.snapshot?.protocols ?? [] as item (item.id)}
                <option value={protocolValue(item.id)}>{item.displayName}</option>
              {/each}
            </select>
          </label>
          <label class="field">
            <span>Folder</span>
            <select value={folderValue()} onchange={(event) => setFolder(event.currentTarget.value)}>
              <option value="">Root</option>
              {#each folders as folder (folder)}
                <option value={folder}>{folder.replaceAll("/", " / ")}</option>
              {/each}
            </select>
          </label>
        </div>

        <div class="icon-row">
          <ServerIcon server={draft} size={52} />
          <div class="icon-actions">
            <Button size="sm" variant="secondary" onclick={generateIcon}>Generate</Button>
            <Button size="sm" variant="secondary" onclick={toggleStockIcons}>Icons</Button>
            <Button size="sm" variant="secondary" onclick={() => void openLibrary()}>Library</Button>
            <label class="file-button">
              Upload
              <input type="file" accept="image/png,image/jpeg" onchange={uploadIcon} />
            </label>
            <Button size="sm" variant="ghost" onclick={() => void saveIconToLibrary()}>Save to library</Button>
          </div>
        </div>
        <div class="row">
          <label class="field grow">
            <span>Icon URL</span>
            <input placeholder="https://example.com/icon.png" bind:value={iconUrl} />
          </label>
          <Button size="sm" variant="secondary" onclick={() => void fetchIcon()}>Fetch</Button>
        </div>

        <div class="row">
          <label class="field color">
            <span>Colour</span>
            <input type="color" value={colorForPicker()} oninput={(event) => setColor(event.currentTarget.value)} />
            <code>{draft.colorHex || "no colour"}</code>
          </label>
          <label class="field grow">
            <span>Tags (comma separated)</span>
            <input value={tagsValue()} placeholder={bulk ? "— different values —" : "prod, linux"} oninput={(event) => setTags(event.currentTarget.value)} />
          </label>
        </div>
        <div class="suggestions">
          {#each catalog.tags().slice(0, 8) as tag (tag.name)}
            <button type="button" onclick={() => addTagFromSuggestion(tag.name)}>#{tag.name}</button>
          {/each}
        </div>

        <label class="field">
          <span>Note (Markdown) <button type="button" class="inline-link" onclick={() => (notePreview = !notePreview)}>{notePreview ? "Edit" : "Preview"}</button></span>
          {#if notePreview}
            <div class="preview"><MarkdownView markdown={draft.note} /></div>
          {:else}
            <textarea rows="5" value={value("note")} oninput={(event) => updateString("note", event.currentTarget.value)}></textarea>
          {/if}
        </label>
      {/if}

      {#if section === "connection" && isAddressProtocol}
        <div class="row">
          <label class="field grow">
            <span>{draft.protocol === "serial" ? "Serial port" : "Address"}</span>
            <input value={draft.protocol === "serial" ? value("serialPort") : value("address")} oninput={(event) => updateString(draft.protocol === "serial" ? "serialPort" : "address", event.currentTarget.value)} />
          </label>
          {#if draft.protocol !== "serial"}
            <label class="field small">
              <span>Port</span>
              <input type="number" value={value("port")} oninput={(event) => update("port", Number(event.currentTarget.value))} />
            </label>
          {/if}
        </div>

        {#if hasCredentials}
          <div class="row">
            <label class="field grow">
              <span>Username</span>
              <input value={value("username")} oninput={(event) => updateString("username", event.currentTarget.value)} />
            </label>
            <label class="field grow">
              <span>Inherited credential</span>
              <select value={draft.inheritedCredentialName ?? ""} onchange={(event) => update("inheritedCredentialName", event.currentTarget.value || null)}>
                <option value="">None</option>
                {#each catalog.snapshot?.credentials ?? [] as credential (credential.id)}
                  <option value={credential.name}>{credential.name}</option>
                {/each}
              </select>
            </label>
            <Button size="sm" variant="ghost" onclick={() => void createCredential()}>New</Button>
          </div>
          <div class="row">
            <PasswordField
              id="server-password"
              label="Password"
              value={value("password")}
              hint="Setting a password clears the private key."
              oninput={(value) => {
                updateString("password", value);
                if (value) update("privateKey", null);
              }}
            />
            <label class="field grow">
              <span>Private key path</span>
              <input
                value={value("privateKey")}
                oninput={(event) => {
                  update("privateKey", event.currentTarget.value || null);
                  if (event.currentTarget.value) updateString("password", "");
                }}
              />
            </label>
          </div>
          <div class="checks">
            <Toggle id="ask-password" label="Ask for password when connecting" checked={bool("askPasswordWhenConnect")} onchange={(checked) => updateBool("askPasswordWhenConnect", checked)} />
            <Toggle id="ping-first" label="Check reachability before connecting" checked={bool("isPingBeforeConnect")} onchange={(checked) => updateBool("isPingBeforeConnect", checked)} />
            <Toggle id="auto-alternate" label="Auto-switch to reachable alternate address" checked={bool("isAutoAlternateAddressSwitching")} onchange={(checked) => updateBool("isAutoAlternateAddressSwitching", checked)} />
          </div>

          <section class="sub">
            <div class="sub-head">
              <h3>Alternate credentials</h3>
              <Button size="sm" variant="ghost" onclick={addAlternate}>Add</Button>
            </div>
            {#each draft.alternateCredentials ?? [] as alternate, index (index)}
              <div class="alternate">
                <input placeholder="Name" value={alternate.name} oninput={(event) => updateAlternate(index, "name", event.currentTarget.value)} />
                <input placeholder="Address" value={alternate.address ?? ""} oninput={(event) => updateAlternate(index, "address", event.currentTarget.value || null)} />
                <input placeholder="User" value={alternate.username} oninput={(event) => updateAlternate(index, "username", event.currentTarget.value)} />
                <input type="password" placeholder="Password" value={alternate.password} oninput={(event) => updateAlternate(index, "password", event.currentTarget.value)} />
                <button type="button" class="remove" onclick={() => removeAlternate(index)} aria-label="Remove alternate credential">×</button>
              </div>
            {/each}
          </section>
        {/if}
      {/if}

      {#if section === "display"}
        {#if draft.protocol === "rdp"}
          <div class="row">
            <label class="field"><span>Domain</span><input value={value("domain")} oninput={(event) => updateString("domain", event.currentTarget.value)} /></label>
            <label class="field"><span>Resize mode</span>
              <select value={draft.rdpWindowResizeMode ?? "autoResize"} onchange={(event) => updateString("rdpWindowResizeMode", event.currentTarget.value)}>
                {#each ["autoResize", "stretch", "fixed", "stretchFullScreen", "fixedFullScreen"] as mode (mode)}<option value={mode}>{mode}</option>{/each}
              </select>
            </label>
            <label class="field small"><span>Width</span><input type="number" value={value("rdpWidth")} oninput={(event) => update("rdpWidth", Number(event.currentTarget.value))} /></label>
            <label class="field small"><span>Height</span><input type="number" value={value("rdpHeight")} oninput={(event) => update("rdpHeight", Number(event.currentTarget.value))} /></label>
          </div>
          <div class="row">
            <label class="field"><span>Display performance</span>
              <select value={draft.displayPerformance ?? "auto"} onchange={(event) => updateString("displayPerformance", event.currentTarget.value)}>
                {#each ["auto", "low", "middle", "high"] as mode (mode)}<option value={mode}>{mode}</option>{/each}
              </select>
            </label>
            <label class="field"><span>Full screen</span>
              <select value={draft.rdpFullScreenFlag ?? "disable"} onchange={(event) => updateString("rdpFullScreenFlag", event.currentTarget.value)}>
                {#each ["disable", "enableFullScreen", "enableFullAllScreens"] as mode (mode)}<option value={mode}>{mode}</option>{/each}
              </select>
            </label>
          </div>
          <div class="row">
            <label class="field"><span>Audio</span>
              <select value={draft.audioRedirectionMode ?? "redirectToLocal"} onchange={(event) => updateString("audioRedirectionMode", event.currentTarget.value)}>
                <option value="redirectToLocal">Play on this computer (default)</option>
                <option value="leaveOnRemote">Leave on the remote computer</option>
                <option value="disabled">Do not play</option>
              </select>
            </label>
            <label class="field"><span>Audio quality</span>
              <select value={draft.audioQualityMode ?? "dynamic"} onchange={(event) => updateString("audioQualityMode", event.currentTarget.value)}>
                {#each ["dynamic", "medium", "high"] as mode (mode)}<option value={mode}>{mode}</option>{/each}
              </select>
            </label>
          </div>
          <p class="hr-small hr-muted">
            “Play on this computer” routes remote audio to your default Windows output device. It is
            the default for new servers; existing servers keep their saved value.
          </p>
          <div class="checks">
            <Toggle id="fullscreen" label="Connect in full screen" checked={bool("isConnWithFullScreen")} onchange={(checked) => updateBool("isConnWithFullScreen", checked)} />
            <Toggle id="multimon" label="Use all monitors" checked={bool("useMultimon")} onchange={(checked) => updateBool("useMultimon", checked)} />
            <Toggle id="scale-system" label="Follow system DPI scale" checked={bool("isScaleFactorFollowSystem")} onchange={(checked) => updateBool("isScaleFactorFollowSystem", checked)} />
            <Toggle id="clipboard" label="Clipboard redirection" checked={bool("enableClipboard")} onchange={(checked) => updateBool("enableClipboard", checked)} />
            <Toggle id="drives" label="Disk drive redirection" checked={bool("enableDiskDrives")} onchange={(checked) => updateBool("enableDiskDrives", checked)} />
            <Toggle id="printers" label="Printer redirection" checked={bool("enablePrinters")} onchange={(checked) => updateBool("enablePrinters", checked)} />
            <Toggle id="ports" label="COM port redirection" checked={bool("enablePorts")} onchange={(checked) => updateBool("enablePorts", checked)} />
            <Toggle id="smartcards" label="Smart cards / Windows Hello" checked={bool("enableSmartCardsAndWinHello")} onchange={(checked) => updateBool("enableSmartCardsAndWinHello", checked)} />
          </div>
        {:else if draft.protocol === "vnc"}
          <label class="field"><span>Resize mode</span>
            <select value={draft.vncWindowResizeMode ?? "stretch"} onchange={(event) => updateString("vncWindowResizeMode", event.currentTarget.value)}>
              <option value="stretch">Stretch</option><option value="fixed">Fixed</option>
            </select>
          </label>
        {:else if draft.protocol === "remoteApp"}
          <label class="field"><span>Published app name</span><input value={value("remoteApplicationName")} oninput={(event) => updateString("remoteApplicationName", event.currentTarget.value)} /></label>
          <label class="field"><span>Program</span><input value={value("remoteApplicationProgram")} oninput={(event) => updateString("remoteApplicationProgram", event.currentTarget.value)} /></label>
        {:else if draft.protocol === "serial"}
          <div class="row">
            <label class="field small"><span>Bit rate</span><input type="number" value={value("bitRate")} oninput={(event) => update("bitRate", Number(event.currentTarget.value))} /></label>
            <label class="field small"><span>Data bits</span><input type="number" value={value("dataBits")} oninput={(event) => update("dataBits", Number(event.currentTarget.value))} /></label>
            <label class="field small"><span>Stop bits</span><input type="number" value={value("stopBits")} oninput={(event) => update("stopBits", Number(event.currentTarget.value))} /></label>
            <label class="field"><span>Parity</span><select value={draft.parity ?? "none"} onchange={(event) => updateString("parity", event.currentTarget.value)}>{#each ["none", "odd", "even", "mark", "space"] as item (item)}<option value={item}>{item}</option>{/each}</select></label>
          </div>
        {:else if draft.protocol === "localApp"}
          <label class="field"><span>Executable</span><input value={value("exePath")} oninput={(event) => updateString("exePath", event.currentTarget.value)} /></label>
          <Toggle id="hosting" label="Run embedded in a session tab" checked={bool("runWithHosting")} onchange={(checked) => updateBool("runWithHosting", checked)} />
          <section class="sub">
            <div class="sub-head"><h3>Arguments</h3><Button size="sm" variant="ghost" onclick={addArgument}>Add</Button></div>
            {#each draft.argumentList ?? [] as argument, index (index)}
              <div class="argument">
                <select value={argument.type} onchange={(event) => updateArgument(index, "type", event.currentTarget.value)}>
                  {#each ["normal", "int", "float", "file", "secret", "flag", "selection", "const"] as kind (kind)}<option value={kind}>{kind}</option>{/each}
                </select>
                <input placeholder="Name" value={argument.name} oninput={(event) => updateArgument(index, "name", event.currentTarget.value)} />
                <input placeholder="Key" value={argument.key} oninput={(event) => updateArgument(index, "key", event.currentTarget.value)} />
                <input placeholder="Value" value={argument.value} oninput={(event) => updateArgument(index, "value", event.currentTarget.value)} />
                <button type="button" onclick={() => moveArgument(index, -1)} aria-label="Move up">↑</button>
                <button type="button" onclick={() => moveArgument(index, 1)} aria-label="Move down">↓</button>
                <button type="button" class="remove" onclick={() => removeArgument(index)} aria-label="Remove">×</button>
              </div>
            {/each}
          </section>
        {:else if draft.protocol === "sftp" || draft.protocol === "ftp"}
          <label class="field"><span>Startup path</span><input value={value("startupPath")} oninput={(event) => updateString("startupPath", event.currentTarget.value)} /></label>
        {:else if draft.protocol === "ssh" || draft.protocol === "telnet"}
          <label class="field"><span>SSH version</span><input value={value("sshVersion")} oninput={(event) => updateString("sshVersion", event.currentTarget.value)} /></label>
          <label class="field"><span>Startup command</span><input value={value("startupAutoCommand")} oninput={(event) => updateString("startupAutoCommand", event.currentTarget.value)} /></label>
          <label class="field"><span>Session config path</span><input value={value("externalSessionConfigPath")} oninput={(event) => update("externalSessionConfigPath", event.currentTarget.value || null)} /></label>
        {:else if draft.protocol === "custom"}
          <label class="field">
            <span>Custom protocol name</span>
            <input
              value={value("customProtocolName")}
              placeholder="MYPROTO"
              oninput={(event) => updateString("customProtocolName", event.currentTarget.value)}
            />
          </label>
          <label class="field">
            <span>Runner fields (JSON object)</span>
            <textarea
              rows="4"
              spellcheck="false"
              value={extraFieldsText}
              oninput={(event) => updateExtraFields(event.currentTarget.value)}
            ></textarea>
          </label>
          {#if extraFieldsError}
            <p class="field-error">Invalid JSON: {extraFieldsError}</p>
          {/if}
          <p class="hr-small hr-muted">
            String fields become {`{field}`} macros and CUSTOM_FIELD environment variables for the
            selected runner. Built-in protocol names cannot be reused.
          </p>
        {/if}
      {/if}

      {#if section === "advanced"}
        {#if draft.protocol === "rdp"}
          <div class="row">
            <label class="field"><span>Gateway mode</span>
              <select value={draft.gatewayMode ?? "doNotUseGateway"} onchange={(event) => updateString("gatewayMode", event.currentTarget.value)}>
                {#each ["doNotUseGateway", "useTheseGatewayServerSettings", "automaticallyDetectGatewayServerSettings"] as mode (mode)}<option value={mode}>{mode}</option>{/each}
              </select>
            </label>
            <label class="field"><span>Gateway host</span><input value={value("gatewayHostName")} oninput={(event) => updateString("gatewayHostName", event.currentTarget.value)} /></label>
            <label class="field"><span>Gateway user</span><input value={value("gatewayUserName")} oninput={(event) => updateString("gatewayUserName", event.currentTarget.value)} /></label>
            <label class="field"><span>Gateway password</span><input type="password" value={value("gatewayPassword")} oninput={(event) => updateString("gatewayPassword", event.currentTarget.value)} /></label>
          </div>
          <div class="checks">
            <Toggle id="admin" label="Administrative / console session" checked={bool("isAdministrativePurposes")} onchange={(checked) => updateBool("isAdministrativePurposes", checked)} />
            <Toggle id="mstsc" label="Launch with external mstsc" checked={bool("mstscModeEnabled")} onchange={(checked) => updateBool("mstscModeEnabled", checked)} />
          </div>
          <label class="field"><span>Additional .rdp settings</span><textarea rows="4" value={value("rdpFileAdditionalSettings")} oninput={(event) => updateString("rdpFileAdditionalSettings", event.currentTarget.value)}></textarea></label>
        {/if}
        <label class="field">
          <span>Runner</span>
          <select
            value={draft.selectedRunnerName ?? ""}
            onchange={(event) => update("selectedRunnerName", event.currentTarget.value || null)}
          >
            <option value="">Follow protocol default</option>
            {#each runnerOptions as runner (runner.id)}
              <option value={runner.name}>{runner.name}{runner.exePath ? "" : " (not installed)"}</option>
            {/each}
          </select>
        </label>
        <Toggle id="new-window" label="Always open in a new tab window" checked={bool("alwaysOpenInNewTabWindow")} onchange={(checked) => updateBool("alwaysOpenInNewTabWindow", checked)} />
        {#if protocol?.helpUrl}
          <a class="help" href={protocol.helpUrl} target="_blank" rel="noreferrer">Protocol help</a>
        {/if}
      {/if}

      {#if section === "scripts"}
        <label class="field"><span>Script before connect</span><textarea rows="4" value={value("commandBeforeConnected")} oninput={(event) => updateString("commandBeforeConnected", event.currentTarget.value)}></textarea></label>
        <div class="row">
          <Toggle id="hide-script" label="Hide the script window" checked={bool("hideCommandBeforeConnectedWindow")} onchange={(checked) => updateBool("hideCommandBeforeConnectedWindow", checked)} />
          <Button size="sm" variant="secondary" onclick={() => void testScript("before")}>Test</Button>
        </div>
        <label class="field"><span>Script after disconnect</span><textarea rows="4" value={value("commandAfterDisconnected")} oninput={(event) => updateString("commandAfterDisconnected", event.currentTarget.value)}></textarea></label>
        <Button size="sm" variant="secondary" onclick={() => void testScript("after")}>Test</Button>
      {/if}
    </div>

    <footer>
      <Button variant="ghost" onclick={() => catalog.closeEditor()}>{i18n.t("common.cancel")}</Button>
      <Button variant="primary" disabled={readOnly} onclick={() => void save()}>{i18n.t("common.save")}</Button>
    </footer>

    {#if stockOpen}
      <div class="library stock-library" role="dialog" aria-label="Icons">
        <header>
          <h3>Icons</h3>
          <button type="button" aria-label="Close icons" onclick={() => (stockOpen = false)}>×</button>
        </header>
        <div class="library-grid">
          {#each STOCK_ICONS as icon (icon.id)}
            <button
              type="button"
              title={icon.name}
              aria-label={icon.name}
              onclick={() => void applyStockIcon(icon)}
            >
              <img src={stockIconDataUrl(icon)} alt="" />
              <span>{icon.name}</span>
            </button>
          {/each}
        </div>
      </div>
    {/if}

    {#if libraryOpen}
      <div class="library" role="dialog" aria-label="Icon library">
        <header><h3>Icon library</h3><button type="button" onclick={() => (libraryOpen = false)}>×</button></header>
        <div class="library-grid">
          {#each iconLibrary as icon (icon.path)}
            <button type="button" title={icon.name} onclick={() => { updateString("iconBase64", icon.base64); libraryOpen = false; }}>
              <img src="data:image/png;base64,{icon.base64}" alt={icon.name} />
            </button>
          {/each}
          {#if iconLibrary.length === 0}<p>Library is empty.</p>{/if}
        </div>
      </div>
    {/if}
  </aside>
{/if}

<style>
  .editor {
    width: min(620px, 46vw);
    min-width: 380px;
    max-width: 100%;
    flex: none;
    display: flex;
    flex-direction: column;
    border-left: 1px solid var(--hr-border);
    background: color-mix(in srgb, var(--hr-primary-mid) 70%, var(--hr-background));
    min-height: 0;
    overflow: hidden;
    position: relative;
  }

  @media (max-width: 980px) {
    .editor {
      flex: 1;
      width: auto;
      min-width: 0;
    }
  }

  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 10px;
    padding: 14px 16px 10px;
    border-bottom: 1px solid var(--hr-border);
  }

  header h2 {
    font-size: var(--hr-font-size-subtitle);
  }

  header p {
    font-size: var(--hr-font-size-small);
    color: color-mix(in srgb, var(--hr-background-text) 55%, transparent);
    margin-top: 3px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .close {
    border: 0;
    background: transparent;
    color: inherit;
    padding: 4px;
  }

  .bulk-hint {
    margin: 8px 16px 0;
    padding: 7px 9px;
    border-radius: var(--hr-radius-small);
    background: color-mix(in srgb, var(--hr-warning) 12%, transparent);
    color: var(--hr-warning);
    font-size: var(--hr-font-size-small);
  }

  .readonly-hint {
    margin: 8px 16px 0;
    padding: 7px 9px;
    border-radius: var(--hr-radius-small);
    border: 1px solid color-mix(in srgb, var(--hr-warning) 35%, transparent);
    background: color-mix(in srgb, var(--hr-warning) 10%, transparent);
    font-size: var(--hr-font-size-small);
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .readonly-hint strong {
    color: var(--hr-warning);
  }

  .field-error {
    font-size: 10px;
    color: var(--hr-danger);
    margin: 0;
  }

  .tabs {
    display: flex;
    gap: 2px;
    padding: 8px 12px 0;
    border-bottom: 1px solid var(--hr-border);
    overflow-x: auto;
  }

  .tabs button {
    border: 0;
    background: transparent;
    color: color-mix(in srgb, var(--hr-background-text) 60%, transparent);
    padding: 6px 9px;
    font-size: var(--hr-font-size-small);
    border-bottom: 2px solid transparent;
    white-space: nowrap;
  }

  .tabs button.active {
    color: var(--hr-accent-light);
    border-bottom-color: var(--hr-accent-mid);
  }

  .body {
    flex: 1;
    overflow: auto;
    padding: 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .row {
    display: flex;
    gap: 9px;
    align-items: flex-end;
    flex-wrap: wrap;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
    flex: 1;
    font-size: var(--hr-font-size-small);
  }

  .field.grow {
    flex: 2;
  }

  .field.small {
    max-width: 110px;
  }

  .field.color {
    flex: none;
    flex-direction: row;
    align-items: center;
    gap: 7px;
  }

  .field.color input {
    width: 38px;
    height: 28px;
    padding: 0;
    border: 1px solid var(--hr-border);
    border-radius: 5px;
    background: transparent;
  }

  .field > span {
    color: color-mix(in srgb, var(--hr-background-text) 68%, transparent);
  }

  input,
  select,
  textarea {
    border-radius: var(--hr-radius);
    border: 1px solid var(--hr-border);
    background: color-mix(in srgb, var(--hr-primary-dark) 50%, var(--hr-background));
    color: inherit;
    padding: 7px 9px;
    font: inherit;
    font-size: var(--hr-font-size-small);
    min-height: 32px;
    min-width: 0;
  }

  textarea {
    resize: vertical;
  }

  .checks {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .sub {
    border: 1px solid var(--hr-border);
    border-radius: var(--hr-radius);
    padding: 9px;
    margin-top: 5px;
  }

  .sub-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 6px;
  }

  .sub-head h3 {
    font-size: var(--hr-font-size-small);
  }

  .icon-row {
    display: flex;
    gap: 12px;
    align-items: center;
  }

  .icon-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .file-button {
    display: inline-flex;
    align-items: center;
    min-height: 28px;
    padding: 0 10px;
    border-radius: var(--hr-radius);
    border: 1px solid var(--hr-border);
    font-size: var(--hr-font-size-small);
    cursor: pointer;
  }

  .file-button input {
    display: none;
  }

  .suggestions {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }

  .suggestions button,
  .inline-link {
    border: 0;
    background: color-mix(in srgb, var(--hr-accent-mid) 12%, transparent);
    color: var(--hr-accent-light);
    border-radius: 999px;
    padding: 2px 8px;
    font-size: 9px;
  }

  .inline-link {
    background: transparent;
    padding: 0 4px;
  }

  .alternate,
  .argument {
    display: flex;
    gap: 5px;
    align-items: center;
    margin-bottom: 5px;
  }

  .alternate input,
  .argument input {
    flex: 1;
    min-width: 0;
  }

  .argument select {
    width: 90px;
    flex: none;
  }

  .alternate button,
  .argument button {
    border: 0;
    background: transparent;
    color: inherit;
    padding: 2px 4px;
  }

  .remove {
    color: var(--hr-danger) !important;
  }

  .preview {
    border: 1px solid var(--hr-border);
    border-radius: var(--hr-radius);
    padding: 9px;
    min-height: 90px;
  }

  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 10px 16px;
    border-top: 1px solid var(--hr-border);
  }

  .help {
    font-size: var(--hr-font-size-small);
    color: var(--hr-accent-light);
  }

  .library {
    position: absolute;
    inset: 20% 10% auto;
    max-height: 55%;
    overflow: auto;
    border-radius: var(--hr-radius-large);
    border: 1px solid var(--hr-border-strong);
    background: color-mix(in srgb, var(--hr-primary-mid) 97%, var(--hr-background));
    box-shadow: var(--hr-shadow);
    padding: 12px;
    z-index: 20;
  }

  .library header {
    padding: 0 0 8px;
    border: 0;
  }

  .library-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(56px, 1fr));
    gap: 7px;
  }

  .library-grid button {
    border: 1px solid var(--hr-border);
    background: transparent;
    border-radius: 8px;
    padding: 3px;
  }

  .library-grid img {
    width: 100%;
    aspect-ratio: 1;
    object-fit: contain;
  }

  .stock-library {
    inset: 14% 8% auto;
    max-height: 62%;
  }

  .stock-library .library-grid {
    grid-template-columns: repeat(auto-fill, minmax(74px, 1fr));
  }

  .stock-library .library-grid button {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 5px;
  }

  .stock-library .library-grid span {
    font-size: 9px;
    line-height: 1.2;
    text-align: center;
    color: color-mix(in srgb, var(--hr-background-text) 72%, transparent);
  }
</style>
