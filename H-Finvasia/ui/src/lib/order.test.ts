import { describe, expect, it } from "vitest";
import type { OrderTargetDto } from "./api";
import { defaultOrderPrice, shortcutSide } from "./order";

function target(ask: string | null, bid: string | null, ltp: string | null): OrderTargetDto {
  return {
    key: "NFO|51001",
    trading_symbol: "NIFTY01DEC26C25900",
    exchange: "NFO",
    expiry: "2026-12-01",
    strike: "25900.00",
    strike_type: "CE",
    lot_size: 65,
    instrument_type: "CE",
    is_index: false,
    tradeable: true,
    quote: {
      key: "NFO|51001",
      trading_symbol: "NIFTY01DEC26C25900",
      ltp: ltp ? { text: ltp, e6: 0 } : null,
      change: null,
      pct: null,
      open: null,
      high: null,
      low: null,
      prev_close: null,
      volume: null,
      oi: null,
      prev_oi: null,
      bid: bid ? { text: bid, e6: 0 } : null,
      ask: ask ? { text: ask, e6: 0 } : null,
      bid_qty: null,
      ask_qty: null,
      trend: "flat",
      seq: 0,
      updated_at_ms: 0,
      is_index: false,
      underlying: "NIFTY",
      has_fno: true,
    },
  };
}

describe("order shortcuts", () => {
  it("maps F1 to buy and F2 to sell", () => {
    expect(shortcutSide("F1")).toBe("B");
    expect(shortcutSide("F2")).toBe("S");
    expect(shortcutSide("F3")).toBeNull();
    expect(shortcutSide("1")).toBeNull();
  });

  it("defaults a ticket to ask for buys and bid for sells", () => {
    expect(defaultOrderPrice(target("100.50", "99.50", "100.00"), "B")).toBe("100.50");
    expect(defaultOrderPrice(target("100.50", "99.50", "100.00"), "S")).toBe("99.50");
    expect(defaultOrderPrice(target(null, "99.50", "100.00"), "B")).toBe("100.00");
    expect(defaultOrderPrice(target("100.50", null, "100.00"), "S")).toBe("100.00");
    expect(defaultOrderPrice(target(null, null, null), "B")).toBe("");
  });
});
