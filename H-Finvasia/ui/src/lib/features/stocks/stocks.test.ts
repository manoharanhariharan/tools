import { describe, expect, it } from "vitest";
import type { StockRowDto } from "../../api";
import { matchesStock, sortStocks, visibleSlice } from "./stocks";

function stock(symbol: string, key = `NSE|${symbol}`): StockRowDto {
  return {
    key,
    symbol,
    name: symbol,
    exchange: "NSE",
    live: false,
    underlying: symbol,
    has_fno: false,
    quote: null,
  };
}

describe("stocks list", () => {
  it("moves watched rows above the rest, in watch order", () => {
    const rows = [stock("ACC"), stock("RELIANCE"), stock("TCS"), stock("INFY")];
    const sorted = sortStocks(rows, ["NSE|TCS", "NSE|ACC"]);
    expect(sorted.map((r) => r.symbol)).toEqual(["TCS", "ACC", "INFY", "RELIANCE"]);
  });

  it("keeps alphabetical order for the unwatched tail and breaks ties by key", () => {
    const rows = [stock("B"), stock("A", "NSE|A2"), stock("A", "NSE|A1")];
    const sorted = sortStocks(rows, []);
    expect(sorted.map((r) => r.key)).toEqual(["NSE|A1", "NSE|A2", "NSE|B"]);
  });

  it("matches symbol or company name, case-insensitively", () => {
    const acc = stock("ACC");
    acc.name = "ACC Limited";
    expect(matchesStock(acc, "acc")).toBe(true);
    expect(matchesStock(acc, "limited")).toBe(true);
    expect(matchesStock(acc, "nifty")).toBe(false);
    expect(matchesStock(acc, "  ")).toBe(true);
  });

  it("virtual slice covers the viewport without crossing the ends", () => {
    const rows = Array.from({ length: 100 }, (_, i) => i);
    const middle = visibleSlice(rows, 560, 280, 28);
    expect(middle.start).toBeGreaterThanOrEqual(0);
    expect(middle.end).toBeGreaterThan(middle.start);
    expect(middle.end).toBeLessThanOrEqual(rows.length);
    expect(middle.top).toBe(middle.start * 28);

    const top = visibleSlice(rows, 0, 280, 28);
    expect(top.start).toBe(0);
    expect(top.top).toBe(0);

    const bottom = visibleSlice(rows, 10_000, 280, 28);
    expect(bottom.end).toBe(rows.length);
    expect(bottom.bottom).toBe(0);
  });
});
