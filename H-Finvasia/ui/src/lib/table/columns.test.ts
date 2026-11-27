import { describe, expect, it } from "vitest";
import {
  loadColumnLayout,
  saveColumnLayout,
  type ColumnMeta,
} from "./columns";
import type { StorageLike } from "../layoutPrefs";

const COLUMNS: ColumnMeta[] = [
  { id: "symbol", label: "Symbol", width: 150, min: 90 },
  { id: "ltp", label: "LTP", width: 70, min: 50 },
  { id: "pnl", label: "P&L", width: 80, min: 60 },
];

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

describe("column layout", () => {
  it("defaults to the declared widths and shows every column", () => {
    const layout = loadColumnLayout("fv.test", COLUMNS, storage());
    expect(layout.widths).toEqual({ symbol: 150, ltp: 70, pnl: 80 });
    expect(layout.hidden).toEqual([]);
  });

  it("round-trips widths and hidden columns", () => {
    const store = storage();
    saveColumnLayout("fv.test", { widths: { symbol: 240, ltp: 55, pnl: 80 }, hidden: ["pnl"] }, store);
    const layout = loadColumnLayout("fv.test", COLUMNS, store);
    expect(layout.widths.symbol).toBe(240);
    expect(layout.widths.ltp).toBe(55);
    expect(layout.hidden).toEqual(["pnl"]);
  });

  it("clamps widths and drops unknown/hidden-everything layouts", () => {
    const store = storage({
      "fv.test": JSON.stringify({ widths: { symbol: 4, ltp: 900 }, hidden: ["symbol", "ltp", "pnl", "ghost"] }),
    });
    const layout = loadColumnLayout("fv.test", COLUMNS, store);
    expect(layout.widths.symbol).toBe(90);
    expect(layout.widths.ltp).toBe(640);
    expect(layout.hidden).toEqual([]);
  });

  it("survives corrupt JSON and unavailable storage", () => {
    expect(loadColumnLayout("fv.test", COLUMNS, storage({ "fv.test": "[bad" })).hidden).toEqual([]);
    expect(loadColumnLayout("fv.test", COLUMNS, throwing).widths.symbol).toBe(150);
    expect(() =>
      saveColumnLayout("fv.test", { widths: {}, hidden: [] }, throwing),
    ).not.toThrow();
  });
});
