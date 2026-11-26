import type { ContainerRowDto } from "../../api";
import type { SortMode } from "../../layoutPrefs";

/**
 * Adopt the backend card order unless a drag is in progress. Using the
 * backend order directly is what keeps an inline rename in the same slot.
 */
export function nextCardOrder(
  current: readonly string[],
  names: readonly string[],
  dragging = false,
): string[] {
  return dragging ? [...current] : [...names];
}

/** Keep the saved order, append groups the file did not know about yet. */
export function mergeOrder(order: readonly string[], names: readonly string[]): string[] {
  const present = new Set(names);
  const out: string[] = [];
  const seen = new Set<string>();
  for (const name of order) {
    if (present.has(name) && !seen.has(name)) {
      out.push(name);
      seen.add(name);
    }
  }
  for (const name of names) {
    if (!seen.has(name)) {
      out.push(name);
      seen.add(name);
    }
  }
  return out;
}

/** Move `dragged` to the position currently occupied by `target`. */
export function moveRelative(
  order: readonly string[],
  dragged: string,
  target: string,
): string[] {
  const from = order.indexOf(dragged);
  const to = order.indexOf(target);
  if (from < 0 || to < 0 || from === to) return [...order];
  const next = [...order];
  const [item] = next.splice(from, 1);
  next.splice(to, 0, item);
  return next;
}

export interface SortableInstrument {
  key: string;
  symbol: string;
  quote: { ltp: { e6: number } | null } | null;
}

function orderMap(order: readonly string[]): Map<string, number> {
  return new Map(order.map((key, index) => [key, index]));
}

/**
 * Apply a display sort without mutating the stored group order.
 * - `added` follows the group's insertion order.
 * - `custom` follows the user's saved per-group order, falling back to insertion.
 * - `price` is highest LTP first; rows without a quote come last.
 */
export function sortInstrumentRows<T extends SortableInstrument>(
  rows: readonly T[],
  mode: SortMode,
  addedOrder: readonly string[] = rows.map((row) => row.key),
  customOrder: readonly string[] = [],
): T[] {
  const out = [...rows];
  if (mode === "name") {
    return out.sort(
      (a, b) => a.symbol.localeCompare(b.symbol, "en-IN") || a.key.localeCompare(b.key),
    );
  }
  if (mode === "price") {
    return out.sort((a, b) => {
      const left = a.quote?.ltp?.e6;
      const right = b.quote?.ltp?.e6;
      if (left === undefined && right === undefined) {
        return a.symbol.localeCompare(b.symbol, "en-IN") || a.key.localeCompare(b.key);
      }
      if (left === undefined) return 1;
      if (right === undefined) return -1;
      return right - left || a.symbol.localeCompare(b.symbol, "en-IN") || a.key.localeCompare(b.key);
    });
  }
  const fallback = orderMap(addedOrder);
  if (mode === "custom") {
    const rank = orderMap(customOrder);
    return out.sort((a, b) => {
      const ar = rank.get(a.key);
      const br = rank.get(b.key);
      if (ar === undefined && br === undefined) {
        return (
          (fallback.get(a.key) ?? Number.MAX_SAFE_INTEGER) -
            (fallback.get(b.key) ?? Number.MAX_SAFE_INTEGER) ||
          a.key.localeCompare(b.key)
        );
      }
      if (ar === undefined) return 1;
      if (br === undefined) return -1;
      return ar - br || a.key.localeCompare(b.key);
    });
  }
  return out.sort((a, b) => {
    const ar = fallback.get(a.key) ?? Number.MAX_SAFE_INTEGER;
    const br = fallback.get(b.key) ?? Number.MAX_SAFE_INTEGER;
    return ar - br || a.key.localeCompare(b.key);
  });
}

export type InstrumentKind = "IDX" | "FUT" | "CE" | "PE" | "EQ" | "INS";

export function instrumentKind(
  row: Pick<ContainerRowDto, "is_index" | "instrument_type" | "strike_type">,
): InstrumentKind {
  if (row.is_index) return "IDX";
  const type = row.instrument_type.trim().toUpperCase();
  if (type === "FUT" || type.startsWith("FUT")) return "FUT";
  if (row.strike_type === "CE") return "CE";
  if (row.strike_type === "PE") return "PE";
  if (type === "EQ" || type === "INDEX") return "EQ";
  return "INS";
}

/** Compact secondary line: expiry and strike for derivatives, name for equities. */
export function instrumentDetail(
  row: Pick<
    ContainerRowDto,
    "is_index" | "instrument_type" | "name" | "underlying" | "expiry" | "strike_type" | "strike"
  >,
): string {
  const kind = instrumentKind(row);
  if (kind === "CE" || kind === "PE") {
    return [row.underlying, row.expiry, row.strike].filter(Boolean).join(" | ");
  }
  if (kind === "FUT") {
    return [row.underlying, row.expiry].filter(Boolean).join(" | ");
  }
  return row.name ?? row.underlying ?? "";
}
