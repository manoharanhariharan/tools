import { describe, expect, it } from "vitest";
import { instrumentDetail, instrumentKind, mergeOrder, moveRelative } from "./cards";
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
