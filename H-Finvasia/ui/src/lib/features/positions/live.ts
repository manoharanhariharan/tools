import type { PositionDto, QuoteDto } from "../../api";
import { parseAmount } from "../../format";

export function positionKey(position: Pick<PositionDto, "exchange" | "token">): string {
  return `${position.exchange}|${position.token}`;
}

/**
 * Recompute one position from the live websocket quote. The broker REST row is
 * never mutated in place, so `pnl` stays the realised leg and cannot compound
 * across ticks.
 */
export function livePosition(position: PositionDto, quote: QuoteDto | null | undefined): PositionDto {
  const ltp = parseAmount(quote?.ltp?.text ?? null);
  const average = parseAmount(position.net_avg);
  if (ltp === null || average === null || position.net_qty === 0) {
    return position;
  }
  const mtm = (ltp - average) * position.net_qty;
  const realised = parseAmount(position.pnl) ?? 0;
  return {
    ...position,
    ltp: quote?.ltp?.text ?? position.ltp,
    mtm: mtm.toFixed(2),
    pnl: (realised + mtm).toFixed(2),
  };
}

export interface LiveTotals {
  mtm: string;
  rpnl: string;
  pnl: string;
}

/** Sum live MTM and realised/total P&L across the position book. */
export function liveTotals(
  positions: PositionDto[],
  quotes: ReadonlyMap<string, QuoteDto>,
): LiveTotals {
  let mtm = 0;
  let rpnl = 0;
  for (const position of positions) {
    const live = livePosition(position, quotes.get(positionKey(position)));
    mtm += parseAmount(live.mtm) ?? 0;
    rpnl += parseAmount(position.pnl) ?? 0;
  }
  return {
    mtm: mtm.toFixed(2),
    rpnl: rpnl.toFixed(2),
    pnl: (mtm + rpnl).toFixed(2),
  };
}
