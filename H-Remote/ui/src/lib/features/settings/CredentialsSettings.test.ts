import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { beforeEach, describe, expect, it } from "vitest";
import CredentialsSettings from "./CredentialsSettings.svelte";
import { resetMockState } from "../../ipc/mock";
import { app } from "../../state/app.svelte";

describe("credential vault", () => {
  beforeEach(() => {
    resetMockState();
    app.resetForTests();
  });

  it("lists credentials grouped by source", async () => {
    render(CredentialsSettings);
    expect(await screen.findByText("Prod admin")).toBeInTheDocument();
    expect(screen.getByText("Local")).toBeInTheDocument();
  });

  it("creates a credential", async () => {
    render(CredentialsSettings);
    await screen.findByText("Prod admin");
    await fireEvent.click(screen.getByRole("button", { name: "Add credential" }));

    const name = screen.getByPlaceholderText("prod-admin");
    await fireEvent.input(name, { target: { value: "staging" } });
    const username = document.querySelectorAll<HTMLInputElement>(".form input")[1];
    await fireEvent.input(username, { target: { value: "deploy" } });
    await fireEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => expect(screen.getByText("staging")).toBeInTheDocument());
  });
});
