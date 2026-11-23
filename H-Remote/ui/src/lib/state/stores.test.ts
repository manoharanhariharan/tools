import { describe, expect, it, vi } from "vitest";
import { CleanupRegistry } from "../ipc/events";
import { mockEmit, mockSubscribe } from "../ipc/mock";
import { busy } from "./busy.svelte";
import { dialogs } from "./dialogs.svelte";

describe("busy overlay store", () => {
  it("is reference counted", () => {
    busy.reset();
    expect(busy.active).toBe(false);
    busy.start("one");
    busy.start("two");
    expect(busy.depth).toBe(2);
    busy.stop();
    expect(busy.active).toBe(true);
    expect(busy.message).toBe("two");
    busy.stop();
    expect(busy.active).toBe(false);
    expect(busy.message).toBe("");
  });

  it("never goes negative", () => {
    busy.reset();
    busy.stop();
    busy.stop();
    expect(busy.depth).toBe(0);
  });

  it("tracks work and always releases", async () => {
    busy.reset();
    await expect(
      busy.track("work", async () => {
        expect(busy.active).toBe(true);
        throw new Error("boom");
      }),
    ).rejects.toThrow("boom");
    expect(busy.active).toBe(false);
  });
});

describe("dialog store", () => {
  it("resolves message buttons", async () => {
    const promise = dialogs.message({
      title: "Delete?",
      message: "Sure?",
      buttons: [
        { value: "cancel", label: "Cancel" },
        { value: "yes", label: "Yes", variant: "danger" },
      ],
    });
    expect(dialogs.current?.kind).toBe("message");
    dialogs.close("yes");
    await expect(promise).resolves.toBe("yes");
    expect(dialogs.current).toBeNull();
  });

  it("resolves confirm as a boolean", async () => {
    const promise = dialogs.confirm({ title: "Continue?", message: "Go on" });
    dialogs.close("ok");
    await expect(promise).resolves.toBe(true);
  });

  it("resolves a replaced dialog so callers cannot hang", async () => {
    const first = dialogs.input({ title: "A", label: "A" });
    const second = dialogs.input({ title: "B", label: "B" });
    await expect(first).resolves.toBeNull();
    dialogs.close("value");
    await expect(second).resolves.toBe("value");
  });
});

describe("cleanup registry", () => {
  it("runs each teardown once and rejects late additions", () => {
    const registry = new CleanupRegistry();
    const cleanup = vi.fn();
    registry.add(cleanup);
    expect(registry.size).toBe(1);
    registry.dispose();
    registry.dispose();
    expect(cleanup).toHaveBeenCalledTimes(1);

    const late = vi.fn();
    registry.add(late);
    expect(late).toHaveBeenCalledTimes(1);
  });

  it("detaches mock event listeners", () => {
    const registry = new CleanupRegistry();
    const handler = vi.fn();
    registry.add(mockSubscribe("evt", handler));
    mockEmit("evt", { ok: true });
    expect(handler).toHaveBeenCalledTimes(1);
    registry.dispose();
    mockEmit("evt", { ok: false });
    expect(handler).toHaveBeenCalledTimes(1);
  });
});
