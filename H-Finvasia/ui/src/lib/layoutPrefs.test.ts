import { describe, expect, it } from "vitest";
import {
  COLLAPSED_GROUPS_KEY,
  HEADER_KEY,
  STOCKS_VIEW_KEY,
  loadCollapsedGroups,
  loadHeaderCollapsed,
  loadStocksView,
  saveCollapsedGroups,
  saveHeaderCollapsed,
  saveStocksView,
  type StorageLike,
} from "./layoutPrefs";

function storage(seed: Record<string, string> = {}): StorageLike & { data: Record<string, string> } {
  const data = { ...seed };
  return {
    data,
    getItem: (key) => data[key] ?? null,
    setItem: (key, value) => {
      data[key] = value;
    },
  };
}

const throwing: StorageLike = {
  getItem() {
    throw new Error("denied");
  },
  setItem() {
    throw new Error("denied");
  },
};

describe("layoutPrefs", () => {
  it("defaults to an expanded header and the list view", () => {
    expect(loadHeaderCollapsed(storage())).toBe(false);
    expect(loadStocksView(storage())).toBe("list");
    expect(loadCollapsedGroups(storage())).toEqual({});
  });

  it("round-trips the header and view choice", () => {
    const store = storage();
    saveHeaderCollapsed(true, store);
    saveStocksView("cards", store);
    expect(store.data[HEADER_KEY]).toBe("1");
    expect(store.data[STOCKS_VIEW_KEY]).toBe("cards");
    expect(loadHeaderCollapsed(store)).toBe(true);
    expect(loadStocksView(store)).toBe("cards");
  });

  it("treats malformed values as defaults", () => {
    expect(loadHeaderCollapsed(storage({ [HEADER_KEY]: "yes" }))).toBe(false);
    expect(loadStocksView(storage({ [STOCKS_VIEW_KEY]: "grid" }))).toBe("list");
  });

  it("keeps only boolean per-group flags", () => {
    const store = storage({
      [COLLAPSED_GROUPS_KEY]: JSON.stringify({ Alpha: true, Beta: "yes", Gamma: false }),
    });
    expect(loadCollapsedGroups(store)).toEqual({ Alpha: true, Gamma: false });
    saveCollapsedGroups({ Alpha: false }, store);
    expect(loadCollapsedGroups(store)).toEqual({ Alpha: false });
  });

  it("ignores corrupt JSON and storage failures", () => {
    expect(loadCollapsedGroups(storage({ [COLLAPSED_GROUPS_KEY]: "[not json" }))).toEqual({});
    expect(loadHeaderCollapsed(throwing)).toBe(false);
    expect(loadStocksView(throwing)).toBe("list");
    expect(loadCollapsedGroups(throwing)).toEqual({});
    expect(() => saveHeaderCollapsed(true, throwing)).not.toThrow();
    expect(() => saveStocksView("cards", throwing)).not.toThrow();
    expect(() => saveCollapsedGroups({ A: true }, throwing)).not.toThrow();
  });
});
