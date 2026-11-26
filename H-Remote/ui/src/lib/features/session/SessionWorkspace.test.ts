import { fireEvent, render, waitFor } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import SessionWorkspace from "./SessionWorkspace.svelte";
import { resetMockState } from "../../ipc/mock";
import { app } from "../../state/app.svelte";
import { sessions } from "../../state/sessions.svelte";

let fullscreenElement: Element | null = null;

beforeEach(() => {
  resetMockState();
  app.resetForTests();
  sessions.resetForTests();
  fullscreenElement = null;
  Object.defineProperty(document, "fullscreenElement", {
    configurable: true,
    get: () => fullscreenElement,
  });
  Object.defineProperty(document.documentElement, "requestFullscreen", {
    configurable: true,
    value: async () => {
      fullscreenElement = document.documentElement;
      document.dispatchEvent(new Event("fullscreenchange"));
    },
  });
  Object.defineProperty(document, "exitFullscreen", {
    configurable: true,
    value: async () => {
      fullscreenElement = null;
      document.dispatchEvent(new Event("fullscreenchange"));
    },
  });
});

afterEach(() => {
  delete (document as unknown as Record<string, unknown>).fullscreenElement;
});

describe("session workspace full screen", () => {
  it("fills the rim and reveals the controls from the top edge", async () => {
    const { container } = render(SessionWorkspace);
    const workspace = container.querySelector(".workspace");
    expect(workspace).not.toBeNull();

    await fireEvent.keyDown(document, { key: "F11" });
    await waitFor(() => expect(workspace).toHaveClass("fullscreen"));
    expect(workspace).not.toHaveClass("controls-visible");

    const header = container.querySelector(".workspace-bar") as HTMLElement;
    expect(header.inert).toBe(true);
    expect(header).toHaveAttribute("aria-hidden", "true");
    const topReveal = container.querySelector(".top-reveal");
    expect(topReveal).not.toBeNull();

    await fireEvent.pointerEnter(topReveal!);
    await waitFor(() => expect(workspace).toHaveClass("controls-visible"));
    expect(header.inert).toBe(false);

    await fireEvent.pointerLeave(header!);
    await waitFor(() => expect(workspace).not.toHaveClass("controls-visible"), {
      timeout: 2000,
    });

    await fireEvent.keyDown(document, { key: "F11" });
    await waitFor(() => expect(workspace).not.toHaveClass("fullscreen"));
  });
});
