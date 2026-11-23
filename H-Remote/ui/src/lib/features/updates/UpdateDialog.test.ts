import { fireEvent, render, screen } from "@testing-library/svelte";
import { beforeEach, describe, expect, it } from "vitest";
import UpdateDialog from "./UpdateDialog.svelte";
import { resetMockState } from "../../ipc/mock";
import { app } from "../../state/app.svelte";

describe("update dialog", () => {
  beforeEach(() => {
    resetMockState();
    app.resetForTests();
  });

  it("shows an available breaking update and can be dismissed", async () => {
    app.updateStatus = {
      enabled: true,
      feedConfigured: true,
      current: "0.1.0",
      checkedAt: 10,
      available: true,
      version: "0.2.0",
      prerelease: false,
      breaking: true,
      notes: "Faster sessions",
      url: "https://example.invalid/app.exe",
      error: null,
    };
    app.updateDialogOpen = true;

    render(UpdateDialog);
    expect(screen.getByText("Breaking update available")).toBeInTheDocument();
    expect(screen.getByText("0.1.0")).toBeInTheDocument();
    expect(screen.getByText("0.2.0")).toBeInTheDocument();
    expect(screen.getByText("Faster sessions")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Download" })).toBeEnabled();

    await fireEvent.click(screen.getByRole("button", { name: "Later" }));
    expect(app.updateDialogOpen).toBe(false);
  });

  it("disables download when the manifest has no URL", () => {
    app.updateStatus = {
      enabled: true,
      feedConfigured: true,
      current: "0.1.0",
      checkedAt: 10,
      available: true,
      version: "0.2.0",
      prerelease: false,
      breaking: false,
      notes: "",
      url: null,
      error: null,
    };
    app.updateDialogOpen = true;
    render(UpdateDialog);
    expect(screen.getByRole("button", { name: "Download" })).toBeDisabled();
  });
});
