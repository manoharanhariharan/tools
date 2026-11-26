import { describe, expect, it } from "vitest";
import { isClosedOrder } from "./orderStatus";

describe("order status classification", () => {
  it("treats both cancelled spellings as closed", () => {
    expect(isClosedOrder("CANCELLED")).toBe(true);
    expect(isClosedOrder("CANCELED")).toBe(true);
    expect(isClosedOrder(" canceled ")).toBe(true);
    expect(isClosedOrder("CANCELLED ")).toBe(true);
  });

  it("treats completed, rejected, lapsed and expired as closed", () => {
    for (const status of ["COMPLETE", "COMPLETED", "REJECTED", "LAPSED", "EXPIRED"]) {
      expect(isClosedOrder(status)).toBe(true);
    }
  });

  it("keeps live order states pending", () => {
    for (const status of ["OPEN", "TRIGGER_PENDING", "PARTIALLY_FILLED", "PENDING"]) {
      expect(isClosedOrder(status)).toBe(false);
    }
  });
});
