import type { StockRowDto } from "../../api";

/** Match a stock by symbol or company name (case-insensitive). */
export function matchesStock(row: StockRowDto, query: string): boolean {
  const needle = query.trim().toUpperCase();
  if (needle === "") return true;
  return (
    row.symbol.toUpperCase().includes(needle) ||
    (row.name ?? "").toUpperCase().includes(needle)
  );
}

/**
 * Watched stocks first, in watch-list order; everything else alphabetically.
 * The caller passes the watch list so the order updates the moment a row is
 * added or removed, without waiting for a backend round-trip.
 */
export function sortStocks(rows: StockRowDto[], watched: string[]): StockRowDto[] {
  const order = new Map(watched.map((key, i) => [key, i]));
  return [...rows].sort((a, b) => {
    const ai = order.get(a.key);
    const bi = order.get(b.key);
    if (ai !== undefined && bi !== undefined) return ai - bi;
    if (ai !== undefined) return -1;
    if (bi !== undefined) return 1;
    return a.symbol.localeCompare(b.symbol) || a.key.localeCompare(b.key);
  });
}

/** Slice used by the virtual list; keeps the DOM to roughly one screen. */
export function visibleSlice<T>(
  rows: T[],
  scrollTop: number,
  viewportHeight: number,
  rowHeight: number,
): { start: number; end: number; top: number; bottom: number } {
  const height = Math.max(rowHeight, viewportHeight);
  const first = Math.max(0, Math.floor(Math.max(0, scrollTop) / rowHeight) - 4);
  const count = Math.ceil(height / rowHeight) + 8;
  const start = Math.min(first, Math.max(0, rows.length - 1));
  const end = Math.min(rows.length, start + count);
  return {
    start,
    end,
    top: start * rowHeight,
    bottom: Math.max(0, (rows.length - end) * rowHeight),
  };
}
