import { describe, expect, it } from "vitest";
import { ACCENTS, isThemeId, lighten, normalizeAccent } from "./theme";

describe("theme helpers", () => {
  it("accepts only known theme ids", () => {
    expect(isThemeId("midnight")).toBe(true);
    expect(isThemeId("light")).toBe(true);
    expect(isThemeId("neon")).toBe(false);
    expect(isThemeId(null)).toBe(false);
  });

  it("normalizes custom accents and rejects malformed values", () => {
    expect(normalizeAccent("#12ab34")).toBe("#12AB34");
    expect(normalizeAccent("12ab34")).toBe(ACCENTS[0].accent);
    expect(normalizeAccent("red")).toBe(ACCENTS[0].accent);
  });

  it("lightens an accent for the hover colour", () => {
    expect(lighten("#000000", 0.2)).toBe("#333333");
    expect(lighten("#FFFFFF", 0.2)).toBe("#FFFFFF");
    expect(lighten("#2A6EA8")).toMatch(/^#[0-9A-F]{6}$/);
  });
});
