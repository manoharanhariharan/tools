import { describe, expect, it } from "vitest";
import type { OrderDto } from "../../api";
import { orderNotification } from "./notify";

function order(overrides: Partial<OrderDto> = {}): OrderDto {
  return {
    order_no: "1",
    status: "PENDING",
    side: "B",
    price_type: "LMT",
    retention: "DAY",
    quantity: 65,
    price: "100.00",
    average_price: null,
    filled: 0,
    pending: 65,
    rejection: null,
    rejected_by: null,
    exchange: "NFO",
    symbol: "NIFTY01DEC26C25500",
    exchange_order_id: null,
    remarks: null,
    order_time: null,
    ...overrides,
  };
}

describe("order notifications", () => {
  it("surfaces broker rejections with the reason", () => {
    expect(
      orderNotification(
        order({
          status: "REJECTED",
          rejection: "ORA:Product CNC not enabled on exchange NFO",
        }),
      ),
    ).toEqual({
      text: "BUY NIFTY01DEC26C25500 rejected: ORA:Product CNC not enabled on exchange NFO",
      tone: "error",
    });
  });

  it("surfaces completed fills with quantity and average price", () => {
    expect(
      orderNotification(
        order({ status: "COMPLETE", side: "S", filled: 130, average_price: "73.85" }),
      ),
    ).toEqual({
      text: "SELL NIFTY01DEC26C25500 complete 130 @ 73.85",
      tone: "ok",
    });
  });

  it("surfaces cancels and ignores pending orders", () => {
    expect(orderNotification(order({ status: "CANCELED" }))?.tone).toBe("warn");
    expect(orderNotification(order())).toBeNull();
  });
});
