import type { ContainerRowDto } from "../../api";

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
