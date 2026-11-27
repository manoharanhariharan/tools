import { describe, expect, it } from "vitest";
import { isBlankChainTab, normaliseBlankChainTabs, type ChainTabLike } from "./chainTabs";

function tab(id: string, overrides: Partial<ChainTabLike> = {}): ChainTabLike {
  return { id, loaded: false, pinned: false, pending: null, ...overrides };
}

describe("chain tab blank normalisation", () => {
  it("creates one blank tab when none exists", () => {
    const tabs = normaliseBlankChainTabs([tab("a", { pinned: true, loaded: true })], () => tab("new"));
    expect(tabs.map((item) => item.id)).toEqual(["a", "new"]);
  });

  it("keeps a single existing blank tab", () => {
    const tabs = normaliseBlankChainTabs([tab("a", { pinned: true }), tab("blank")], () => tab("new"));
    expect(tabs.map((item) => item.id)).toEqual(["a", "blank"]);
  });

  it("removes duplicate blanks after a pin", () => {
    const tabs = normaliseBlankChainTabs([tab("a", { pinned: true }), tab("b1"), tab("b2")], () =>
      tab("new"),
    );
    expect(tabs.map((item) => item.id)).toEqual(["a", "b1"]);
  });

  it("does not treat a loading tab as reusable", () => {
    expect(isBlankChainTab(tab("a", { pending: { underlying: "NIFTY" } }))).toBe(false);
  });
});
