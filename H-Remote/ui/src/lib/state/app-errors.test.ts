import { beforeEach, describe, expect, it } from "vitest";
import { resetMockState } from "../ipc/mock";
import { app } from "./app.svelte";

describe("error reporting", () => {
  beforeEach(() => {
    resetMockState();
    app.resetForTests();
  });

  it("dismisses and does not reopen the same error", async () => {
    await app.reportError(new Error("session input failed"));
    expect(app.errorReport?.message).toBe("session input failed");

    app.dismissError();
    expect(app.errorReport).toBeNull();

    await app.reportError(new Error("session input failed"));
    expect(app.errorReport).toBeNull();

    await app.reportError(new Error("a different failure"));
    expect(app.errorReport?.message).toBe("a different failure");
  });

  it("keeps one modal while the same error is already open", async () => {
    await app.reportError(new Error("boom"));
    const first = app.errorReport;
    await app.reportError(new Error("boom"));
    expect(app.errorReport).toBe(first);
  });
});
