/**
 * Grid/snap layouts for the session workspace.
 *
 * Layouts are pure data: a column/row count plus one cell per session. Cells may
 * span multiple tracks (e.g. "1 left + 3 right"), which the workspace maps to
 * CSS grid line placement. Every layout is validated against the session count
 * before use; a mismatch falls back to the automatic layout so no session is
 * ever hidden.
 */

export interface GridCell {
  column: number;
  row: number;
  spanX: number;
  spanY: number;
}

export interface GridLayout {
  id: string;
  columns: number;
  rows: number;
  cells: GridCell[];
}

export interface LayoutOption {
  id: string;
  label: string;
}

type CellTuple = [column: number, row: number, spanX: number, spanY: number];

interface Preset {
  id: string;
  label: string;
  columns: number;
  rows: number;
  cells: CellTuple[];
}

function preset(
  id: string,
  label: string,
  columns: number,
  rows: number,
  cells: CellTuple[],
): Preset {
  return { id, label, columns, rows, cells };
}

const PRESETS: Record<number, Preset[]> = {
  2: [
    preset("2-cols", "Side by side", 2, 1, [
      [0, 0, 1, 1],
      [1, 0, 1, 1],
    ]),
    preset("2-rows", "Stacked", 1, 2, [
      [0, 0, 1, 1],
      [0, 1, 1, 1],
    ]),
    preset("2-wide-left", "Wide left", 3, 1, [
      [0, 0, 2, 1],
      [2, 0, 1, 1],
    ]),
    preset("2-wide-right", "Wide right", 3, 1, [
      [0, 0, 1, 1],
      [1, 0, 2, 1],
    ]),
  ],
  3: [
    preset("3-cols", "Three columns", 3, 1, [
      [0, 0, 1, 1],
      [1, 0, 1, 1],
      [2, 0, 1, 1],
    ]),
    preset("3-left", "One left, two right", 2, 2, [
      [0, 0, 1, 2],
      [1, 0, 1, 1],
      [1, 1, 1, 1],
    ]),
    preset("3-right", "Two left, one right", 2, 2, [
      [0, 0, 1, 1],
      [0, 1, 1, 1],
      [1, 0, 1, 2],
    ]),
    preset("3-top", "One top, two bottom", 2, 2, [
      [0, 0, 2, 1],
      [0, 1, 1, 1],
      [1, 1, 1, 1],
    ]),
    preset("3-bottom", "Two top, one bottom", 2, 2, [
      [0, 0, 1, 1],
      [1, 0, 1, 1],
      [0, 1, 2, 1],
    ]),
  ],
  4: [
    preset("4-grid", "2 × 2", 2, 2, [
      [0, 0, 1, 1],
      [1, 0, 1, 1],
      [0, 1, 1, 1],
      [1, 1, 1, 1],
    ]),
    preset("4-cols", "Four columns", 4, 1, [
      [0, 0, 1, 1],
      [1, 0, 1, 1],
      [2, 0, 1, 1],
      [3, 0, 1, 1],
    ]),
    preset("4-left", "One left, three right", 2, 3, [
      [0, 0, 1, 3],
      [1, 0, 1, 1],
      [1, 1, 1, 1],
      [1, 2, 1, 1],
    ]),
    preset("4-right", "Three left, one right", 2, 3, [
      [0, 0, 1, 1],
      [0, 1, 1, 1],
      [0, 2, 1, 1],
      [1, 0, 1, 3],
    ]),
    preset("4-top", "One top, three bottom", 3, 2, [
      [0, 0, 3, 1],
      [0, 1, 1, 1],
      [1, 1, 1, 1],
      [2, 1, 1, 1],
    ]),
    preset("4-bottom", "Three top, one bottom", 3, 2, [
      [0, 0, 1, 1],
      [1, 0, 1, 1],
      [2, 0, 1, 1],
      [0, 1, 3, 1],
    ]),
  ],
  5: [
    preset("5-cols", "Five columns", 5, 1, [
      [0, 0, 1, 1],
      [1, 0, 1, 1],
      [2, 0, 1, 1],
      [3, 0, 1, 1],
      [4, 0, 1, 1],
    ]),
    preset("5-rows", "Five rows", 1, 5, [
      [0, 0, 1, 1],
      [0, 1, 1, 1],
      [0, 2, 1, 1],
      [0, 3, 1, 1],
      [0, 4, 1, 1],
    ]),
    preset("5-3-2", "Three top, two bottom", 3, 2, [
      [0, 0, 1, 1],
      [1, 0, 1, 1],
      [2, 0, 1, 1],
      [0, 1, 1, 1],
      [1, 1, 2, 1],
    ]),
    preset("5-2-3", "Two top, three bottom", 3, 2, [
      [0, 0, 1, 1],
      [1, 0, 2, 1],
      [0, 1, 1, 1],
      [1, 1, 1, 1],
      [2, 1, 1, 1],
    ]),
  ],
  6: [
    preset("6-grid", "3 × 2", 3, 2, [
      [0, 0, 1, 1],
      [1, 0, 1, 1],
      [2, 0, 1, 1],
      [0, 1, 1, 1],
      [1, 1, 1, 1],
      [2, 1, 1, 1],
    ]),
    preset("6-cols", "Six columns", 6, 1, [
      [0, 0, 1, 1],
      [1, 0, 1, 1],
      [2, 0, 1, 1],
      [3, 0, 1, 1],
      [4, 0, 1, 1],
      [5, 0, 1, 1],
    ]),
    preset("6-grid-2", "2 × 3", 2, 3, [
      [0, 0, 1, 1],
      [1, 0, 1, 1],
      [0, 1, 1, 1],
      [1, 1, 1, 1],
      [0, 2, 1, 1],
      [1, 2, 1, 1],
    ]),
  ],
  8: [
    preset("8-grid", "4 × 2", 4, 2, [
      [0, 0, 1, 1],
      [1, 0, 1, 1],
      [2, 0, 1, 1],
      [3, 0, 1, 1],
      [0, 1, 1, 1],
      [1, 1, 1, 1],
      [2, 1, 1, 1],
      [3, 1, 1, 1],
    ]),
    preset("8-rows", "2 × 4", 2, 4, [
      [0, 0, 1, 1],
      [1, 0, 1, 1],
      [0, 1, 1, 1],
      [1, 1, 1, 1],
      [0, 2, 1, 1],
      [1, 2, 1, 1],
      [0, 3, 1, 1],
      [1, 3, 1, 1],
    ]),
  ],
  9: [
    preset("9-grid", "3 × 3", 3, 3, [
      [0, 0, 1, 1],
      [1, 0, 1, 1],
      [2, 0, 1, 1],
      [0, 1, 1, 1],
      [1, 1, 1, 1],
      [2, 1, 1, 1],
      [0, 2, 1, 1],
      [1, 2, 1, 1],
      [2, 2, 1, 1],
    ]),
    preset("9-cols", "Nine columns", 9, 1, [
      [0, 0, 1, 1],
      [1, 0, 1, 1],
      [2, 0, 1, 1],
      [3, 0, 1, 1],
      [4, 0, 1, 1],
      [5, 0, 1, 1],
      [6, 0, 1, 1],
      [7, 0, 1, 1],
      [8, 0, 1, 1],
    ]),
  ],
};

