import { describe, expect, it } from "vitest";
import type { KeywordMatchConfig, MatcherKind } from "../ipc/contracts";
import { effectiveMatchers } from "./matchers";

function config(matchers: MatcherKind[], autoLocale = true): KeywordMatchConfig {
  return { matchers, debounceMs: 150, parallel: true, autoLocale };
}

describe("effective matchers", () => {
  it("adds pinyin providers for Chinese locales when only direct is selected", () => {
    expect(effectiveMatchers(config(["direct"]), "zh-CN")).toEqual([
      "direct",
      "pinyin",
      "pinyinInitials",
    ]);
  });

  it("keeps explicit selections and non-Chinese locales unchanged", () => {
    expect(effectiveMatchers(config(["direct", "initials"]), "zh-CN")).toEqual([
      "direct",
      "initials",
    ]);
    expect(effectiveMatchers(config(["direct"]), "en-US")).toEqual(["direct"]);
  });

  it("respects the auto-locale opt-out and always keeps direct first", () => {
    expect(effectiveMatchers(config(["direct"], false), "zh-TW")).toEqual(["direct"]);
    expect(effectiveMatchers(config(["initials"]), "en-US")).toEqual(["direct", "initials"]);
  });
});
