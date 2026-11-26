/**
 * Small, pure-ish UI preference helpers.
 *
 * They accept an explicit storage object so the parsing and clamping logic is
 * testable under Node without a DOM. `localStorage` failures (hardened webview,
 * private mode) are always non-fatal.
 */

export interface StorageLike {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

export type StocksView = "list" | "cards";
export type SortMode = "name" | "price" | "added" | "custom";

export const HEADER_KEY = "fv.headerCollapsed";
export const STOCKS_VIEW_KEY = "fv.stocksView";
export const STOCKS_SORT_KEY = "fv.stocksSort";
export const COLLAPSED_GROUPS_KEY = "fv.groupCards.collapsed";

function defaultStorage(): StorageLike | null {
  try {
    return typeof localStorage === "undefined" ? null : localStorage;
  } catch {
    return null;
  }
}

function readText(storage: StorageLike | null, key: string): string | null {
  if (!storage) return null;
  try {
    return storage.getItem(key);
  } catch {
    return null;
  }
}

function writeText(storage: StorageLike | null, key: string, value: string): void {
  if (!storage) return;
  try {
    storage.setItem(key, value);
  } catch {
    // Preference persistence must never break the workspace.
  }
}

export function loadHeaderCollapsed(storage: StorageLike | null = defaultStorage()): boolean {
  return readText(storage, HEADER_KEY) === "1";
}

export function saveHeaderCollapsed(
  collapsed: boolean,
  storage: StorageLike | null = defaultStorage(),
): void {
  writeText(storage, HEADER_KEY, collapsed ? "1" : "0");
}

export function loadStocksView(storage: StorageLike | null = defaultStorage()): StocksView {
  return readText(storage, STOCKS_VIEW_KEY) === "cards" ? "cards" : "list";
}

export function saveStocksView(
  view: StocksView,
  storage: StorageLike | null = defaultStorage(),
): void {
  writeText(storage, STOCKS_VIEW_KEY, view);
}

const SORT_MODES: readonly SortMode[] = ["name", "price", "added", "custom"];

export function loadSortMode(storage: StorageLike | null = defaultStorage()): SortMode {
  const raw = readText(storage, STOCKS_SORT_KEY);
  return SORT_MODES.includes(raw as SortMode) ? (raw as SortMode) : "name";
}

export function saveSortMode(
  mode: SortMode,
  storage: StorageLike | null = defaultStorage(),
): void {
  writeText(storage, STOCKS_SORT_KEY, mode);
}

/** Collapsed state per group name. Unknown values are ignored, not coerced. */
export function loadCollapsedGroups(
  storage: StorageLike | null = defaultStorage(),
): Record<string, boolean> {
  const raw = readText(storage, COLLAPSED_GROUPS_KEY);
  if (!raw) return {};
  try {
    const parsed = JSON.parse(raw) as unknown;
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return {};
    const out: Record<string, boolean> = {};
    for (const [name, value] of Object.entries(parsed as Record<string, unknown>)) {
      if (typeof value === "boolean") out[name] = value;
    }
    return out;
  } catch {
    return {};
  }
}

export function saveCollapsedGroups(
  collapsed: Record<string, boolean>,
  storage: StorageLike | null = defaultStorage(),
): void {
  writeText(storage, COLLAPSED_GROUPS_KEY, JSON.stringify(collapsed));
}