export function availableLayouts(count: number): LayoutOption[] {
  if (count < 2) {
    return [{ id: "auto", label: "Automatic" }];
  }
  return [
    { id: "auto", label: "Automatic" },
    { id: "cols", label: `${count} columns` },
    { id: "rows", label: `${count} rows` },
    ...(PRESETS[count] ?? []).map(({ id, label }) => ({ id, label })),
  ];
}

export function resolveLayout(layoutId: string, count: number): GridLayout {
  if (layoutId === "cols" || layoutId === "rows") {
    return linearLayout(layoutId, count);
  }
  const preset = PRESETS[count]?.find((candidate) => candidate.id === layoutId);
  if (preset && preset.cells.length === count) {
    return toLayout(preset);
  }
  return autoLayout(count);
}

export function autoLayout(count: number): GridLayout {
  const safeCount = Math.max(1, Math.floor(count));
  const columns = Math.max(1, Math.ceil(Math.sqrt(safeCount)));
  const rows = Math.max(1, Math.ceil(safeCount / columns));
  return {
    id: "auto",
    columns,
    rows,
    cells: Array.from({ length: safeCount }, (_, index) => ({
      column: index % columns,
      row: Math.floor(index / columns),
      spanX: 1,
      spanY: 1,
    })),
  };
}

function linearLayout(direction: "cols" | "rows", count: number): GridLayout {
  const safeCount = Math.max(1, Math.floor(count));
  const columns = direction === "cols" ? safeCount : 1;
  const rows = direction === "rows" ? safeCount : 1;
  return {
    id: direction,
    columns,
    rows,
    cells: Array.from({ length: safeCount }, (_, index) => ({
      column: direction === "cols" ? index : 0,
      row: direction === "rows" ? index : 0,
      spanX: 1,
      spanY: 1,
    })),
  };
}

function toLayout(preset: Preset): GridLayout {
  return {
    id: preset.id,
    columns: preset.columns,
    rows: preset.rows,
    cells: preset.cells.map(([column, row, spanX, spanY]) => ({
      column,
      row,
      spanX,
      spanY,
    })),
  };
}

/** Validates that every cell fits and no two cells overlap. */
export function isLayoutValid(layout: GridLayout): boolean {
  const occupied = new Set<string>();
  for (const cell of layout.cells) {
    if (
      cell.column < 0 ||
      cell.row < 0 ||
      cell.spanX < 1 ||
      cell.spanY < 1 ||
      cell.column + cell.spanX > layout.columns ||
      cell.row + cell.spanY > layout.rows
    ) {
      return false;
    }
    for (let x = cell.column; x < cell.column + cell.spanX; x += 1) {
      for (let y = cell.row; y < cell.row + cell.spanY; y += 1) {
        const key = `${x}:${y}`;
        if (occupied.has(key)) {
          return false;
        }
        occupied.add(key);
      }
    }
  }
  return true;
}
