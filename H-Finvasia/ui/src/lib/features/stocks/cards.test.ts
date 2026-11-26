import { describe, expect, it } from "vitest";
import {
  instrumentDetail,
  instrumentKind,
  mergeOrder,
  moveRelative,
  sortInstrumentRows,
} from "./cards";
import type { ContainerRowDto } from "../../api";

function row(overrides: Partial<ContainerRowDto> = {}): ContainerRowDto {
  return {
    key: "NSE|2885",
    symbol: "RELIANCE",
    name: "Reliance Industries",
    exchange: "NSE",
    instrument_type: "EQ",
    underlying: "RELIANCE",
    expiry: null,
    strike: "0.00",
    strike_type: "XX",
    lot_size: 1,
    is_index: false,
    live: true,
    quote: null,
    ...overrides,
  };
}

describe("group card order", () => {
  it("keeps saved order and appends new groups", () => {
    expect(mergeOrder(["B", "A"], ["A", "B", "C"])).toEqual(["B", "A", "C"]);
  });

  it("drops groups that no longer exist and de-duplicates input", () => {
    expect(mergeOrder(["Gone", "A", "A"], ["A"])).toEqual(["A"]);
  });

  it("moves a dragged card onto the target slot", () => {
    expect(moveRelative(["A", "B", "C", "D"], "D", "B")).toEqual(["A", "D", "B", "C"]);
    expect(moveRelative(["A", "B", "C"], "A", "C")).toEqual(["B", "C", "A"]);
  });

  it("is a no-op for unknown or identical names", () => {
    expect(moveRelative(["A", "B"], "A", "A")).toEqual(["A", "B"]);
    expect(moveRelative(["A", "B"], "Z", "A")).toEqual(["A", "B"]);
  });
});

describe("instrument sorting", () => {
  const rows = [
    { ...row({ key: "A", symbol: "ZETA" }), quote: { ltp: { e6: 300 } } },
    { ...row({ key: "B", symbol: "ALPHA" }), quote: { ltp: { e6: 100 } } },
    { ...row({ key: "C", symbol: "MID" }), quote: null },
    { ...row({ key: "D", symbol: "BETA" }), quote: { ltp: { e6: 200 } } },
  ];

  it("sorts by name", () => {
    expect(sortInstrumentRows(rows, "name").map((item) => item.key)).toEqual([
      "B",
      "D",
      "C",
      "A",
    ]);
  });

  it("sorts by price descending with missing quotes last", () => {
    expect(sortInstrumentRows(rows, "price").map((item) => item.key)).toEqual([
      "A",
      "D",
      "B",
      "C",
    ]);
  });

  it("follows added order and custom order, appending unknown keys", () => {
    expect(sortInstrumentRows(rows, "added", ["D", "C", "B", "A"]).map((item) => item.key)).toEqual(
      ["D", "C", "B", "A"],
    );
    expect(
      sortInstrumentRows(rows, "custom", ["D", "C", "B", "A"], ["B", "A"]).map((item) => item.key),
    ).toEqual(["B", "A", "D", "C"]);
  });
});

describe("instrument labels", () => {
  it("classifies every instrument family", () => {
    expect(instrumentKind(row())).toBe("EQ");
    expect(instrumentKind(row({ is_index: true }))).toBe("IDX");
    expect(instrumentKind(row({ instrument_type: "FUT", strike_type: "XX" }))).toBe("FUT");
    expect(instrumentKind(row({ instrument_type: "OPT", strike_type: "CE" }))).toBe("CE");
    expect(instrumentKind(row({ instrument_type: "OPT", strike_type: "PE" }))).toBe("PE");
    expect(instrumentKind(row({ instrument_type: "BOND" }))).toBe("INS");
  });

  it("shows expiry and strike for options", () => {
    expect(
      instrumentDetail(
        row({
          key: "NFO|1",
          instrument_type: "OPT",
          strike_type: "CE",
          underlying: "NIFTY",
          expiry: "2026-12-31",
          strike: "25800.00",
          name: "NIFTY 25800 CE",
        }),
      ),
    ).toBe("NIFTY | 2026-12-31 | 25800.00");
  });

  it("falls back to the company name for equities", () => {
    expect(instrumentDetail(row())).toBe("Reliance Industries");
    expect(instrumentDetail(row({ name: null, underlying: null }))).toBe("");
  });
});
