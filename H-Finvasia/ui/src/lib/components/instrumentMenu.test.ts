import { describe, expect, it } from "vitest";
import type { OrderTargetDto } from "../api";
import { instrumentMenuItems } from "./instrumentMenu";

function target(overrides: Partial<OrderTargetDto> = {}): OrderTargetDto {
  return {
    key: "NSE|2885",
    trading_symbol: "RELIANCE-EQ",
    exchange: "NSE",
    expiry: null,
    strike: "0.00",
    strike_type: "XX",
    lot_size: 1,
    instrument_type: "EQ",
    is_index: false,
    tradeable: true,
    underlying: "RELIANCE",
    has_fno: true,
    quote: null,
    ...overrides,
  };
}

const handlers = {
  onbuy: () => {},
  onsell: () => {},
  onchain: () => {},
};

describe("instrument menu", () => {
  it("shows F1/F2 for tradeable instruments and appends row actions", () => {
    const labels = instrumentMenuItems(target(), true, "NSE|2885", "RELIANCE", [
      { label: "Remove from watch" },
    ], handlers).map((item) => item.label);
    expect(labels).toEqual([
      "F1 — Buy RELIANCE-EQ",
      "F2 — Sell RELIANCE-EQ",
      "Open option chain (RELIANCE)",
      "Remove from watch",
    ]);
  });

  it("hides F1/F2 for an index but still opens its chain", () => {
    const items = instrumentMenuItems(
      target({
        trading_symbol: "NIFTY 50",
        instrument_type: "INDEX",
        is_index: true,
        tradeable: false,
        underlying: "NIFTY",
        has_fno: true,
      }),
      true,
      "NSE|26000",
      "NIFTY 50",
      [],
      handlers,
    );
    expect(items.map((item) => item.label)).toEqual([
      "NIFTY 50 is not tradeable",
      "Open option chain (NIFTY)",
    ]);
  });

  it("omits the chain for a cash scrip without F&O", () => {
    const items = instrumentMenuItems(
      target({ underlying: "ABC", has_fno: false }),
      true,
      "NSE|1",
      "ABC",
      [],
      handlers,
    );
    expect(items.map((item) => item.label)).toEqual(["F1 — Buy RELIANCE-EQ", "F2 — Sell RELIANCE-EQ"]);
  });

  it("is explicit while metadata is loading or missing", () => {
    expect(instrumentMenuItems(null, false, "X", "X", [], handlers)[0].label).toBe(
      "Loading instrument…",
    );
    expect(instrumentMenuItems(null, true, "X", "X", [], handlers)[0].label).toBe(
      "Instrument metadata unavailable",
    );
  });
});
