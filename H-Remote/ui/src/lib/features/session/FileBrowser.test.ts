import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { beforeEach, describe, expect, it } from "vitest";
import FileBrowser from "./FileBrowser.svelte";
import { mockInvoke, resetMockState } from "../../ipc/mock";
import { app } from "../../state/app.svelte";

describe("file browser", () => {
  beforeEach(async () => {
    resetMockState();
    app.resetForTests();
    await mockInvoke("connect_server", { id: "mock-sftp-1" });
  });

  it("lists remote entries and navigates into a folder", async () => {
    const snapshot = (await mockInvoke("session_list", {})) as Array<{ id: string }>;
    render(FileBrowser, {
      sessionId: snapshot[0].id,
      serverName: "File Server",
    });
    expect(await screen.findByText("readme.txt")).toBeInTheDocument();
    expect(screen.getByText("docs")).toBeInTheDocument();

    await fireEvent.dblClick(screen.getByText("docs"));
    await waitFor(() =>
      expect(
        (screen.getByLabelText("Remote path") as HTMLInputElement).value,
      ).toContain("/docs"),
    );
  });

  it("sorts by size and selects files", async () => {
    const snapshot = (await mockInvoke("session_list", {})) as Array<{ id: string }>;
    render(FileBrowser, {
      sessionId: snapshot[0].id,
      serverName: "File Server",
    });
    await screen.findByText("readme.txt");
    await fireEvent.click(screen.getByRole("columnheader", { name: /Size/ }));
    await fireEvent.click(screen.getByLabelText("Select readme.txt"));
    expect(screen.getByRole("button", { name: "Download" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Rename" })).toBeEnabled();
  });

  it("shows the empty state for an empty folder", async () => {
    const snapshot = (await mockInvoke("session_list", {})) as Array<{ id: string }>;
    render(FileBrowser, {
      sessionId: snapshot[0].id,
      serverName: "File Server",
    });
    expect(await screen.findByText("readme.txt")).toBeInTheDocument();
    expect(screen.queryByText("This folder is empty.")).toBeNull();
  });
});
