import { describe, expect, it } from "vitest";
import {
  LIST_COLUMN_KEYS,
  listTemplate,
  moveColumnBefore,
  normalizeColumnOrder,
} from "./columns";

describe("catalog column order", () => {
  it("keeps known columns, removes duplicates and appends missing ones", () => {
    expect(normalizeColumnOrder(undefined)).toEqual([...LIST_COLUMN_KEYS]);
    expect(normalizeColumnOrder(["ip", "name", "ip", "nope"])).toEqual([
      "ip",
      "name",
      "protocol",
      "note",
      "target",
    ]);
  });

  it("moves a column in front of another", () => {
    expect(moveColumnBefore([...LIST_COLUMN_KEYS], "ip", "protocol")).toEqual([
      "ip",
      "protocol",
      "name",
      "note",
      "target",
    ]);
    expect(moveColumnBefore([...LIST_COLUMN_KEYS], "name", "target")).toEqual([
      "protocol",
      "ip",
      "note",
      "name",
      "target",
    ]);
  });

  it("builds the same template order as the header and hides note on demand", () => {
    const widths = { protocol: 100, name: 200, ip: 150, note: 260, target: 220 };
    expect(listTemplate(["ip", "protocol", "name", "target"], widths, false)).toBe(
      "46px 150px 100px minmax(200px, 250px) minmax(220px, 1fr) 62px",
    );
    expect(listTemplate(["protocol", "name", "ip", "note", "target"], widths, true)).toBe(
      "46px 100px minmax(200px, 250px) 150px 260px minmax(220px, 1fr) 62px",
    );
  });
});
