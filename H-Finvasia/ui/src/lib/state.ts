import type { QuoteDto, Trend } from "./api";

/** A display-ready row. Formatting happens once, not per render. */
export interface QuoteRow {
  key: string;
  symbol: string;
  ltp: string;
  change: string;
  pct: string;
  bid: string;
  ask: string;
  volume: string;
  trend: Trend;
  seq: number;
  updated: number;
  isIndex: boolean;
}

const DASH = "-";

export function formatVolume(value: number | null | undefined): string {
  if (value === null || value === undefined) return DASH;
  const abs = Math.abs(value);
  if (abs >= 1e7) return `${(value / 1e7).toFixed(2)}Cr`;
  if (abs >= 1e5) return `${(value / 1e5).toFixed(2)}L`;
  if (abs >= 1e3) return `${(value / 1e3).toFixed(1)}K`;
  return String(value);
}

export function trendClass(trend: Trend): string {
  return trend === "up" ? "up" : trend === "down" ? "down" : "flat";
}

export function toRow(quote: QuoteDto): QuoteRow {
  return {
    key: quote.key,
    symbol: quote.trading_symbol ?? quote.key,
    ltp: quote.ltp?.text ?? DASH,
    change: quote.change?.text ?? DASH,
    pct: quote.pct ?? DASH,
    bid: quote.bid?.text ?? DASH,
    ask: quote.ask?.text ?? DASH,
    volume: formatVolume(quote.volume),
    trend: quote.trend,
    seq: quote.seq,
    updated: quote.updated_at_ms,
    isIndex: quote.is_index,
  };
}

/**
 * A watched instrument with no tick yet. Showing the row immediately (with
 * dashes) is deliberate: otherwise adding an illiquid scrip appears to do
 * nothing until its first tick, which may be minutes away.
 */
export function placeholderRow(key: string): QuoteRow {
  return {
    key,
    symbol: key,
    ltp: DASH,
    change: DASH,
    pct: DASH,
    bid: DASH,
    ask: DASH,
    volume: DASH,
    trend: "flat",
    seq: 0,
    updated: 0,
    isIndex: false,
  };
}

/**
 * Holds the watched rows.
 *
 * Deliberately not reactive itself: components copy `rows()` into `$state` after
 * each delta, which keeps the update path explicit and testable without a DOM.
 */
export class QuoteTable {
  private byKey = new Map<string, QuoteRow>();
  private order: string[] = [];

  /** Replace the watch list (sorted for stable rendering) and prune stale rows. */
  setKeys(keys: string[], quotes: QuoteDto[] = []): void {
    this.order = [...new Set(keys)].sort();
    const keep = new Set(this.order);
    for (const key of [...this.byKey.keys()]) {
      if (!keep.has(key)) this.byKey.delete(key);
    }
    for (const key of this.order) {
      if (!this.byKey.has(key)) this.byKey.set(key, placeholderRow(key));
    }
    this.upsert(quotes);
  }

  /**
   * Merge a delta into rows that are already on the watch list.
   *
   * Unknown keys are deliberately ignored: option-chain legs and other
   * subscription groups stream through the same delta event, and appending them
   * here would silently grow the market watch with instruments the user never
   * added.
   */
  upsert(quotes: QuoteDto[]): void {
    for (const quote of quotes) {
      if (!this.byKey.has(quote.key)) continue;
      this.byKey.set(quote.key, toRow(quote));
    }
  }

  rows(): QuoteRow[] {
    const out: QuoteRow[] = [];
    for (const key of this.order) {
      const row = this.byKey.get(key);
      if (row) out.push(row);
    }
    return out;
  }

  get(key: string): QuoteRow | undefined {
    return this.byKey.get(key);
  }

  keys(): string[] {
    return [...this.order];
  }

  size(): number {
    return this.byKey.size;
  }

  clear(): void {
    this.byKey.clear();
    this.order = [];
  }
}
