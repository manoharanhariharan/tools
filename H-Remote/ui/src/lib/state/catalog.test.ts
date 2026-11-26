import { beforeEach, describe, expect, it, vi } from "vitest";
import { mockInvoke, resetMockState } from "../ipc/mock";
import { commands } from "../ipc/client";
import type { CatalogFolder, DataSourceConfig, Server } from "../ipc/contracts";
import { app } from "./app.svelte";
import { catalog, cloneJson } from "./catalog.svelte";

async function seed(): Promise<void> {
  catalog.dispose();
  catalog.editor = null;
  catalog.selected.clear();
  catalog.clearSelection();
  catalog.query = "";
  catalog.viewMode = "list";
  catalog.clearFolderFilter();
  await catalog.load();
}

function serverByName(name: string): Server {
  const server = catalog.snapshot?.servers.find(
    (candidate) => candidate.displayName === name,
  );
  if (!server) {
    throw new Error(`missing mock server ${name}`);
  }
  return server;
}

describe("catalog store", () => {
  beforeEach(async () => {
    resetMockState();
    app.resetForTests();
    await seed();
  });

  it("loads the catalog snapshot, tags and view", () => {
    expect(catalog.loading).toBe(false);
    expect(catalog.snapshot?.servers).toHaveLength(7);
    expect(catalog.snapshot?.folders.length).toBeGreaterThan(0);
    expect(catalog.tags().map((tag) => tag.name)).toContain("prod");
    expect(catalog.visibleServerIds().length).toBe(7);
    expect(catalog.viewMode).toBe("list");
  });

  it("confines the catalog to a selected folder subtree", () => {
    const folder = catalog.rootFolders()[0];
    const flatten = (node: CatalogFolder): Server[] => [
      ...node.servers,
      ...node.folders.flatMap(flatten),
    ];
    const expected = flatten(folder);
    catalog.setFolderFilter(folder.path);
    expect(catalog.visibleServerIds().sort()).toEqual(
      expected.map((server) => server.id).sort(),
    );
    expect(catalog.rootFolders()).toEqual(folder.folders);
    expect(catalog.rootServers()).toEqual(folder.servers);
    catalog.clearFolderFilter();
    expect(catalog.visibleServerIds()).toHaveLength(7);
  });

  it("keeps direct matching first and filters by tag operators", async () => {
    expect(catalog.providers[0]).toBe("direct");

    catalog.query = "#prod -#linux";
    await catalog.refresh();
    expect(catalog.visibleServers().map((server) => server.displayName)).toEqual([
      "Production RDP",
      "Staging RDP",
    ]);

    catalog.query = "Build";
    await catalog.refresh();
    expect(catalog.visibleServers().map((server) => server.displayName)).toEqual([
      "Build Server",
    ]);
  });

  it("supports tri-state selection across visible servers", () => {
    const [first] = catalog.visibleServerIds();
    catalog.toggleSelected(first);
    expect(catalog.someSelected()).toBe(true);
    expect(catalog.allSelected()).toBe(false);

    catalog.toggleSelectAll();
    expect(catalog.allSelected()).toBe(true);
    expect(catalog.selected.size).toBe(7);

    catalog.toggleSelectAll();
    expect(catalog.selected.size).toBe(0);
    expect(catalog.allSelected()).toBe(false);
  });

  it("drops selection entries that a filter hides", async () => {
    catalog.toggleSelectAll();
    catalog.query = "Build";
    await catalog.refresh();
    expect(catalog.selected.size).toBe(1);
    expect(catalog.selectedServers()[0].displayName).toBe("Build Server");
  });

  it("creates a server through the editor and reloads", async () => {
    await catalog.openAdd("rdp");
    expect(catalog.editor?.mode).toBe("add");
    catalog.updateDraft("displayName", "New RDP");
    catalog.updateDraft("address", "10.0.0.77");
    expect(await catalog.saveEditor()).toBe(true);
    expect(catalog.editor).toBeNull();
    expect(catalog.snapshot?.servers).toHaveLength(8);
    const created = catalog.snapshot?.servers.find(
      (server) => server.displayName === "New RDP",
    );
    expect(created?.address).toBe("10.0.0.77");
  });

  it("switches protocol on a new draft but keeps shared fields", async () => {
    await catalog.openAdd("rdp");
    catalog.updateDraft("displayName", "New SSH");
    catalog.updateDraft("treeNodes", ["Prod"]);
    await catalog.changeEditorProtocol("ssh");
    expect(catalog.editor?.draft.protocol).toBe("ssh");
    expect(catalog.editor?.draft.displayName).toBe("New SSH");
    expect(catalog.editor?.draft.treeNodes).toEqual(["Prod"]);
    expect(catalog.editor?.draft.port).toBe(22);
  });

  it("applies only dirty fields to a bulk edit", async () => {
    const first = serverByName("Production RDP");
    const second = serverByName("Build Server");
    catalog.openBulk([first, second]);
    expect(catalog.editor?.mode).toBe("bulk");
    catalog.updateDraft("tags", ["shared"]);
    expect(await catalog.saveEditor()).toBe(true);
    const reloaded = catalog.snapshot?.servers ?? [];
    for (const id of [first.id, second.id]) {
      expect(reloaded.find((server) => server.id === id)?.tags).toEqual(["shared"]);
    }
  });

  it("saves an inline address without opening the editor", async () => {
    const server = cloneJson(serverByName("Lab VNC"));
    server.address = "192.168.1.44";
    expect(await catalog.saveInline(server)).toBe(true);
    expect(serverByName("Lab VNC").address).toBe("192.168.1.44");
  });

  it("creates, moves and deletes folders with server rewrites", async () => {
    await catalog.createFolder(["Temp", "Nested"]);
    expect(
      catalog.snapshot?.folders.some(
        (folder) => folder.path.join("/") === "Temp/Nested",
      ),
    ).toBe(true);

    const server = serverByName("Production RDP");
    await catalog.moveServer(server, ["Temp", "Nested"]);
    expect(serverByName("Production RDP").treeNodes).toEqual(["Temp", "Nested"]);

    const folder = catalog.snapshot?.folders.find(
      (candidate) => candidate.path.join("/") === "Temp",
    )!;
    await catalog.renameFolder(folder, "Temporary");
    expect(serverByName("Production RDP").treeNodes).toEqual([
      "Temporary",
      "Nested",
    ]);

    const temporary = catalog.snapshot?.folders.find(
      (candidate) => candidate.path.join("/") === "Temporary",
    )!;
    await catalog.duplicateFolder(temporary);
    expect(
      catalog.snapshot?.folders.some(
        (candidate) => candidate.path.join("/") === "Temporary (copy)",
      ),
    ).toBe(true);

    await catalog.moveFolder(temporary, []);
    expect(
      catalog.snapshot?.folders.some(
        (candidate) => candidate.path.join("/") === "Temporary",
      ),
    ).toBe(true);
  });

  it("sets and clears an individual folder colour", async () => {
    const folder = catalog.snapshot?.folders.find(
      (candidate) => candidate.path.join("/") === "Lab",
    )!;
    await catalog.setFolderColor(folder, "#ffab00");
    expect(
      catalog.snapshot?.folders.find((candidate) => candidate.id === folder.id)
        ?.colorHex,
    ).toBe("#FFAB00");
    expect(
      catalog
        .allRootFolders()
        .find((candidate) => candidate.name === "Lab")?.colorHex,
    ).toBe("#FFAB00");

    await catalog.setFolderColor(folder, "");
    expect(
      catalog.snapshot?.folders.find((candidate) => candidate.id === folder.id)
        ?.colorHex,
    ).toBe("");
  });

  it("renames and deletes tags across every server", async () => {
    await catalog.renameTag("prod", "Production");
    expect(catalog.tags().map((tag) => tag.name)).toContain("production");
    for (const server of catalog.snapshot?.servers ?? []) {
      expect(server.tags).not.toContain("prod");
    }

    await catalog.deleteTag("production");
    expect(catalog.tags().map((tag) => tag.name)).not.toContain("production");
  });

  it("persists sort, view mode and column widths", async () => {
    await catalog.setSort("address");
    expect(catalog.listView?.sort).toBe("address");
    await catalog.setViewMode("card");
    expect(catalog.viewMode).toBe("card");
    await catalog.setColumnWidth("name", 320);
    expect(catalog.listView?.columnWidths.name).toBe(320);
  });

  it("persists a custom column order across reloads", async () => {
    await catalog.setColumnOrder(["ip", "protocol", "name", "target"]);
    expect(catalog.columnOrder).toEqual([
      "ip",
      "protocol",
      "name",
      "target",
      "note",
    ]);
    await catalog.setColumnWidth("ip", 222);
    await catalog.load();
    expect(catalog.columnOrder[0]).toBe("ip");
    expect(catalog.columnWidths.ip).toBe(222);
  });

  it("reorders folders within their siblings only", async () => {
    const folder = catalog.rootFolders()[1];
    await catalog.moveFolderOrder(folder.path, -1);
    expect(catalog.treeView?.nodeOrder[0]).toBe(folder.path.join("/"));

    // Servers only reorder inside their own folder, never across folders.
    const server = serverByName("Build Server");
    await catalog.setSort("insertion");
    await catalog.moveServerOrder(server.id, -1);
    expect(catalog.listView?.customOrder).toEqual([]);
    expect(catalog.sort).toBe("insertion");

    const source = cloneJson(serverByName("Production RDP"));
    await mockInvoke("save_server", {
      server: { ...source, id: "sibling-1", displayName: "Sibling RDP" },
    });
    await catalog.load();
    await catalog.setSort("custom");
    await catalog.moveServerOrder("sibling-1", -1);
    await catalog.moveServerOrder("sibling-1", -1);
    const order = catalog.listView?.customOrder ?? [];
    expect(order.indexOf("sibling-1")).toBeLessThan(order.indexOf(source.id));
  });

  it("exports JSON with a plaintext warning and imports it back", async () => {
    const content = await mockInvoke<string>("export_servers_json", { ids: [] });
    expect(JSON.parse(content).servers).toHaveLength(7);
    const outcome = await mockInvoke<{ imported: number }>("import_json_content", {
      content,
    });
    expect(outcome.imported).toBe(7);
    await catalog.load();
    expect(catalog.snapshot?.servers).toHaveLength(14);
  });

  it("cancels the debounced refresh on dispose", async () => {
    catalog.setQuery("prod");
    catalog.dispose();
    await new Promise((resolve) => setTimeout(resolve, 250));
    expect(catalog.query).toBe("prod");
  });

  it("coalesces overlapping refreshes into one follow-up run", async () => {
    const spy = vi.spyOn(commands, "catalogView");
    const first = catalog.refresh();
    const second = catalog.refresh();
    await Promise.all([first, second]);
    expect(spy).toHaveBeenCalledTimes(2);
    spy.mockRestore();
  });

  it("labels local servers and groups extra sources as read-only", async () => {
    const source = await mockInvoke<DataSourceConfig>("data_source_save", {
      config: {
        id: "",
        name: "Team catalog",
        kind: "sqlite",
        path: "C:\\shared\\hremote.db",
        host: "",
        port: 0,
        database: "",
        username: "",
        password: "",
        readOnly: true,
        enabled: true,
      },
      password: null,
    });
    await mockInvoke("data_source_refresh", { id: source.id });
    await catalog.load();

    expect(catalog.hasMultipleSources()).toBe(true);
    expect(catalog.isReadOnly(serverByName("Production RDP"))).toBe(false);
    const imported = catalog.snapshot?.servers.find((server) => server.sourceId === source.id);
    expect(imported).toBeDefined();
    expect(catalog.isReadOnly(imported!)).toBe(true);
    expect(catalog.sourceLabel(source.id)).toBe("Team catalog");

    const groups = catalog.groupServers(catalog.rootServers());
    expect(groups[0].id).toBe("local");
    expect(groups[0].readOnly).toBe(false);
    expect(groups.at(-1)?.label).toBe("Team catalog");
    expect(groups.at(-1)?.readOnly).toBe(true);

    await catalog.setSourceFilter(source.id);
    expect(catalog.visibleServers().every((server) => server.sourceId === source.id)).toBe(true);

    await catalog.setSourceFilter("all");
    expect(catalog.visibleServers().some((server) => server.sourceId === "local")).toBe(true);
  });
});
