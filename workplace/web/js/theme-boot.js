// Runs synchronously in <head>, before the stylesheet paints. Keep it tiny and
// dependency-free; the CSP allows same-origin scripts only.
(function () {
  var theme = 'system';
  try {
    var stored = localStorage.getItem('daybook_theme');
    if (stored === 'light' || stored === 'dark' || stored === 'contrast' || stored === 'system') {
      theme = stored;
    }
  } catch (error) {
    // Storage blocked: fall back to the device scheme.
  }
  document.documentElement.setAttribute('data-theme', theme);
  var meta = document.querySelector('meta[name="theme-color"]');
  if (meta) {
    var dark =
      theme === 'dark' ||
      (theme === 'system' &&
        window.matchMedia &&
        window.matchMedia('(prefers-color-scheme: dark)').matches);
    meta.setAttribute('content', theme === 'contrast' ? '#000000' : dark ? '#0b131b' : '#12263a');
  }
})();
