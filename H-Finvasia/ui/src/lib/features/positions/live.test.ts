import { describe, expect, it } from "vitest";
import type { PositionDto, QuoteDto } from "../../api";
import { livePosition, liveTotals, positionKey } from "./live";

function position(overrides: Partial<PositionDto> = {}): PositionDto {
  return {
    exchange: "NFO",
    symbol: "NIFTY01DEC26P25500",
    token: "12345",
    product: "M",
    product_alias: "NRML",
    instrument: "OPTIDX",
    lot_size: 65,
    net_qty: 130,
    net_avg: "70.00",
    ltp: "69.70",
    mtm: "-39.00",
    pnl: "0.00",
    day_buy_qty: 130,
    day_sell_qty: 0,
    buy_value: "9308.00",
    sell_value: "0.00",
    ...overrides,
  };
}

function quote(text: string): QuoteDto {
  return {
    key: "NFO|12345",
    trading_symbol: "NIFTY01DEC26P25500",
    ltp: { text, e6: 0 },
    change: null,
    pct: null,
    open: null,
    high: null,
    low: null,
    prev_close: null,
    volume: null,
    oi: null,
    prev_oi: null,
    bid: null,
    ask: null,
    bid_qty: null,
    ask_qty: null,
    trend: "flat",
    seq: 1,
    updated_at_ms: 1,
    is_index: false,
    underlying: "NIFTY",
    has_fno: true,
  };
}

describe("live position P&L", () => {
  it("recomputes LTP, MTM and total P&L from a live quote", () => {
    const live = livePosition(position(), quote("69.00"));
    expect(live.ltp).toBe("69.00");
    expect(live.mtm).toBe("-130.00");
    expect(live.pnl).toBe("-130.00");
  });

  it("adds realised P&L to the live MTM without compounding", () => {
    const starting = position({ pnl: "20.00" });
    const once = livePosition(starting, quote("69.00"));
    const twice = livePosition(starting, quote("68.00"));
    expect(once.pnl).toBe("-110.00");
    expect(twice.pnl).toBe("-240.00");
  });

  it("falls back to the REST row when there is no quote", () => {
    expect(livePosition(position(), null)).toEqual(position());
  });

  it("aggregates live totals for the account header", () => {
    const quotes = new Map([[positionKey(position()), quote("69.00")]]);
    const totals = liveTotals(
      [position(), position({ token: "2", pnl: "10.00", net_qty: 0, mtm: "0.00" })],
      quotes,
    );
    expect(totals.mtm).toBe("-130.00");
    expect(totals.rpnl).toBe("10.00");
    expect(totals.pnl).toBe("-120.00");
  });
});
