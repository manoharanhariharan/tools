import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { beforeEach, describe, expect, it } from "vitest";
import LegacyMigrationDialog from "./LegacyMigrationDialog.svelte";
import { resetMockState } from "../../ipc/mock";
import { app } from "../../state/app.svelte";
import { bootstrapFixture } from "../../../tests/fixtures";

describe("legacy migration dialog", () => {
  beforeEach(() => {
    resetMockState();
    app.resetForTests();
    app.bootstrap = bootstrapFixture({
      legacy: {
        kind: "oneRemote",
        path: "C:\\Users\\me\\AppData\\Roaming\\1Remote",
        configFile: "C:\\Users\\me\\AppData\\Roaming\\1Remote\\1Remote.json",
      },
    });
    app.config = app.bootstrap.config;
  });

  it("shows the detected profile and can be dismissed", async () => {
    app.legacyOpen = true;
    render(LegacyMigrationDialog);

    expect(await screen.findByText("1Remote profile detected")).toBeInTheDocument();
    expect(
      screen.getByText("C:\\Users\\me\\AppData\\Roaming\\1Remote"),
    ).toBeInTheDocument();

    await fireEvent.click(screen.getByRole("button", { name: "Not now" }));
    await waitFor(() => expect(app.legacyOpen).toBe(false));
    expect(app.config?.general.skipLegacyMigration).toBe(true);
  });
});
