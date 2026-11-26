export const LIST_COLUMN_KEYS = [
  "protocol",
  "name",
  "ip",
  "note",
  "target",
] as const;

export type ListColumnKey = (typeof LIST_COLUMN_KEYS)[number];

export const LIST_COLUMN_LABELS: Record<ListColumnKey, string> = {
  protocol: "Protocol",
  name: "Name",
  ip: "IP",
  note: "Note",
  target: "Target",
};

export const DEFAULT_COLUMN_WIDTHS: Record<ListColumnKey, number> = {
  protocol: 118,
  name: 240,
  ip: 170,
  note: 260,
  target: 230,
};

/** Keeps only known columns, removes duplicates and appends anything missing. */
export function normalizeColumnOrder(order: readonly string[] | undefined): ListColumnKey[] {
  const known = new Set<ListColumnKey>(LIST_COLUMN_KEYS);
  const result: ListColumnKey[] = [];
  for (const key of order ?? []) {
    if (known.has(key as ListColumnKey) && !result.includes(key as ListColumnKey)) {
      result.push(key as ListColumnKey);
    }
  }
  for (const key of LIST_COLUMN_KEYS) {
    if (!result.includes(key)) {
      result.push(key);
    }
  }
  return result;
}

export function visibleColumns(
  order: readonly string[] | undefined,
  showNote: boolean,
): ListColumnKey[] {
  return normalizeColumnOrder(order).filter((key) => key !== "note" || showNote);
}

/**
 * The shared grid template for the list header and every server row. The Name
 * column grows only a little over its configured width so the address column
 * does not drift to the far right; Target absorbs the remaining space.
 */
export function listTemplate(
  order: readonly string[] | undefined,
  widths: Record<string, number>,
  showNote: boolean,
): string {
  const tracks = visibleColumns(order, showNote).map((key) => {
    const width = Math.max(80, Math.round(widths[key] ?? DEFAULT_COLUMN_WIDTHS[key]));
    if (key === "name") {
      return `minmax(${width}px, ${Math.round(width * 1.25)}px)`;
    }
    if (key === "target") {
      return `minmax(${width}px, 1fr)`;
    }
    return `${width}px`;
  });
  return `46px ${tracks.join(" ")} 62px`;
}

/** Moves `from` in front of `before`, returning a persisted-order array. */
export function moveColumnBefore(
  order: readonly string[],
  from: string,
  before: string,
): ListColumnKey[] {
  const normalized = normalizeColumnOrder(order);
  if (from === before) {
    return normalized;
  }
  const moved = normalized.filter((key) => key !== from);
  const index = moved.indexOf(before as ListColumnKey);
  moved.splice(index < 0 ? moved.length : index, 0, from as ListColumnKey);
  return moved;
}
