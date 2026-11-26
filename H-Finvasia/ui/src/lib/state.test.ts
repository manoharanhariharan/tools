import { describe, expect, it } from "vitest";
import { formatVolume, QuoteTable, toRow, trendClass } from "./state";
import type { QuoteDto } from "./api";

function quote(overrides: Partial<QuoteDto> = {}): QuoteDto {
  return {
    key: "NSE|22",
    trading_symbol: "ACC-EQ",
    ltp: { text: "1287.30", e6: 1_287_300_000 },
    change: { text: "12.10", e6: 12_100_000 },
    pct: "0.95%",
    open: null,
    high: null,
    low: null,
    prev_close: { text: "1275.20", e6: 1_275_200_000 },
    volume: 980_515,
    bid: { text: "1287.25", e6: 1_287_250_000 },
    ask: { text: "1287.35", e6: 1_287_350_000 },
    bid_qty: 26,
    ask_qty: 6325,
    trend: "up",
    seq: 1,
    updated_at_ms: 1,
    is_index: false,
    ...overrides,
  };
}

describe("formatVolume", () => {
  it("uses Indian units", () => {
    expect(formatVolume(980)).toBe("980");
    expect(formatVolume(980_515)).toBe("9.81L");
    expect(formatVolume(12_300_000)).toBe("1.23Cr");
    expect(formatVolume(4_500)).toBe("4.5K");
  });

  it("renders a dash for missing values", () => {
    expect(formatVolume(null)).toBe("â€”");
    expect(formatVolume(undefined)).toBe("â€”");
  });

  it("keeps negative values readable", () => {
    expect(formatVolume(-980_515)).toBe("-9.81L");
  });
});

describe("trendClass", () => {
  it("maps trends to css classes", () => {
    expect(trendClass("up")).toBe("up");
    expect(trendClass("down")).toBe("down");
    expect(trendClass("flat")).toBe("flat");
  });
});

describe("toRow", () => {
  it("falls back to the key when there is no symbol", () => {
    const row = toRow(quote({ trading_symbol: null }));
    expect(row.symbol).toBe("NSE|22");
  });

  it("uses dashes for absent prices", () => {
    const row = toRow(quote({ ltp: null, change: null, pct: null, bid: null, ask: null }));
    expect(row.ltp).toBe("â€”");
    expect(row.change).toBe("â€”");
    expect(row.pct).toBe("â€”");
  });
});

describe("QuoteTable", () => {
  it("keeps watch order stable and sorted", () => {
    const table = new QuoteTable();
    table.setKeys(["NSE|3", "NSE|1", "NSE|2"]);
    expect(table.keys()).toEqual(["NSE|1", "NSE|2", "NSE|3"]);
  });

  it("prunes rows that leave the watch list", () => {
    const table = new QuoteTable();
    table.setKeys(["NSE|1", "NSE|2"], [quote({ key: "NSE|1" }), quote({ key: "NSE|2" })]);
    expect(table.size()).toBe(2);

    table.setKeys(["NSE|2"]);
    expect(table.size()).toBe(1);
    expect(table.get("NSE|1")).toBeUndefined();
    expect(table.get("NSE|2")).toBeDefined();
  });

  it("deduplicates repeated keys", () => {
    const table = new QuoteTable();
    table.setKeys(["NSE|1", "NSE|1", "NSE|1"]);
    expect(table.keys()).toEqual(["NSE|1"]);
  });

  it("merges deltas and updates existing rows", () => {
    const table = new QuoteTable();
    table.setKeys(["NSE|22"], [quote()]);

    table.upsert([
      quote({ ltp: { text: "1290.00", e6: 1_290_000_000 }, trend: "up", seq: 2 }),
    ]);

    const row = table.get("NSE|22");
    expect(row?.ltp).toBe("1290.00");
    expect(row?.seq).toBe(2);
    expect(table.size()).toBe(1);
  });

  it("shows a placeholder row immediately for a newly watched key", () => {
    const table = new QuoteTable();
    table.setKeys(["NSE|2885"]);
    const row = table.get("NSE|2885");
    expect(row).toBeDefined();
    expect(row?.ltp).toBe("â€”");
    expect(table.rows()).toHaveLength(1);
  });

  it("ignores deltas for instruments that are not on the watch list", () => {
    const table = new QuoteTable();
    table.setKeys(["NSE|1"]);
    table.upsert([quote({ key: "NSE|9" })]);
    expect(table.keys()).not.toContain("NSE|9");
    expect(table.size()).toBe(1);
  });

  it("exposes rows in key order", () => {
    const table = new QuoteTable();
    table.setKeys(["NSE|2", "NSE|1"], [quote({ key: "NSE|2" }), quote({ key: "NSE|1" })]);
    expect(table.rows().map((r) => r.key)).toEqual(["NSE|1", "NSE|2"]);
  });

  it("clears completely", () => {
    const table = new QuoteTable();
    table.setKeys(["NSE|1"], [quote({ key: "NSE|1" })]);
    table.clear();
    expect(table.size()).toBe(0);
    expect(table.rows()).toEqual([]);
  });
});
