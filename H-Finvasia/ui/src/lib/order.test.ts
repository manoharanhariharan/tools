import { describe, expect, it } from "vitest";
import type { OrderTargetDto } from "./api";
import { buildOrderRequest, defaultOrderPrice, shortcutSide, type TicketFields } from "./order";

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

  it("defaults a ticket to the LTP snapshot for both sides", () => {
    expect(defaultOrderPrice(target("100.50", "99.50", "100.00"), "B")).toBe("100.00");
    expect(defaultOrderPrice(target("100.50", "99.50", "100.00"), "S")).toBe("100.00");
    expect(defaultOrderPrice(target(null, null, null), "B")).toBe("");
  });
});

describe("ticket request", () => {
  const base: TicketFields = {
    key: "NFO|51001",
    side: "S",
    lots: 2,
    product: "I",
    priceType: "LMT",
    price: "100.00",
    trigger: "",
    trail: "",
    validity: "DAY",
  };

  it("omits trigger and trail for a limit order", () => {
    const request = buildOrderRequest(base);
    expect(request.price).toBe("100.00");
    expect(request.trigger_price).toBeNull();
    expect(request.trail).toBeNull();
  });

  it("includes stop fields for SL-L and TSL", () => {
    const stop = buildOrderRequest({ ...base, priceType: "SL-L", trigger: "99.00" });
    expect(stop.trigger_price).toBe("99.00");
    expect(stop.trail).toBeNull();

    const trail = buildOrderRequest({
      ...base,
      priceType: "TSL",
      trigger: "99.00",
      trail: "1.50",
    });
    expect(trail.trigger_price).toBe("99.00");
    expect(trail.trail).toBe("1.50");
  });

  it("sends a null price when the field is empty", () => {
    expect(buildOrderRequest({ ...base, price: " " }).price).toBeNull();
  });
});
