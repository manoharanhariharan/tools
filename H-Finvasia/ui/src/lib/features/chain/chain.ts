import type { ChainRowDto, QuoteDto } from "../../api";

export type Moneyness = "itm" | "atm" | "otm";

/** Strike in the same integer unit as `PriceDto.e6` (1e-6). */
export function strikeE6(strike: string): number {
  const value = Number(strike.replace(/,/g, ""));
  return Number.isFinite(value) ? Math.round(value * 1_000_000) : 0;
}

export interface StrikeLadderRow {
  strike: string;
  strikeE6: number;
  ce: ChainRowDto | null;
  pe: ChainRowDto | null;
}

/** Pair the flat CE/PE leg list into one ladder row per strike. */
export function groupStrikes(rows: ChainRowDto[]): StrikeLadderRow[] {
  const byStrike = new Map<number, StrikeLadderRow>();
  for (const row of rows) {
    const e6 = strikeE6(row.strike);
    let entry = byStrike.get(e6);
    if (!entry) {
      entry = { strike: row.strike, strikeE6: e6, ce: null, pe: null };
      byStrike.set(e6, entry);
    }
    if (row.strike_type === "CE") entry.ce = row;
    else if (row.strike_type === "PE") entry.pe = row;
  }
  return [...byStrike.values()].sort((a, b) => a.strikeE6 - b.strikeE6);
}

function nearestIndex(ladder: StrikeLadderRow[], target: number): number {
  if (ladder.length === 0) return -1;
  let best = 0;
  let bestDistance = Number.POSITIVE_INFINITY;
  ladder.forEach((row, i) => {
    const distance = Math.abs(row.strikeE6 - target);
    if (distance < bestDistance) {
      best = i;
      bestDistance = distance;
    }
  });
  return best;
}

/**
 * The ATM strike:
 * 1. an explicit user value wins (snapped to the nearest listed strike);
 * 2. otherwise the median of the currently subscribed strikes;
 * 3. otherwise the middle of the full chain (the backend makes the same call).
 */
export function pickAtm(ladder: StrikeLadderRow[], preferred?: string | null): number | null {
  if (ladder.length === 0) return null;
  const wanted = preferred?.trim();
  if (wanted) {
    const parsed = Number(wanted.replace(/,/g, ""));
    if (Number.isFinite(parsed)) {
      return ladder[nearestIndex(ladder, Math.round(parsed * 1_000_000))].strikeE6;
    }
  }
  const live = ladder.filter((row) => row.ce?.live || row.pe?.live);
  if (live.length > 0) return live[Math.floor(live.length / 2)].strikeE6;
  return ladder[Math.floor(ladder.length / 2)].strikeE6;
}

/**
 * Take `eachSide` strikes on either side of the ATM, padding at the edges so the
 * default view stays a full 2N+1 ladder whenever the chain is long enough.
 */
export function windowAround(ladder: StrikeLadderRow[], atmE6: number | null, eachSide: number): StrikeLadderRow[] {
  if (ladder.length === 0) return [];
  const side = Math.max(1, Math.min(20, Math.round(eachSide)));
  const centre = atmE6 === null ? nearestIndex(ladder, ladder[0].strikeE6) : nearestIndex(ladder, atmE6);
  const desired = Math.min(ladder.length, side * 2 + 1);
  let start = centre - side;
  let end = centre + side + 1;
  if (start < 0) {
    start = 0;
    end = desired;
  }
  if (end > ladder.length) {
    end = ladder.length;
    start = Math.max(0, end - desired);
  }
  return ladder.slice(start, end);
}

/** Relative position of a strike to the reference (spot, else ATM). */
export function moneyness(strike: number, reference: number): Moneyness {
  if (strike === reference) return "atm";
  return strike < reference ? "itm" : "otm";
}

/** Calls are ITM below the reference, puts above it. */
export function sideMoneyness(side: "CE" | "PE", strike: number, reference: number): Moneyness {
  const base = moneyness(strike, reference);
  if (base === "atm") return "atm";
  if (side === "CE") return base;
  return base === "itm" ? "otm" : "itm";
}

/** Day change in open interest, when the venue sends both legs. */
export function deltaOi(quote: QuoteDto | null | undefined): number | null {
  if (!quote || quote.oi === null || quote.prev_oi === null) return null;
  return quote.oi - quote.prev_oi;
}

/** Percentage of `max` used for the background fill, clamped to 0..100. */
export function fillPercent(value: number | null | undefined, max: number): number {
  if (!value || value <= 0 || max <= 0) return 0;
  return Math.max(0, Math.min(100, (value / max) * 100));
}
