import { describe, expect, it } from "vitest";
import {
  availableLayouts,
  autoLayout,
  isLayoutValid,
  resolveLayout,
} from "./layouts";

function cells(layout: ReturnType<typeof autoLayout>): number {
  return layout.cells.length;
}

describe("session grid layouts", () => {
  it("auto layout balances every count from 1 to 12", () => {
    for (let count = 1; count <= 12; count += 1) {
      const layout = autoLayout(count);
      expect(cells(layout)).toBe(count);
      expect(layout.rows * layout.columns).toBeGreaterThanOrEqual(count);
      expect(isLayoutValid(layout)).toBe(true);
    }
    expect(autoLayout(2).columns).toBe(2);
    expect(autoLayout(4)).toMatchObject({ columns: 2, rows: 2 });
    expect(autoLayout(9)).toMatchObject({ columns: 3, rows: 3 });
  });

  it("linear layouts produce N columns or rows", () => {
    const cols = resolveLayout("cols", 6);
    expect(cols).toMatchObject({ columns: 6, rows: 1 });
    expect(isLayoutValid(cols)).toBe(true);
    const rows = resolveLayout("rows", 5);
    expect(rows).toMatchObject({ columns: 1, rows: 5 });
    expect(isLayoutValid(rows)).toBe(true);
  });

  it("every offered preset fits the session count without overlaps", () => {
    for (let count = 2; count <= 9; count += 1) {
      for (const option of availableLayouts(count)) {
        const layout = resolveLayout(option.id, count);
        expect(layout.cells).toHaveLength(count);
        expect(isLayoutValid(layout), `${count}:${option.id}`).toBe(true);
        for (const cell of layout.cells) {
          expect(cell.column + cell.spanX).toBeLessThanOrEqual(layout.columns);
          expect(cell.row + cell.spanY).toBeLessThanOrEqual(layout.rows);
        }
      }
    }
  });

  it("falls back to automatic when a preset does not match the count", () => {
    const layout = resolveLayout("4-left", 6);
    expect(layout.id).toBe("auto");
    expect(layout.cells).toHaveLength(6);
    expect(isLayoutValid(resolveLayout("does-not-exist", 3))).toBe(true);
  });

  it("never hides a session for any tab count", () => {
    for (let count = 2; count <= 12; count += 1) {
      const options = availableLayouts(count);
      expect(options.some((option) => option.id === "auto")).toBe(true);
      for (const option of options) {
        expect(resolveLayout(option.id, count).cells.length).toBe(count);
      }
    }
  });

  it("supports the wide and spanning presets", () => {
    const wide = resolveLayout("2-wide-left", 2);
    expect(wide.cells[0]).toMatchObject({ column: 0, spanX: 2 });
    const left = resolveLayout("4-left", 4);
    expect(left.cells[0]).toMatchObject({ column: 0, row: 0, spanY: 3 });
    const top = resolveLayout("3-top", 3);
    expect(top.cells[0]).toMatchObject({ spanX: 2 });
    expect(isLayoutValid(top)).toBe(true);
  });
});
