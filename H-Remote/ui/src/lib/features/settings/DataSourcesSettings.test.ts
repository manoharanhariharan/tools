import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { beforeEach, describe, expect, it } from "vitest";
import DataSourcesSettings from "./DataSourcesSettings.svelte";
import { resetMockState } from "../../ipc/mock";
import { app } from "../../state/app.svelte";

describe("data sources settings", () => {
  beforeEach(() => {
    resetMockState();
    app.resetForTests();
  });

  it("shows the local source and adds a read-only source", async () => {
    render(DataSourcesSettings);
    expect(await screen.findByText("Local catalog")).toBeInTheDocument();
    expect(await screen.findByText("No additional sources configured.")).toBeInTheDocument();

    await fireEvent.click(screen.getByRole("button", { name: "Add source" }));
    const name = screen.getByPlaceholderText("Team catalog");
    await fireEvent.input(name, { target: { value: "Team DB" } });
    await fireEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => expect(screen.getByText("Team DB")).toBeInTheDocument());
    expect(screen.getAllByText("OK").length).toBeGreaterThanOrEqual(2);
    expect(screen.getByRole("button", { name: "Remove" })).toBeInTheDocument();
  });
});
