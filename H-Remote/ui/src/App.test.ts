import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { beforeEach, describe, expect, it } from "vitest";
import App from "./App.svelte";
import { mockInvoke, defaultConfiguration } from "./lib/ipc/mock";
import { mockEmit } from "./lib/ipc/mock";
import { app } from "./lib/state/app.svelte";

async function seedCompletedProfile() {
  const config = defaultConfiguration();
  config.general.guidanceCompleted = true;
  await mockInvoke("update_config", { config });
}

describe("App shell", () => {
  beforeEach(async () => {
    app.resetForTests();
    await seedCompletedProfile();
  });

  it("boots into the catalog shell", async () => {
    render(App);
    expect(await screen.findByTestId("nav-servers")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Servers" })).toBeInTheDocument();
    expect(screen.queryByText("Your sessions, one place")).toBeNull();
  });

  it("navigates to settings and switches themes", async () => {
    render(App);
    await screen.findByTestId("nav-servers");
    await fireEvent.click(screen.getByTestId("nav-settings"));
    expect(await screen.findByTestId("settings-tab-appearance")).toBeInTheDocument();

    await fireEvent.click(screen.getByTestId("settings-tab-appearance"));
    await fireEvent.click(screen.getByTestId("theme-Forest"));
    await waitFor(() => expect(document.documentElement.dataset.theme).toBe("Forest"));
  });

  it("changes language and updates the document locale", async () => {
    render(App);
    await screen.findByTestId("nav-servers");
    await fireEvent.click(screen.getByTestId("nav-settings"));
    const select = (await screen.findByLabelText("Display language")) as HTMLSelectElement;
    await fireEvent.change(select, { target: { value: "de-DE" } });
    await waitFor(() => expect(document.documentElement.lang).toBe("de-DE"));
  });

  it("shows the error report for unhandled UI errors", async () => {
    render(App);
    await screen.findByTestId("nav-servers");
    window.dispatchEvent(
      new ErrorEvent("error", {
        message: "synthetic failure",
        error: new Error("synthetic failure"),
      }),
    );
    expect(await screen.findByText("Something went wrong")).toBeInTheDocument();
    expect(screen.getAllByText(/synthetic failure/).length).toBeGreaterThan(0);
  });

  it("reacts to backend configuration events", async () => {
    render(App);
    await screen.findByTestId("nav-servers");
    const config = defaultConfiguration();
    config.general.guidanceCompleted = true;
    config.general.showNoteInList = false;
    mockEmit("config:changed", config);
    await waitFor(() => expect(app.config?.general.showNoteInList).toBe(false));
  });
});
