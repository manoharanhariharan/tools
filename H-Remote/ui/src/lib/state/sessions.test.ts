import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { SessionEvent } from "../ipc/contracts";
import { mockEmit, mockInvoke, resetMockState } from "../ipc/mock";
import { commands } from "../ipc/client";
import { app } from "./app.svelte";
import { sessions } from "./sessions.svelte";

function rdpServer(id = "mock-rdp-1", name = "Production RDP") {
  return {
    id,
    protocol: "rdp" as const,
    displayName: name,
    classVersion: 1,
    subTitle: "",
    iconBase64: null,
    colorHex: "",
    tags: [],
    treeNodes: [],
    note: "",
    commandBeforeConnected: "",
    hideCommandBeforeConnectedWindow: false,
    commandAfterDisconnected: "",
    selectedRunnerName: null,
    alwaysOpenInNewTabWindow: false,
    sessionId: null,
    sourceId: "local",
  };
}

async function connectRdp(id = "mock-rdp-1", name = "Production RDP") {
  await sessions.connect(rdpServer(id, name));
  return sessions.sessions[sessions.sessions.length - 1];
}

describe("session store", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    resetMockState();
    app.resetForTests();
    sessions.resetForTests();
  });

  afterEach(() => {
    sessions.resetForTests();
    vi.useRealTimers();
  });

  it("connects, receives state events and polls a frame", async () => {
    const snapshot = await connectRdp();
    expect(snapshot.state).toBe("connecting");
    expect(sessions.activeId).toBe(snapshot.id);

    await vi.advanceTimersByTimeAsync(250);
    expect(sessions.active?.state).toBe("connected");
    expect(sessions.active?.frameCount).toBeGreaterThan(0);

    await vi.advanceTimersByTimeAsync(150);
    const frame = sessions.activeFrame;
    expect(frame).toContain("data:image/svg+xml;base64,");
    const encoded = frame!.split(",")[1];
    expect(atob(encoded)).toContain("Production RDP");
  });

  it("keeps the last frame when a refresh fails", async () => {
    const snapshot = await connectRdp();
    await vi.advanceTimersByTimeAsync(250);
    const before = sessions.activeFrame;
    expect(before).toContain("data:image/svg+xml;base64,");
    const spy = vi
      .spyOn(commands, "sessionFrameImage")
      .mockRejectedValueOnce(new Error("ring replaced"));
    await sessions.refreshFrame(snapshot.id);
    expect(sessions.activeFrame).toBe(before);
    spy.mockRestore();
  });

  it("tracks remote cursor shape events per session", async () => {
    const snapshot = await connectRdp();
    sessions.handleEvent({
      kind: "cursor",
      sessionId: snapshot.id,
      cursor: {
        kind: "image",
        dataUrl: "data:image/png;base64,AAAA",
        hotspotX: 3,
        hotspotY: 4,
      },
    });
    expect(sessions.cursorFor(snapshot.id)).toEqual({
      kind: "image",
      dataUrl: "data:image/png;base64,AAAA",
      hotspotX: 3,
      hotspotY: 4,
    });
    sessions.handleEvent({
      kind: "cursor",
      sessionId: snapshot.id,
      cursor: { kind: "hidden" },
    });
    expect(sessions.cursorFor(snapshot.id)).toEqual({ kind: "hidden" });
  });

  it("coalesces bursts of frame events into one in-flight fetch", async () => {
    const snapshot = await connectRdp();
    const refresh = vi.spyOn(sessions, "refreshFrame").mockResolvedValue();
    for (let seq = 0; seq < 10; seq += 1) {
      sessions.handleEvent({
        kind: "frame",
        sessionId: snapshot.id,
        seq,
        width: 1280,
        height: 720,
      });
    }
    await vi.advanceTimersByTimeAsync(0);
    expect(refresh.mock.calls.length).toBeLessThanOrEqual(2);
    refresh.mockRestore();
  });

  it("does not duplicate a tab when the connect event arrives first", async () => {
    const snapshot = await connectRdp();
    sessions.handleEvent({ kind: "state", session: snapshot });
    sessions.handleEvent({
      kind: "state",
      session: { ...snapshot, state: "connected" },
    });
    expect(sessions.sessions.map((session) => session.id)).toEqual([snapshot.id]);
  });

  it("merges state events for the same session", () => {    sessions.handleEvent({
      kind: "state",
      session: {
        id: "s1",
        serverId: "server",
        serverName: "One",
        backend: "synthetic",
        state: "connecting",
        reason: null,
        message: null,
        attempt: 1,
        maxAttempts: 5,
        width: 1280,
        height: 720,
        frameCount: 0,
        startedAt: 1,
        connectedAt: null,
        endedAt: null,
        frameRing: null,
      },
    } satisfies SessionEvent);
    sessions.handleEvent({
      kind: "state",
      session: { ...sessions.sessions[0], state: "connected", connectedAt: 2 },
    } satisfies SessionEvent);
    expect(sessions.sessions).toHaveLength(1);
    expect(sessions.sessions[0].state).toBe("connected");
  });

  it("cancels, reconnects and closes a session", async () => {
    const snapshot = await connectRdp();
    await vi.advanceTimersByTimeAsync(250);

    await sessions.cancel(snapshot.id);
    expect(sessions.active?.state).toBe("disconnected");
    expect(sessions.active?.reason).toBe("localCancel");
    expect(vi.getTimerCount()).toBe(0);

    await sessions.reconnect(snapshot.id);
    expect(sessions.active?.state).toBe("reconnecting");
    await vi.advanceTimersByTimeAsync(250);
    expect(sessions.active?.state).toBe("connected");

    await sessions.close(snapshot.id);
    expect(sessions.sessions).toHaveLength(0);
    expect(sessions.activeId).toBeNull();
  });

  it("releases the polling interval on dispose", async () => {
    await connectRdp();
    await vi.advanceTimersByTimeAsync(250);
    expect(vi.getTimerCount()).toBeGreaterThan(0);
    sessions.dispose();
    expect(vi.getTimerCount()).toBe(0);
  });

  it("ignores session commands without an active session", async () => {
    await expect(sessions.cancel()).resolves.toBeUndefined();
    await expect(sessions.reconnect()).resolves.toBeUndefined();
    await expect(sessions.close()).resolves.toBeUndefined();
    await expect(sessions.sendCtrlAltDel()).resolves.toBeUndefined();
    await expect(sessions.resize("missing", 800, 600)).resolves.toBeUndefined();
  });

  it("auto reconnects after a network loss", async () => {
    const snapshot = await connectRdp();
    const reconnect = vi.spyOn(sessions, "reconnect").mockResolvedValue();
    sessions.handleEvent({
      kind: "state",
      session: {
        ...snapshot,
        state: "disconnected",
        reason: "networkLoss",
        message: "The network connection was lost",
      },
    });
    await vi.advanceTimersByTimeAsync(1600);
    expect(reconnect).toHaveBeenCalledWith(snapshot.id);
    reconnect.mockRestore();
  });

  it("focuses an existing connection instead of creating a duplicate tab", async () => {
    const first = await connectRdp();
    const connectSpy = vi.spyOn(commands, "connectServer");
    await sessions.connect(rdpServer());
    expect(connectSpy).not.toHaveBeenCalled();
    expect(sessions.sessions).toHaveLength(1);
    expect(sessions.activeId).toBe(first.id);
    expect(sessions.focusedId).toBe(first.id);
    connectSpy.mockRestore();
  });

  it("writes a diagnostic report when the network drops", async () => {
    const snapshot = await connectRdp();
    const report = vi
      .spyOn(commands, "reportSessionFailure")
      .mockResolvedValue({ path: "C:\\logs\\crash-session.md", markdown: "" });
    sessions.handleEvent({
      kind: "state",
      session: {
        ...snapshot,
        state: "disconnected",
        reason: "networkLoss",
        message: "The network connection was lost",
      },
    });
    await vi.advanceTimersByTimeAsync(0);
    expect(report).toHaveBeenCalledWith(snapshot.id);
    report.mockRestore();
  });

  it("writes one diagnostic report per failed session", async () => {
    const snapshot = await connectRdp();
    const report = vi
      .spyOn(commands, "reportSessionFailure")
      .mockResolvedValue({ path: "C:\\logs\\crash-session.md", markdown: "" });
    const failed = {
      ...snapshot,
      state: "failed" as const,
      reason: "connectionFailed" as const,
      message: "RDP connection failed",
    };
    sessions.handleEvent({ kind: "state", session: failed });
    sessions.handleEvent({ kind: "state", session: failed });
    await vi.advanceTimersByTimeAsync(0);
    expect(report).toHaveBeenCalledTimes(1);
    expect(report).toHaveBeenCalledWith(snapshot.id);
    report.mockRestore();
  });

  it("keeps multiple tabs, cycles, selects by index and reorders", async () => {
    const first = await connectRdp();
    const second = await connectRdp("mock-ssh-1", "Build Server");
    expect(sessions.tabs).toHaveLength(2);
    expect(sessions.activeId).toBe(second.id);

    sessions.cycle(-1);
    expect(sessions.activeId).toBe(first.id);
    sessions.selectIndex(1);
    expect(sessions.activeId).toBe(second.id);

    sessions.reorder(second.id, 0);
    expect(sessions.tabs.map((session) => session.id)).toEqual([
      second.id,
      first.id,
    ]);
  });

  it("shows every tab in grid mode and persists the layout", async () => {
    const first = await connectRdp();
    const second = await connectRdp("mock-vnc-1", "Lab VNC");
    await vi.advanceTimersByTimeAsync(250);

    await sessions.setGridMode(true);
    expect(sessions.visibleIds).toEqual([first.id, second.id]);
    expect(sessions.sessions.map((session) => session.id)).toEqual([
      first.id,
      second.id,
    ]);
    expect(sessions.gridMode).toBe(true);

    await sessions.setLayout("2-rows");
    await sessions.setTabButtons({ showClose: false, showReconnect: false });
    const locality = await mockInvoke<import("../ipc/contracts").Locality>(
      "get_locality",
    );
    expect(locality.ui.gridMode).toBe(true);
    expect(locality.ui.tabLayout).toBe("2-rows");
    expect(locality.ui.showTabClose).toBe(false);
    expect(locality.ui.showTabReconnect).toBe(false);
  });

  it("keeps connections in the shared workspace in the browser mock", async () => {
    const snapshot = await connectRdp();
    await sessions.hostSession(snapshot.id);
    expect(sessions.activeId).toBe(snapshot.id);
    expect(sessions.visibleIds).toEqual([snapshot.id]);
  });

  it("closes other tabs but keeps the requested one", async () => {
    const first = await connectRdp();
    const second = await connectRdp("mock-vnc-1", "Lab VNC");
    await sessions.closeOthers(first.id);
    expect(sessions.tabs.map((session) => session.id)).toEqual([first.id]);
    expect(sessions.activeId).toBe(first.id);
    expect(second.id).not.toBe(first.id);
  });

  it("loads existing sessions from the backend", async () => {    await mockInvoke("connect_server", { id: "mock-rdp-1" });
    await sessions.load();
    expect(sessions.sessions).toHaveLength(1);
  });

  it("forwards clipboard events when the browser permits it", () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      value: { writeText },
      configurable: true,
    });
    mockEmit("session:event", {
      kind: "clipboard",
      sessionId: "s1",
      text: "hello",
    } satisfies SessionEvent);
    sessions.handleEvent({ kind: "clipboard", sessionId: "s1", text: "hello" });
    expect(writeText).toHaveBeenCalled();
  });
});
