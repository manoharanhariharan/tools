import { fireEvent, render, screen } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import SessionPane from "./SessionPane.svelte";
import { resetMockState } from "../../ipc/mock";
import { app } from "../../state/app.svelte";
import { sessions } from "../../state/sessions.svelte";

async function connectRdp() {
  await sessions.connect({
    id: "mock-rdp-1",
    protocol: "rdp",
    displayName: "Production RDP",
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
  });
  return sessions.sessions[sessions.sessions.length - 1];
}

describe("RDP session input", () => {
  beforeEach(() => {
    resetMockState();
    app.resetForTests();
    sessions.resetForTests();
  });

  it("forwards pointer buttons, movement, wheel and keys to the backend", async () => {
    const session = await connectRdp();
    sessions.handleEvent({
      kind: "state",
      session: { ...session, state: "connected" },
    });
    const connected = sessions.sessions.find((candidate) => candidate.id === session.id)!;
    const button = vi.spyOn(sessions, "rdpPointerButton");
    const key = vi.spyOn(sessions, "rdpKey");
    const wheel = vi.spyOn(sessions, "rdpWheel");
    render(SessionPane, {
      session: connected,
      frame: null,
      onfocus: () => {},
      onclose: () => {},
    });

    const stage = screen.getByRole("application", { name: "RDP desktop" });
    vi.spyOn(stage, "getBoundingClientRect").mockReturnValue({
      x: 0,
      y: 0,
      left: 0,
      top: 0,
      right: session.width,
      bottom: session.height,
      width: session.width,
      height: session.height,
      toJSON: () => ({}),
    } as DOMRect);
    await fireEvent.pointerDown(stage, {
      button: 0,
      buttons: 1,
      clientX: 10,
      clientY: 10,
    });
    expect(button).toHaveBeenCalledWith(session.id, 10, 10, 0, true);

    await fireEvent.pointerUp(stage, { button: 0, clientX: 10, clientY: 10 });
    expect(button).toHaveBeenCalledWith(session.id, 10, 10, 0, false);

    await fireEvent.wheel(stage, { deltaY: -120, clientX: 10, clientY: 10 });
    expect(wheel).toHaveBeenCalledWith(session.id, 10, 10, true, 120);

    await fireEvent.keyDown(stage, { code: "KeyA", key: "a" });
    expect(key).toHaveBeenCalledWith(session.id, "KeyA", "a", true);
    await fireEvent.keyUp(stage, { code: "KeyA", key: "a" });
    expect(key).toHaveBeenCalledWith(session.id, "KeyA", "a", false);

    button.mockRestore();
    key.mockRestore();
    wheel.mockRestore();
  });

  it("applies the remote cursor bitmap to the stage", async () => {
    const session = await connectRdp();
    sessions.handleEvent({
      kind: "state",
      session: { ...session, state: "connected" },
    });
    const connected = sessions.sessions.find(
      (candidate) => candidate.id === session.id,
    )!;
    render(SessionPane, {
      session: connected,
      frame: "data:image/svg+xml;base64,PHN2Zy8+",
      onfocus: () => {},
      onclose: () => {},
    });
    sessions.handleEvent({
      kind: "cursor",
      sessionId: session.id,
      cursor: {
        kind: "image",
        dataUrl: "data:image/png;base64,AAAA",
        hotspotX: 2,
        hotspotY: 3,
      },
    });
    const stage = screen.getByRole("application", { name: "RDP desktop" });
    await vi.waitFor(() => {
      expect(stage.style.cursor).toContain("data:image/png;base64,AAAA");
      expect(stage.style.cursor).toContain("2 3");
    });
  });

  it("maps pointer input through the rendered canvas rect", async () => {
    const session = await connectRdp();
    sessions.handleEvent({
      kind: "state",
      session: { ...session, state: "connected", width: 200, height: 100 },
    });
    const connected = sessions.sessions.find(
      (candidate) => candidate.id === session.id,
    )!;
    const button = vi.spyOn(sessions, "rdpPointerButton");
    const { container } = render(SessionPane, {
      session: connected,
      frame: "data:image/svg+xml;base64,PHN2Zy8+",
      onfocus: () => {},
      onclose: () => {},
    });

    const canvas = container.querySelector("canvas");
    expect(canvas).not.toBeNull();
    // jsdom does not decode the SVG, so seed the bitmap size the effect would
    // have copied from the loaded frame.
    canvas!.width = 200;
    canvas!.height = 100;
    vi.spyOn(canvas!, "getBoundingClientRect").mockReturnValue({
      x: 100,
      y: 50,
      left: 100,
      top: 50,
      right: 500,
      bottom: 250,
      width: 400,
      height: 200,
      toJSON: () => ({}),
    } as DOMRect);

    const stage = screen.getByRole("application", { name: "RDP desktop" });
    await fireEvent.pointerDown(stage, {
      button: 0,
      buttons: 1,
      clientX: 200,
      clientY: 100,
    });
    // half of a 400x200 rendered frame maps to half of the 200x100 desktop
    expect(button).toHaveBeenCalledWith(session.id, 50, 25, 0, true);
    button.mockRestore();
  });

  it("applies a fixed resolution and virtual-monitor multiplier", async () => {
    const session = await connectRdp();
    const resize = vi.spyOn(sessions, "resize");
    sessions.recordFitSize(session.id, 1280, 720);

    await sessions.applyDisplay(session.id, 1920, 1080, 2);
    expect(resize).toHaveBeenCalledWith(session.id, 3840, 1080);
    expect(sessions.displayOverrides[session.id]).toEqual({
      width: 1920,
      height: 1080,
      monitors: 2,
    });

    const callsBeforeFit = resize.mock.calls.length;
    await sessions.resetDisplay(session.id);
    expect(resize.mock.calls.length).toBe(callsBeforeFit);
    expect(sessions.displayOverrides[session.id]).toBeUndefined();
    expect(sessions.shouldAutoResize(session.id)).toBe(false);
    resize.mockRestore();
  });

  it("clamps virtual monitors to the RDP 8192 px desktop limit", async () => {
    const session = await connectRdp();
    const resize = vi.spyOn(sessions, "resize");
    await sessions.applyDisplay(session.id, 3840, 2160, 4);
    expect(resize).toHaveBeenCalledWith(session.id, 7680, 2160);
    expect(sessions.displayOverrides[session.id]?.monitors).toBe(2);
    resize.mockRestore();
  });
});
