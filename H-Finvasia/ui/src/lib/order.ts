import type { OrderTargetDto, PlaceOrderRequest } from "./api";

export interface OrderTicketState {
  target: OrderTargetDto;
  side: "B" | "S";
  price: string;
}

export interface TicketFields {
  key: string;
  side: "B" | "S";
  lots: number;
  product: string;
  priceType: string;
  price: string;
  trigger: string;
  trail: string;
  validity: string;
}

/** Build the wire request; trigger/trail are only sent for stop variants. */
export function buildOrderRequest(fields: TicketFields): PlaceOrderRequest {
  const stop = fields.priceType === "SL-L" || fields.priceType === "TSL";
  const trailing = fields.priceType === "TSL";
  return {
    key: fields.key,
    side: fields.side,
    lots: Math.max(1, Math.round(fields.lots)),
    product: fields.product,
    price_type: fields.priceType,
    price: fields.price.trim() === "" ? null : fields.price.trim(),
    trigger_price: stop && fields.trigger.trim() !== "" ? fields.trigger.trim() : null,
    trail: trailing && fields.trail.trim() !== "" ? fields.trail.trim() : null,
    validity: fields.validity,
  };
}

/**
 * Default ticket price: the LTP that was running when the dialog opened.
 * The quote is a snapshot handed to the ticket, so later ticks cannot move it.
 */
export function defaultOrderPrice(target: OrderTargetDto, _side: "B" | "S"): string {
  return target.quote?.ltp?.text ?? "";
}

/** F1 = buy, F2 = sell. */
export function shortcutSide(key: string): "B" | "S" | null {
  if (key === "F1") return "B";
  if (key === "F2") return "S";
  return null;
}
