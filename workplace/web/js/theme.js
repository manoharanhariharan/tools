// Appearance preference. The chosen theme is stored per device so a phone and
// a desktop can look different; the inline-safe boot script applies it before
// the stylesheet paints to avoid a flash.

export const THEMES = [
  { value: 'system', label: 'System' },
  { value: 'light', label: 'Light' },
  { value: 'dark', label: 'Dark' },
  { value: 'contrast', label: 'High contrast' },
];

export const THEME_STORAGE_KEY = 'daybook_theme';

const VALID = new Set(THEMES.map((theme) => theme.value));

/** Coerce arbitrary stored input to a supported theme. */
export function normalizeTheme(value) {
  return VALID.has(value) ? value : 'system';
}

/** Read the saved preference, falling back to the device scheme. */
export function readStoredTheme() {
  try {
    return normalizeTheme(localStorage.getItem(THEME_STORAGE_KEY));
  } catch {
    return 'system';
  }
}

function systemPrefersDark() {
  try {
    return window.matchMedia('(prefers-color-scheme: dark)').matches;
  } catch {
    return false;
  }
}

function themeColor(theme) {
  if (theme === 'contrast') return '#000000';
  if (theme === 'dark' || (theme === 'system' && systemPrefersDark())) return '#0b131b';
  return '#12263a';
}

/** Apply a theme immediately and remember it for the next visit. */
export function applyTheme(value, { persist = true } = {}) {
  const theme = normalizeTheme(value);
  if (typeof document !== 'undefined') {
    document.documentElement.dataset.theme = theme;
    const meta = document.querySelector('meta[name="theme-color"]');
    if (meta) meta.setAttribute('content', themeColor(theme));
    for (const select of document.querySelectorAll('select[data-theme-select]')) {
      if (select.value !== theme) select.value = theme;
    }
  }
  if (persist) {
    try {
      localStorage.setItem(THEME_STORAGE_KEY, theme);
    } catch {
      // Private browsing or a full quota: the current page still uses the theme.
    }
  }
  return theme;
}

/** Describe the active preference for status text. */
export function themeLabel(value) {
  const theme = normalizeTheme(value);
  return THEMES.find((candidate) => candidate.value === theme)?.label ?? 'System';
}
