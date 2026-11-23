import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { beforeEach, describe, expect, it } from "vitest";
import RatingDialog from "./RatingDialog.svelte";
import { resetMockState } from "../../ipc/mock";
import { app } from "../../state/app.svelte";
import { bootstrapFixture } from "../../../tests/fixtures";

describe("rating dialog", () => {
  beforeEach(() => {
    resetMockState();
    app.resetForTests();
    app.bootstrap = bootstrapFixture({ firstRun: false });
    app.config = app.bootstrap.config;
  });

  it("records a permanent opt-out", async () => {
    app.ratingOpen = true;
    render(RatingDialog);

    await fireEvent.click(screen.getByRole("button", { name: "Don't ask again" }));
    await waitFor(() => expect(app.ratingOpen).toBe(false));
    expect(app.config?.engagement.doNotShowRating).toBe(true);
  });

  it("can be postponed", async () => {
    app.ratingOpen = true;
    render(RatingDialog);

    await fireEvent.click(screen.getByRole("button", { name: "Later" }));
    await waitFor(() => expect(app.ratingOpen).toBe(false));
    expect(app.config?.engagement.doNotShowRating).toBe(false);
    expect(app.config?.engagement.lastRatingPromptAt).not.toBeNull();
  });
});
