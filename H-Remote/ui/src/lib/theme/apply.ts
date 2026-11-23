import type { ResolvedTheme } from "../ipc/contracts";

/** Converts `#AARRGGBB` (reference app notation) to CSS `#RRGGBBAA`. */
export function toCssColor(value: string): string {
  const trimmed = value.trim();
  const hex = trimmed.startsWith("#") ? trimmed.slice(1) : "";
  if (!/^[0-9a-fA-F]+$/.test(hex)) {
    return "#000000";
  }
  if (hex.length === 6) {
    return `#${hex.toUpperCase()}`;
  }
  if (hex.length === 8) {
    const alpha = hex.slice(0, 2);
    const rgb = hex.slice(2);
    return alpha.toUpperCase() === "FF"
      ? `#${rgb.toUpperCase()}`
      : `#${rgb.toUpperCase()}${alpha.toUpperCase()}`;
  }
  return "#000000";
}

export function themeVariables(theme: ResolvedTheme): Record<string, string> {
  const size = Math.min(20, Math.max(10, Math.round(theme.fontSize)));
  return {
    "--hr-primary-mid": toCssColor(theme.colors.primaryMid),
    "--hr-primary-light": toCssColor(theme.colors.primaryLight),
    "--hr-primary-dark": toCssColor(theme.colors.primaryDark),
    "--hr-primary-text": toCssColor(theme.colors.primaryText),
    "--hr-accent-mid": toCssColor(theme.colors.accentMid),
    "--hr-accent-light": toCssColor(theme.colors.accentLight),
    "--hr-accent-dark": toCssColor(theme.colors.accentDark),
    "--hr-accent-text": toCssColor(theme.colors.accentText),
    "--hr-background": toCssColor(theme.colors.background),
    "--hr-background-text": toCssColor(theme.colors.backgroundText),
    "--hr-font-family": theme.fontFamily || "system-ui",
    "--hr-font-size-body": `${size}px`,
    "--hr-font-size-small": `${Math.max(10, size - 2)}px`,
    "--hr-font-size-subtitle": `${Math.min(20, size + 2)}px`,
    "--hr-font-size-title": `${Math.min(20, size + 6)}px`,
  };
}

export function isDarkTheme(theme: ResolvedTheme): boolean {
  const background = toCssColor(theme.colors.background);
  const value = Number.parseInt(background.slice(1, 7), 16);
  if (!Number.isFinite(value)) {
    return false;
  }
  const red = (value >> 16) & 0xff;
  const green = (value >> 8) & 0xff;
  const blue = value & 0xff;
  const luminance = (0.2126 * red + 0.7152 * green + 0.0722 * blue) / 255;
  return luminance < 0.5;
}

export function applyTheme(theme: ResolvedTheme, root: HTMLElement = document.documentElement): void {
  for (const [property, value] of Object.entries(themeVariables(theme))) {
    root.style.setProperty(property, value);
  }
  root.dataset.theme = theme.id;
  root.dataset.colorScheme = isDarkTheme(theme) ? "dark" : "light";
  root.style.colorScheme = isDarkTheme(theme) ? "dark" : "light";
}
