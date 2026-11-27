import { describe, expect, it } from "vitest";
import { ACCENTS, THEMES, isThemeId, lighten, normalizeAccent } from "./theme";

describe("theme helpers", () => {
  it("accepts only known theme ids", () => {
    expect(isThemeId("midnight")).toBe(true);
    expect(isThemeId("light")).toBe(true);
    expect(isThemeId("nord")).toBe(true);
    expect(isThemeId("solarized")).toBe(true);
    expect(isThemeId("tokyo")).toBe(true);
    expect(isThemeId("forest")).toBe(true);
    expect(isThemeId("plum")).toBe(true);
    expect(isThemeId("sand")).toBe(true);
    expect(isThemeId("dark-plus")).toBe(true);
    expect(isThemeId("light-high-contrast")).toBe(true);
    expect(isThemeId("solarized-light")).toBe(true);
    expect(isThemeId("tomorrow-night-blue")).toBe(true);
    expect(isThemeId("neon")).toBe(false);
    expect(isThemeId(null)).toBe(false);
  });

  it("keeps the theme catalogue unique and complete", () => {
    const ids = THEMES.map((theme) => theme.id);
    expect(new Set(ids).size).toBe(ids.length);
    expect(ids.length).toBeGreaterThanOrEqual(30);
    for (const theme of THEMES) {
      expect(theme.label.trim()).not.toBe("");
      expect(theme.swatch).toMatch(/^#[0-9a-fA-F]{6}$/);
    }
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
