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

export interface MarginInput {
  target: Pick<OrderTargetDto, "strike" | "strike_type" | "instrument_type" | "is_index" | "expiry">;
  side: "B" | "S";
  product: string;
  price: string;
  quantity: number;
}

/**
 * Internal pre-trade margin estimate, used instead of a broker round-trip.
 *
 * This is deliberately approximate and labelled as such in the ticket:
 * - option buys: premium (quantity x price)
 * - option sells: max(premium, 15% of strike notional)
 * - futures: 15% of notional
 * - equity: 20% intraday, 50% margin product, 100% delivery
 */
export function estimateMargin(input: MarginInput): string {
  const price = Number(input.price.replace(/[,₹\s]/g, ""));
  const quantity = Math.max(0, Math.round(input.quantity));
  if (!Number.isFinite(price) || price <= 0 || quantity <= 0) return "-";

  const type = input.target.instrument_type.trim().toUpperCase();
  const option =
    input.target.strike_type === "CE" ||
    input.target.strike_type === "PE" ||
    type === "CE" ||
    type === "PE" ||
    type === "OPT";
  const future =
    type === "FUT" ||
    (!!input.target.expiry && !option && !input.target.is_index && input.target.strike_type !== "CE" && input.target.strike_type !== "PE");

  const notional = quantity * price;
  let margin: number;
  if (option) {
    const strike = Number(input.target.strike);
    margin =
      input.side === "B"
        ? notional
        : Math.max(notional, (Number.isFinite(strike) ? strike : price) * quantity * 0.15);
  } else if (future) {
    margin = notional * 0.15;
  } else {
    const product = input.product.trim().toUpperCase();
    margin = notional * (product === "I" ? 0.2 : product === "M" ? 0.5 : 1);
  }
  return margin.toFixed(2);
}
