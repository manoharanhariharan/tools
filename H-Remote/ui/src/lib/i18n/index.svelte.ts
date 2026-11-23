import { FluentBundle, FluentResource } from "@fluent/bundle";
import { catalogs } from "./catalogs";

type FormatArgs = Record<string, string | number>;

const resources = new Map<string, string>(Object.entries(catalogs));

const bundles = new Map<string, FluentBundle>();
const missing = new Set<string>();

function canonical(id: string): string | null {
  for (const candidate of resources.keys()) {
    if (candidate.toLowerCase() === id.trim().toLowerCase()) {
      return candidate;
    }
  }
  return null;
}

/** Mirrors the Rust negotiation: exact, then language, then English. */
export function negotiateLocale(requested: string | null | undefined): string {
  if (requested && requested !== "system") {
    const exact = canonical(requested);
    if (exact) {
      return exact;
    }
    const language = requested.split("-")[0]?.toLowerCase();
    if (language === "zh") {
      return /hant|tw|hk/i.test(requested) ? "zh-TW" : "zh-CN";
    }
    for (const candidate of resources.keys()) {
      if (candidate.split("-")[0]?.toLowerCase() === language) {
        return candidate;
      }
    }
  }
  return "en-US";
}

function bundleFor(locale: string): FluentBundle | null {
  const existing = bundles.get(locale);
  if (existing) {
    return existing;
  }
  const source = resources.get(locale);
  if (!source) {
    return null;
  }
  try {
    const resource = new FluentResource(source);
    const bundle = new FluentBundle(locale, { useIsolating: false });
    const errors = bundle.addResource(resource);
    if (errors.length > 0) {
      console.warn(`locale ${locale} has Fluent errors`, errors);
    }
    bundles.set(locale, bundle);
    return bundle;
  } catch (error) {
    console.error(`cannot load locale ${locale}`, error);
    return null;
  }
}

class I18nStore {
  locale = $state("en-US");
  readonly catalogs = $derived(Array.from(resources.keys()));

  setLocale(locale: string): void {
    this.locale = negotiateLocale(locale);
    if (typeof document !== "undefined") {
      document.documentElement.lang = this.locale;
    }
  }

  t(key: string, args?: FormatArgs): string {
    // Read the reactive locale so components re-render on change.
    const locale = this.locale;
    const fromActive = this.format(locale, key, args);
    if (fromActive !== null) {
      return fromActive;
    }
    if (locale !== "en-US") {
      const fallback = this.format("en-US", key, args);
      if (fallback !== null) {
        return fallback;
      }
    }
    if (!missing.has(key)) {
      missing.add(key);
      console.warn(`missing translation key: ${key}`);
    }
    return key;
  }

  t_args(key: string, args: FormatArgs): string {
    return this.t(key, args);
  }

  has(key: string, locale = this.locale): boolean {
    return this.format(locale, key) !== null;
  }
  missingKeys(): string[] {
    return [...missing];
  }

  private format(locale: string, key: string, args?: FormatArgs): string | null {
    const bundle = bundleFor(locale);
    const message = bundle?.getMessage(fluentId(key));
    const pattern = message?.value;
    if (!bundle || !pattern) {
      return null;
    }
    try {
      return bundle.formatPattern(pattern, args ?? {});
    } catch (error) {
      console.warn(`cannot format ${key} in ${locale}`, error);
      return null;
    }
  }
}

function fluentId(key: string): string {
  return key.replaceAll(".", "-");
}

export const i18n = new I18nStore();
