import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { tick } from "svelte";
import { beforeEach, describe, expect, it } from "vitest";
import DialogHost from "../components/DialogHost.svelte";
import { dialogs } from "../state/dialogs.svelte";

describe("DialogHost", () => {
  beforeEach(() => {
    dialogs.close(null);
  });

  it("submits input values", async () => {
    render(DialogHost);
    const promise = dialogs.input({
      title: "Rename",
      label: "Name",
      value: "old",
      submitLabel: "Save",
    });
    await tick();

    const input = screen.getByLabelText("Name") as HTMLInputElement;
    expect(input.value).toBe("old");
    await fireEvent.input(input, { target: { value: "new name" } });
    await fireEvent.click(screen.getByRole("button", { name: "Save" }));

    await expect(promise).resolves.toBe("new name");
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });

  it("reveals passwords and submits them", async () => {
    render(DialogHost);
    const promise = dialogs.password({ title: "Password", label: "Secret" });
    await tick();

    const input = screen.getByLabelText("Secret") as HTMLInputElement;
    expect(input.type).toBe("password");
    await fireEvent.click(screen.getByRole("button", { name: "Show password" }));
    expect(input.type).toBe("text");
    await fireEvent.input(input, { target: { value: "hunter2" } });
    await fireEvent.click(screen.getByRole("button", { name: "OK" }));
    await expect(promise).resolves.toBe("hunter2");
  });

  it("cancels with Escape and restores focus", async () => {
    const opener = document.createElement("button");
    opener.textContent = "opener";
    document.body.appendChild(opener);
    opener.focus();

    render(DialogHost);
    const promise = dialogs.input({ title: "Name", label: "Name" });
    await tick();
    await fireEvent.keyDown(document, { key: "Escape" });
    await expect(promise).resolves.toBeNull();
    await waitFor(() => expect(document.activeElement).toBe(opener));
    opener.remove();
  });

  it("resolves message buttons", async () => {
    render(DialogHost);
    const promise = dialogs.message({
      title: "Warning",
      message: "Proceed?",
      variant: "warning",
      buttons: [
        { value: "cancel", label: "Cancel" },
        { value: "proceed", label: "Proceed", variant: "danger" },
      ],
    });
    await tick();
    await fireEvent.click(screen.getByRole("button", { name: "Proceed" }));
    await expect(promise).resolves.toBe("proceed");
  });
});
