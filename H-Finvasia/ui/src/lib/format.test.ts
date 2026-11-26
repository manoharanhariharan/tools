import { describe, expect, it } from "vitest";
import { amountTone, formatIndianAmount, parseAmount } from "./format";

describe("amount parsing", () => {
  it("accepts broker strings with separators and a plus sign", () => {
    expect(parseAmount("1,23,456.70")).toBe(123456.7);
    expect(parseAmount(" +1950.00 ")).toBe(1950);
    expect(parseAmount("₹500")).toBe(500);
    expect(parseAmount(-42.5)).toBe(-42.5);
  });

  it("treats accounting parentheses as negative", () => {
    expect(parseAmount("(1,950.00)")).toBe(-1950);
  });

  it("returns null for empty or malformed values", () => {
    expect(parseAmount("")).toBeNull();
    expect(parseAmount(null)).toBeNull();
    expect(parseAmount("n/a")).toBeNull();
  });
});

describe("Indian money formatting", () => {
  it("groups lakhs and crores", () => {
    expect(formatIndianAmount("1234567.89")).toBe("12,34,567.89");
    expect(formatIndianAmount("12345678.9")).toBe("1,23,45,678.90");
    expect(formatIndianAmount("999")).toBe("999.00");
    expect(formatIndianAmount("-1950")).toBe("-1,950.00");
  });

  it("supports signed P&L and zero decimals", () => {
    expect(formatIndianAmount("1950", { signed: true })).toBe("+1,950.00");
    expect(formatIndianAmount("-1950", { signed: true })).toBe("-1,950.00");
    expect(formatIndianAmount("0", { signed: true })).toBe("0.00");
    expect(formatIndianAmount("1234.56", { decimals: 0 })).toBe("1,235");
  });

  it("classifies tone from exact parsed values", () => {
    expect(amountTone("+1,950.00")).toBe("up");
    expect(amountTone("-1,950.00")).toBe("down");
    expect(amountTone("0.00")).toBe("flat");
    expect(amountTone("n/a")).toBe("flat");
  });
});
