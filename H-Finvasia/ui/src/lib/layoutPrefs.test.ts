import { describe, expect, it } from "vitest";
import {
  COLLAPSED_GROUPS_KEY,
  GROUP_SORT_KEY,
  HEADER_KEY,
  INDEX_VIEW_KEY,
  STOCKS_SORT_KEY,
  STOCKS_VIEW_KEY,
  loadCollapsedGroups,
  loadGroupSort,
  loadHeaderCollapsed,
  loadIndexView,
  loadSortMode,
  loadStocksView,
  saveCollapsedGroups,
  saveGroupSort,
  saveHeaderCollapsed,
  saveIndexView,
  saveSortMode,
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
  it("defaults to an expanded header, list view and name sort", () => {
    expect(loadHeaderCollapsed(storage())).toBe(false);
    expect(loadStocksView(storage())).toBe("list");
    expect(loadIndexView(storage())).toBe("list");
    expect(loadSortMode(storage())).toBe("name");
    expect(loadCollapsedGroups(storage())).toEqual({});
  });

  it("round-trips the header, view and sort choice", () => {
    const store = storage();
    saveHeaderCollapsed(true, store);
    saveStocksView("cards", store);
    saveIndexView("cards", store);
    saveSortMode("custom", store);
    expect(store.data[HEADER_KEY]).toBe("1");
    expect(store.data[STOCKS_VIEW_KEY]).toBe("cards");
    expect(store.data[INDEX_VIEW_KEY]).toBe("cards");
    expect(store.data[STOCKS_SORT_KEY]).toBe("custom");
    expect(loadHeaderCollapsed(store)).toBe(true);
    expect(loadStocksView(store)).toBe("cards");
    expect(loadIndexView(store)).toBe("cards");
    expect(loadSortMode(store)).toBe("custom");
  });

  it("treats malformed values as defaults", () => {
    expect(loadHeaderCollapsed(storage({ [HEADER_KEY]: "yes" }))).toBe(false);
    expect(loadStocksView(storage({ [STOCKS_VIEW_KEY]: "grid" }))).toBe("list");
    expect(loadIndexView(storage({ [INDEX_VIEW_KEY]: "grid" }))).toBe("list");
    expect(loadSortMode(storage({ [STOCKS_SORT_KEY]: "volume" }))).toBe("name");
  });

  it("keeps only boolean per-group flags", () => {
    const store = storage({
      [COLLAPSED_GROUPS_KEY]: JSON.stringify({ Alpha: true, Beta: "yes", Gamma: false }),
    });
    expect(loadCollapsedGroups(store)).toEqual({ Alpha: true, Gamma: false });
    saveCollapsedGroups({ Alpha: false }, store);
    expect(loadCollapsedGroups(store)).toEqual({ Alpha: false });
  });

  it("round-trips per-group sort modes", () => {
    const store = storage();
    saveGroupSort({ Alpha: "price", Beta: "custom" }, store);
    expect(store.data[GROUP_SORT_KEY]).toContain("price");
    expect(loadGroupSort(store)).toEqual({ Alpha: "price", Beta: "custom" });
    expect(loadGroupSort(storage({ [GROUP_SORT_KEY]: JSON.stringify({ A: "bad", B: "name" }) }))).toEqual({
      B: "name",
    });
  });

  it("ignores corrupt JSON and storage failures", () => {
    expect(loadCollapsedGroups(storage({ [COLLAPSED_GROUPS_KEY]: "[not json" }))).toEqual({});
    expect(loadHeaderCollapsed(throwing)).toBe(false);
    expect(loadStocksView(throwing)).toBe("list");
    expect(loadIndexView(throwing)).toBe("list");
    expect(loadCollapsedGroups(throwing)).toEqual({});
    expect(() => saveHeaderCollapsed(true, throwing)).not.toThrow();
    expect(() => saveStocksView("cards", throwing)).not.toThrow();
    expect(() => saveIndexView("cards", throwing)).not.toThrow();
    expect(() => saveCollapsedGroups({ A: true }, throwing)).not.toThrow();
  });
});
