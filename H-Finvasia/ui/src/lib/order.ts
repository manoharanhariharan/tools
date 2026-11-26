import type { OrderTargetDto } from "./api";

export interface OrderTicketState {
  target: OrderTargetDto;
  side: "B" | "S";
  price: string;
}

/** Default limit price: buy at ask, sell at bid, then LTP. */
export function defaultOrderPrice(target: OrderTargetDto, side: "B" | "S"): string {
  const quote = target.quote;
  return side === "B"
    ? (quote?.ask?.text ?? quote?.ltp?.text ?? "")
    : (quote?.bid?.text ?? quote?.ltp?.text ?? "");
}

/** F1 = buy, F2 = sell. */
export function shortcutSide(key: string): "B" | "S" | null {
  if (key === "F1") return "B";
  if (key === "F2") return "S";
  return null;
}
