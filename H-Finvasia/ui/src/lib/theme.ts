export type ThemeId =
  | "midnight"
  | "graphite"
  | "ocean"
  | "contrast"
  | "light"
  | "nord"
  | "solarized"
  | "tokyo"
  | "forest"
  | "plum"
  | "sand";

export interface ThemeSpec {
  id: ThemeId;
  label: string;
  description: string;
  swatch: string;
}

export interface AccentSpec {
  id: string;
  label: string;
  accent: string;
  accent2: string;
}

export interface ThemeSelection {
  theme: ThemeId;
  accent: string;
}

export const THEMES: ThemeSpec[] = [
  { id: "midnight", label: "Midnight", description: "The default deep-blue terminal", swatch: "#16212d" },
  { id: "graphite", label: "Graphite", description: "Neutral dark grey", swatch: "#22262b" },
  { id: "ocean", label: "Ocean", description: "Deep navy with teal highlights", swatch: "#0b1d29" },
  { id: "contrast", label: "Contrast", description: "Black with high-contrast signals", swatch: "#000000" },
  { id: "light", label: "Light", description: "Bright paper theme", swatch: "#eef2f6" },
  { id: "nord", label: "Nord", description: "Cool arctic blue-grey", swatch: "#3b4252" },
  { id: "solarized", label: "Solarized", description: "Warm classic teal and sand", swatch: "#073642" },
  { id: "tokyo", label: "Tokyo Night", description: "Indigo night with neon signals", swatch: "#1a1b26" },
  { id: "forest", label: "Forest", description: "Deep green, calm contrast", swatch: "#102218" },
  { id: "plum", label: "Plum", description: "Dark violet with rose accents", swatch: "#241a33" },
  { id: "sand", label: "Sand", description: "Warm light paper theme", swatch: "#f0e8d8" },
];

export const ACCENTS: AccentSpec[] = [
  { id: "blue", label: "Blue", accent: "#2A6EA8", accent2: "#4B9BD8" },
  { id: "teal", label: "Teal", accent: "#0F7F8F", accent2: "#25B3C2" },
  { id: "green", label: "Green", accent: "#1F8A54", accent2: "#3DC77E" },
  { id: "amber", label: "Amber", accent: "#A56B00", accent2: "#D9A441" },
  { id: "violet", label: "Violet", accent: "#7B3FA0", accent2: "#B05BD8" },
  { id: "red", label: "Red", accent: "#A63B3B", accent2: "#E25C5C" },
];

const STORAGE_KEY = "fv.theme";
const HEX = /^#[0-9a-fA-F]{6}$/;

export function isThemeId(value: string | null | undefined): value is ThemeId {
  return THEMES.some((theme) => theme.id === value);
}

export function normalizeAccent(value: string | null | undefined): string {
  const trimmed = (value ?? "").trim();
  return HEX.test(trimmed) ? trimmed.toUpperCase() : ACCENTS[0].accent;
}

/** Mix toward white. Used for the lighter accent hover colour. */
export function lighten(hex: string, amount = 0.18): string {
  const normalized = normalizeAccent(hex).slice(1);
  const channels = [0, 2, 4].map((offset) => {
    const value = parseInt(normalized.slice(offset, offset + 2), 16);
    return Math.round(value + (255 - value) * amount);
  });
  return `#${channels.map((value) => value.toString(16).padStart(2, "0")).join("")}`.toUpperCase();
}

export function loadTheme(): ThemeSelection {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw) as Partial<ThemeSelection>;
      return {
        theme: isThemeId(parsed.theme) ? parsed.theme : "midnight",
        accent: normalizeAccent(parsed.accent),
      };
    }
  } catch {
    // Corrupt storage should never stop the app from loading.
  }
  return { theme: "midnight", accent: ACCENTS[0].accent };
}

export function applyTheme(selection: ThemeSelection): void {
  if (typeof document === "undefined") return;
  const theme = isThemeId(selection.theme) ? selection.theme : "midnight";
  const accent = normalizeAccent(selection.accent);
  const root = document.documentElement;
  root.dataset.theme = theme;
  root.style.setProperty("--accent", accent);
  root.style.setProperty("--accent-2", lighten(accent));
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ theme, accent }));
  } catch {
    // Apply still works when storage is unavailable.
  }
}

/** Called before mounting so the first paint already has the chosen theme. */
export function applyStoredTheme(): ThemeSelection {
  const selection = loadTheme();
  applyTheme(selection);
  return selection;
}
