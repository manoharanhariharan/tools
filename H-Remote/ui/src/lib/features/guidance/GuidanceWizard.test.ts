import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import GuidanceWizard from "./GuidanceWizard.svelte";
import type { GuidanceResult, GuidanceSelection } from "../../ipc/contracts";
import { bootstrapFixture } from "../../../tests/fixtures";
import { i18n } from "../../i18n/index.svelte";

async function clickNext() {
  await fireEvent.click(screen.getByRole("button", { name: /next/i }));
}

describe("GuidanceWizard", () => {
  it("walks through the steps and reports the selected values", async () => {
    i18n.setLocale("en-US");
    const bootstrap = bootstrapFixture();
    const onfinish = vi.fn(
      async (_selection: GuidanceSelection): Promise<GuidanceResult> => ({
        config: bootstrap.config,
        restartRequired: false,
        migrated: null,
        shortcutPath: null,
        warnings: [],
      }),
    );

    render(GuidanceWizard, { bootstrap, onfinish });

    expect(
      screen.getByRole("heading", { name: "Your sessions, one place" }),
    ).toBeInTheDocument();
    await clickNext();

    expect(
      screen.getByRole("heading", { name: "Where should your profile live?" }),
    ).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: /AppData profile/i }));
    await clickNext();

    const language = screen.getByLabelText("Display language") as HTMLSelectElement;
    await fireEvent.change(language, { target: { value: "de-DE" } });
    await clickNext();

    await fireEvent.click(screen.getByRole("button", { name: /Forest/i }));
    await clickNext();
    await clickNext();
    await clickNext();
    await clickNext();

    expect(screen.getByRole("heading", { name: "Ready" })).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Finish" }));

    expect(onfinish).toHaveBeenCalledTimes(1);
    const selection = onfinish.mock.calls[0][0];
    expect(selection.profileMode).toBe("appData");
    expect(selection.language).toBe("de-DE");
    expect(selection.themeName).toBe("Forest");
  });

  it("respects a locked profile mode", () => {
    const bootstrap = bootstrapFixture({ markerLocked: true });
    render(GuidanceWizard, {
      bootstrap,
      onfinish: vi.fn(),
    });
    expect(
      screen.getByRole("heading", { name: "Your sessions, one place" }),
    ).toBeInTheDocument();
  });
});
