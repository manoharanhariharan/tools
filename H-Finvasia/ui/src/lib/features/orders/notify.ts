import type { OrderDto } from "../../api";

export interface OrderNotification {
  text: string;
  tone: "ok" | "error" | "warn";
}

const COMPLETE = new Set(["COMPLETE", "COMPLETED", "FILLED"]);
const CLOSED = new Set(["CANCELED", "CANCELLED", "LAPSED", "EXPIRED"]);

/** Turn a terminal order-book change into a screen notification, or null. */
export function orderNotification(order: OrderDto): OrderNotification | null {
  const status = order.status.trim().toUpperCase();
  const side = order.side.trim().toUpperCase() === "S" ? "SELL" : "BUY";
  if (status === "REJECTED") {
    return {
      text: `${side} ${order.symbol} rejected: ${order.rejection ?? "broker rejected the order"}`,
      tone: "error",
    };
  }
  if (COMPLETE.has(status)) {
    return {
      text: `${side} ${order.symbol} ${status.toLowerCase()} ${order.filled} @ ${
        order.average_price ?? order.price ?? "-"
      }`,
      tone: "ok",
    };
  }
  if (CLOSED.has(status)) {
    return { text: `${side} ${order.symbol} ${status.toLowerCase()}`, tone: "warn" };
  }
  return null;
}
