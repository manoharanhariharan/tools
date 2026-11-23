// Daybook service worker.
// Bump VERSION on every deployment so old caches are removed on activate.
// The UI is a no-build ES-module app; the shell list below must stay in sync
// with web/ (an integration test verifies every path exists).

const VERSION = 'daybook-v1';
const CACHE = `daybook-${VERSION}`;

const SHELL = [
  '/',
  '/index.html',
  '/css/app.css',
  '/manifest.webmanifest',
  '/favicon.png',
  '/icons/icon-192.png',
  '/icons/icon-512.png',
  '/icons/icon-maskable-512.png',
  '/icons/apple-touch-icon.png',
  '/js/app.js',
  '/js/api.js',
  '/js/db.js',
  '/js/store.js',
  '/js/format.js',
  '/js/pay.js',
  '/js/dom.js',
  '/js/mutations.js',
  '/js/sync.js',
  '/js/sync-logic.js',
  '/js/views/login.js',
  '/js/views/today.js',
  '/js/views/employees.js',
  '/js/views/employee-dialog.js',
  '/js/views/entry-dialog.js',
  '/js/views/reports.js',
  '/js/views/sync.js',
];

self.addEventListener('install', (event) => {
  event.waitUntil(
    (async () => {
      const cache = await caches.open(CACHE);
      await cache.addAll(SHELL.map((url) => new Request(url, { cache: 'reload' })));
      await self.skipWaiting();
    })(),
  );
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    (async () => {
      const keys = await caches.keys();
      await Promise.all(keys.filter((key) => key !== CACHE).map((key) => caches.delete(key)));
      await self.clients.claim();
    })(),
  );
});

self.addEventListener('fetch', (event) => {
  const { request } = event;
  if (request.method !== 'GET') return;

  const url = new URL(request.url);
  if (url.origin !== self.location.origin) return;

  // API traffic is never cached: stale payroll data is worse than an error.
  if (url.pathname.startsWith('/api/')) return;

  if (request.mode === 'navigate') {
    event.respondWith(networkFirst(event, request));
    return;
  }
  event.respondWith(staleWhileRevalidate(event, request));
});

async function networkFirst(event, request) {
  const cache = await caches.open(CACHE);
  try {
    const response = await fetch(request);
    if (response.ok) {
      await cache.put(new Request('/index.html'), response.clone());
    }
    return response;
  } catch {
    const cached = await cache.match('/index.html');
    return (
      cached ??
      new Response('Daybook is offline and the cached app shell is unavailable.', {
        status: 503,
        headers: { 'Content-Type': 'text/plain; charset=utf-8' },
      })
    );
  }
}

async function staleWhileRevalidate(event, request) {
  const cache = await caches.open(CACHE);
  const cached = await cache.match(request);

  const update = fetch(request)
    .then(async (response) => {
      if (response.ok) await cache.put(request, response.clone());
      return response;
    })
    .catch(() => null);

  if (cached) {
    event.waitUntil(update);
    return cached;
  }
  const response = await update;
  return response ?? new Response('', { status: 504, statusText: 'Offline' });
}
