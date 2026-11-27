import type { StorageLike } from "../layoutPrefs";

export interface ColumnMeta {
  id: string;
  label: string;
  width: number;
  min?: number;
  max?: number;
}

export interface ColumnLayout {
  widths: Record<string, number>;
  hidden: string[];
}

const MIN_WIDTH = 36;

function defaultStorage(): StorageLike | null {
  try {
    return typeof localStorage === "undefined" ? null : localStorage;
  } catch {
    return null;
  }
}

function clamp(value: number, low: number, high: number): number {
  return Math.max(low, Math.min(high, value));
}

/** Load saved widths/visibility, ignoring unknown columns and bad values. */
export function loadColumnLayout(
  key: string,
  columns: readonly ColumnMeta[],
  storage: StorageLike | null = defaultStorage(),
): ColumnLayout {
  const defaults: ColumnLayout = {
    widths: Object.fromEntries(columns.map((column) => [column.id, column.width])),
    hidden: [],
  };
  if (!storage) return defaults;
  let raw: string | null;
  try {
    raw = storage.getItem(key);
  } catch {
    return defaults;
  }
  if (!raw) return defaults;
  try {
    const parsed = JSON.parse(raw) as Partial<ColumnLayout>;
    const widths = { ...defaults.widths };
    if (parsed.widths && typeof parsed.widths === "object") {
      for (const column of columns) {
        const value = (parsed.widths as Record<string, unknown>)[column.id];
        if (typeof value === "number" && Number.isFinite(value)) {
          widths[column.id] = clamp(value, column.min ?? MIN_WIDTH, column.max ?? 640);
        }
      }
    }
    const known = new Set(columns.map((column) => column.id));
    const hidden = Array.isArray(parsed.hidden)
      ? parsed.hidden.filter((id): id is string => typeof id === "string" && known.has(id))
      : [];
    // Never persist a layout that hides every column.
    return { widths, hidden: hidden.length >= columns.length ? [] : hidden };
  } catch {
    return defaults;
  }
}

export function saveColumnLayout(
  key: string,
  layout: ColumnLayout,
  storage: StorageLike | null = defaultStorage(),
): void {
  if (!storage) return;
  try {
    storage.setItem(key, JSON.stringify(layout));
  } catch {
    // Preference persistence must never break the dock.
  }
}
