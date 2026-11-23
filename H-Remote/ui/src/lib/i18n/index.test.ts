import { describe, expect, it } from "vitest";
import { i18n, negotiateLocale } from "./index.svelte";

describe("localisation", () => {
  it("ships all fourteen catalogs", () => {
    expect(i18n.catalogs).toHaveLength(14);
  });

  it("negotiates exact, language and script matches", () => {
    expect(negotiateLocale("de-DE")).toBe("de-DE");
    expect(negotiateLocale("de-AT")).toBe("de-DE");
    expect(negotiateLocale("pt-BR")).toBe("pt-BR");
    expect(negotiateLocale("zh-Hans-CN")).toBe("zh-CN");
    expect(negotiateLocale("zh-Hant-HK")).toBe("zh-TW");
    expect(negotiateLocale("xx-YY")).toBe("en-US");
    expect(negotiateLocale("system")).toBe("en-US");
  });

  it("falls back to English for scaffolded locales", () => {
    i18n.setLocale("de-DE");
    expect(i18n.locale).toBe("de-DE");
    expect(i18n.t("common.cancel")).toBe("Cancel");
    expect(i18n.has("common.cancel", "de-DE")).toBe(false);
    expect(i18n.has("common.cancel", "en-US")).toBe(true);
  });

  it("interpolates Fluent arguments", () => {
    i18n.setLocale("en-US");
    expect(i18n.t_args("common.seconds", { count: 5 })).toBe("5 s");
    expect(
      i18n.t_args("profile.notWritable", { path: "/x", message: "denied" }),
    ).toBe("/x is not writable: denied");
  });

  it("returns and records genuinely missing keys", () => {
    i18n.setLocale("en-US");
    expect(i18n.t("does.not.exist")).toBe("does.not.exist");
    expect(i18n.missingKeys()).toContain("does.not.exist");
  });

  it("updates the document language when switching", () => {
    i18n.setLocale("fr-FR");
    expect(document.documentElement.lang).toBe("fr-FR");
    i18n.setLocale("en-US");
  });
});
