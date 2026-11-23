import type { KeywordMatchConfig, MatcherKind } from "../ipc/contracts";

/**
 * Matchers used for the next search (P10-03). Direct matching is always first;
 * Chinese locales add pinyin providers when the user only selected Direct and
 * locale-aware defaults are enabled.
 */
export function effectiveMatchers(
  config: KeywordMatchConfig | null | undefined,
  language: string,
): MatcherKind[] {
  const matchers: MatcherKind[] = config?.matchers?.length
    ? [...config.matchers]
    : ["direct"];
  if (!matchers.includes("direct")) {
    matchers.unshift("direct");
  }
  const onlyDirect = matchers.every((matcher) => matcher === "direct");
  if (
    (config?.autoLocale ?? true) &&
    onlyDirect &&
    language.toLowerCase().startsWith("zh")
  ) {
    return [...matchers, "pinyin", "pinyinInitials"];
  }
  return matchers;
}
