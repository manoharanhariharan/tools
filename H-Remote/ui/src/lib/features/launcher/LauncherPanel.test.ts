import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { beforeEach, describe, expect, it } from "vitest";
import LauncherPanel from "./LauncherPanel.svelte";
import { resetMockState } from "../../ipc/mock";
import { app } from "../../state/app.svelte";
import { sessions } from "../../state/sessions.svelte";

describe("launcher panel", () => {
  beforeEach(() => {
    resetMockState();
    app.resetForTests();
    sessions.resetForTests();
  });

  it("filters servers and highlights matches", async () => {
    render(LauncherPanel);
    const input = await screen.findByPlaceholderText(/Search servers/);
    await fireEvent.input(input, { target: { value: "Pro" } });
    await waitFor(() =>
      expect(
        screen.getAllByRole("option").some((item) =>
          item.textContent?.includes("Production RDP"),
        ),
      ).toBe(true),
    );
    const production = screen
      .getAllByRole("option")
      .find((item) => item.textContent?.includes("Production RDP"))!;
    expect(production.querySelector("mark")?.textContent?.toLowerCase()).toBe("pro");
  });

  it("supports arrow selection and Tab into quick connect with prefill", async () => {
    render(LauncherPanel);
    const input = await screen.findByPlaceholderText(/Search servers/);
    await fireEvent.keyDown(input, { key: "Tab" });

    expect(await screen.findByRole("heading", { name: "Quick connect" })).toBeInTheDocument();
    const address = screen.getByPlaceholderText("host or IP") as HTMLInputElement;
    expect(address.value).toBe("10.0.0.20");
    expect(
      (screen.getByLabelText("Username") as HTMLInputElement).value,
    ).toBe("root");
  });

  it("navigates results and connects the selected server", async () => {
    render(LauncherPanel);
    const input = await screen.findByPlaceholderText(/Search servers/);
    await waitFor(() => expect(screen.getAllByRole("option").length).toBeGreaterThan(1));

    await fireEvent.keyDown(input, { key: "ArrowDown" });
    await fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(sessions.sessions).toHaveLength(1));
    expect(sessions.sessions[0].serverName.length).toBeGreaterThan(0);
  });

  it("runs a quick connect and keeps history", async () => {
    render(LauncherPanel);
    const input = await screen.findByPlaceholderText(/Search servers/);
    await fireEvent.keyDown(input, { key: "Tab" });

    const address = screen.getByPlaceholderText("host or IP");
    await fireEvent.input(address, { target: { value: "10.9.9.9" } });
    const port = document.querySelector<HTMLInputElement>('input[type="number"]')!;
    await fireEvent.input(port, { target: { value: "3390" } });

    await fireEvent.click(screen.getByRole("button", { name: "Connect" }));
    await waitFor(() =>
      expect(screen.getByText("10.9.9.9:3390")).toBeInTheDocument(),
    );
    expect(sessions.sessions).toHaveLength(1);
  });

  it("opens quick connect from the keyboard shortcut without a selection", async () => {
    render(LauncherPanel);
    const input = await screen.findByPlaceholderText(/Search servers/);
    await fireEvent.input(input, { target: { value: "zzzz-no-match" } });
    await waitFor(() => expect(screen.getByText(/No matching servers/)).toBeInTheDocument());
    await fireEvent.keyDown(input, { key: "Tab" });
    expect(await screen.findByRole("heading", { name: "Quick connect" })).toBeInTheDocument();
    expect((screen.getByPlaceholderText("host or IP") as HTMLInputElement).value).toBe("");
  });
});
