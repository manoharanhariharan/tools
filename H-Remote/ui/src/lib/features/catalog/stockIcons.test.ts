import { describe, expect, it } from "vitest";
import { STOCK_ICONS, stockIconDataUrl } from "./stockIcons";

describe("connection icons", () => {
  it("ships a unique, named and renderable set", () => {
    expect(STOCK_ICONS.length).toBeGreaterThanOrEqual(20);
    const ids = new Set(STOCK_ICONS.map((icon) => icon.id));
    expect(ids.size).toBe(STOCK_ICONS.length);
    for (const icon of STOCK_ICONS) {
      expect(icon.name.trim().length).toBeGreaterThan(0);
      expect(icon.svg).toContain('xmlns="http://www.w3.org/2000/svg"');
      expect(stockIconDataUrl(icon)).toMatch(/^data:image\/svg\+xml/);
    }
  });

  it("covers common connection families", () => {
    const ids = new Set(STOCK_ICONS.map((icon) => icon.id));
    for (const id of [
      "windows",
      "linux",
      "apple",
      "server",
      "database",
      "terminal",
      "network",
      "printer",
      "docker",
      "kubernetes",
      "phone",
      "laptop",
    ]) {
      expect(ids.has(id)).toBe(true);
    }
  });
});
