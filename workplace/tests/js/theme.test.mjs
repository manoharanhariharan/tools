import test from 'node:test';
import assert from 'node:assert/strict';

import {
  THEMES,
  THEME_STORAGE_KEY,
  normalizeTheme,
  readStoredTheme,
  themeLabel,
} from '../../web/js/theme.js';

test('theme options include the device scheme, light, dark and high contrast', () => {
  assert.deepEqual(
    THEMES.map((theme) => theme.value),
    ['system', 'light', 'dark', 'contrast'],
  );
  assert.equal(THEME_STORAGE_KEY, 'daybook_theme');
});

test('unknown stored values fall back to the system theme', () => {
  assert.equal(normalizeTheme('dark'), 'dark');
  assert.equal(normalizeTheme('contrast'), 'contrast');
  assert.equal(normalizeTheme('neon'), 'system');
  assert.equal(normalizeTheme(null), 'system');
  assert.equal(normalizeTheme(undefined), 'system');
});

test('theme labels are human readable', () => {
  assert.equal(themeLabel('system'), 'System');
  assert.equal(themeLabel('contrast'), 'High contrast');
  assert.equal(themeLabel('nope'), 'System');
});

test('stored theme is read without touching private storage data', () => {
  const previous = globalThis.localStorage;
  globalThis.localStorage = {
    getItem(key) {
      assert.equal(key, THEME_STORAGE_KEY);
      return 'dark';
    },
  };
  try {
    assert.equal(readStoredTheme(), 'dark');
  } finally {
    if (previous === undefined) delete globalThis.localStorage;
    else globalThis.localStorage = previous;
  }
});

test('unavailable storage falls back to system', () => {
  const previous = globalThis.localStorage;
  globalThis.localStorage = {
    getItem() {
      throw new Error('blocked');
    },
  };
  try {
    assert.equal(readStoredTheme(), 'system');
  } finally {
    if (previous === undefined) delete globalThis.localStorage;
    else globalThis.localStorage = previous;
  }
});
