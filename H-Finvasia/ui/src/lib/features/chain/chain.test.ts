import { describe, expect, it } from "vitest";
import type { ChainRowDto, QuoteDto } from "../../api";
import {
  defaultTicketPrice,
  deltaOi,
  fillPercent,
  groupStrikes,
  pickAtm,
  sideMoneyness,
  strikeE6,
  windowAround,
} from "./chain";

function quote(oi: number | null, prevOi: number | null): QuoteDto {
  return {
    key: "x",
    trading_symbol: null,
    ltp: null,
    change: null,
    pct: null,
    open: null,
    high: null,
    low: null,
    prev_close: null,
    volume: null,
    oi,
    prev_oi: prevOi,
    bid: null,
    ask: null,
    bid_qty: null,
    ask_qty: null,
    trend: "flat",
    seq: 0,
    updated_at_ms: 0,
    is_index: false,
    underlying: "NIFTY",
    has_fno: true,
  };
}

function leg(strike: string, side: "CE" | "PE", live: boolean, q: QuoteDto | null = null): ChainRowDto {
  return {
    key: `NFO|${strike}${side}`,
    exchange: "NFO",
    symbol: `NIFTY${strike}${side}`,
    expiry: "2026-12-01",
    strike,
    strike_type: side,
    lot_size: 65,
    quote: q,
    live,
  };
}

describe("option chain ladder", () => {
  it("pairs calls and puts by strike and sorts numerically", () => {
    const ladder = groupStrikes([
      leg("25800.00", "PE", true),
      leg("25800.00", "CE", true),
      leg("25750.00", "CE", false),
      leg("26000.00", "PE", false),
    ]);
    expect(ladder.map((r) => r.strike)).toEqual(["25750.00", "25800.00", "26000.00"]);
    expect(ladder[1].ce?.strike_type).toBe("CE");
    expect(ladder[1].pe?.strike_type).toBe("PE");
    expect(ladder[0].pe).toBeNull();
    expect(ladder[2].ce).toBeNull();
  });

  it("picks the median subscribed strike as ATM", () => {
    const ladder = groupStrikes([
      leg("25700.00", "CE", true),
      leg("25750.00", "CE", true),
      leg("25800.00", "CE", true),
      leg("25850.00", "CE", false),
      leg("25900.00", "CE", false),
    ]);
    expect(pickAtm(ladder)).toBe(strikeE6("25750.00"));
  });

  it("snaps an explicit ATM to the nearest listed strike", () => {
    const ladder = groupStrikes([
      leg("25700.00", "CE", true),
      leg("25800.00", "CE", true),
      leg("25900.00", "CE", true),
    ]);
    expect(pickAtm(ladder, "25,740")).toBe(strikeE6("25700.00"));
    expect(pickAtm(ladder, "25890")).toBe(strikeE6("25900.00"));
  });

  it("windows 10 strikes each side and pads near the edge", () => {
    const rows = Array.from({ length: 600 }, (_, i) => {
      const strike = (25000 + i * 50).toFixed(2);
      return [leg(strike, "CE", true), leg(strike, "PE", true)];
    }).flat();
    const ladder = groupStrikes(rows);
    const atm = strikeE6("25800.00");
    const window = windowAround(ladder, atm, 10);
    expect(window).toHaveLength(21);
    expect(window[0].strikeE6).toBe(strikeE6("25300.00"));
    expect(window[10].strikeE6).toBe(atm);
    expect(window[20].strikeE6).toBe(strikeE6("26300.00"));

    const edge = windowAround(ladder, ladder[0].strikeE6, 10);
    expect(edge).toHaveLength(21);
    expect(edge[0].strike).toBe(ladder[0].strike);
  });

  it("shades calls and puts on opposite sides of the reference", () => {
    const atm = strikeE6("25800.00");
    expect(sideMoneyness("CE", strikeE6("25700.00"), atm)).toBe("itm");
    expect(sideMoneyness("CE", strikeE6("25900.00"), atm)).toBe("otm");
    expect(sideMoneyness("PE", strikeE6("25900.00"), atm)).toBe("itm");
    expect(sideMoneyness("PE", strikeE6("25700.00"), atm)).toBe("otm");
    expect(sideMoneyness("PE", atm, atm)).toBe("atm");
  });

  it("defaults a ticket to ask for buys and bid for sells", () => {
    const q = quote(1200, 1000);
    q.ltp = { text: "100.00", e6: 100_000_000 };
    q.bid = { text: "99.50", e6: 99_500_000 };
    q.ask = { text: "100.50", e6: 100_500_000 };
    const row = leg("25800.00", "CE", true, q);
    expect(defaultTicketPrice(row, "B")).toBe("100.50");
    expect(defaultTicketPrice(row, "S")).toBe("99.50");
    q.ask = null;
    expect(defaultTicketPrice(row, "B")).toBe("100.00");
    q.bid = null;
    expect(defaultTicketPrice(row, "S")).toBe("100.00");
  });

  it("computes delta OI and background fill percentages", () => {
    expect(deltaOi(quote(1200, 1000))).toBe(200);
    expect(deltaOi(quote(800, 1000))).toBe(-200);
    expect(deltaOi(quote(1200, null))).toBeNull();
    expect(deltaOi(null)).toBeNull();

    expect(fillPercent(50, 100)).toBe(50);
    expect(fillPercent(500, 100)).toBe(100);
    expect(fillPercent(0, 100)).toBe(0);
    expect(fillPercent(null, 100)).toBe(0);
    expect(fillPercent(50, 0)).toBe(0);
  });
});
