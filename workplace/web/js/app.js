// Application shell, routing, auth lifecycle and bootstrap.

import * as api from './api.js';
import * as db from './db.js';
import { createStore } from './store.js';
import { createSyncEngine } from './sync.js';
import { errorState, h, spinner, toast } from './dom.js';
import { isoToday } from './format.js';
import { renderLogin } from './views/login.js';
import { renderToday } from './views/today.js';
import { renderEmployees } from './views/employees.js';
import { renderReports } from './views/reports.js';
import { renderSync } from './views/sync.js';
import { renderSettings } from './views/settings.js';
import { renderEmployeeDay } from './views/employee-day.js';
import { renderEmployeeReports } from './views/employee-reports.js';
import { THEMES, applyTheme, readStoredTheme } from './theme.js';

const store = createStore({
  booting: true,
  auth: false,
  role: 'admin',
  employeeId: null,
  employeeName: '',
  employeeCode: '',
  online: navigator.onLine,
  employees: [],
  allEmployees: [],
  entries: [],
  types: [],
  outbox: [],
  conflicts: [],
  syncing: false,
  lastSyncAt: null,
  syncError: null,
  selectedDate: isoToday(),
  route: '/today',
});

let viewCleanup = null;
let currentPath = null;
let shellReady = false;
let statusChip = null;
let syncBadge = null;
let navLinks = new Map();
let loggingOut = false;

const sync = createSyncEngine({
  store,
  refresh: () => refreshFromDb(),
  notify: (message, kind) => toast(message, kind),
  onSynced: () => refreshTypes(),
});

const ctx = {
  store,
  api,
  sync,
  get state() {
    return store.get();
  },
  refresh: refreshFromDb,
  refreshTypes,
  navigate,
  toast,
  findEmployee(id) {
    return store.get().allEmployees.find((employee) => employee.id === id) ?? null;
  },
  isPending(entity, id) {
    return store.get().outbox.some((change) => change.entity === entity && change.id === id);
  },
  findConflict(entity, id) {
    return (
      store.get().conflicts.find(
        (conflict) => conflict.entity === entity && conflict.id === id,
      ) ?? null
    );
  },
  onAuthenticated,
  logout: () => logout({ expired: false }),
};

/* ------------------------------------------------------------------ state */

async function refreshFromDb() {
  const [employees, entries, types, outbox, conflicts, lastSyncAt] = await Promise.all([
    db.getAll(db.STORES.employees),
    db.getAll(db.STORES.entries),
    db.getAll(db.STORES.types),
    db.getAll(db.STORES.outbox),
    db.getAll(db.STORES.conflicts),
    db.getMeta('last_sync', null),
  ]);
  store.set({
    booting: false,
    employees: employees.filter((employee) => !employee.deleted_at),
    allEmployees: employees,
    entries: entries.filter((entry) => !entry.deleted_at),
    types: types
      .filter((type) => !type.deleted_at)
      .sort(
        (a, b) =>
          (a.sort_order ?? 0) - (b.sort_order ?? 0) ||
          a.name.localeCompare(b.name, 'en', { sensitivity: 'base' }),
      ),
    outbox,
    conflicts,
    lastSyncAt: lastSyncAt ?? store.get().lastSyncAt,
  });
}

/// Earning types are server configuration; refresh the offline cache when the
/// device is online. Offline day-sheet entry keeps using the cached list.
async function refreshTypes() {
  if (!navigator.onLine) return;
  try {
    const types = await api.listEarningTypes();
    await db.putMany(db.STORES.types, types);
    store.set({
      types: types
        .filter((type) => !type.deleted_at)
        .sort(
          (a, b) =>
            (a.sort_order ?? 0) - (b.sort_order ?? 0) ||
            a.name.localeCompare(b.name, 'en', { sensitivity: 'base' }),
        ),
    });
  } catch (error) {
    console.warn('earning type refresh failed', error);
  }
}

/* ------------------------------------------------------------------- auth */

async function onAuthenticated(identity = {}) {
  const role = identity.role === 'employee' ? 'employee' : 'admin';
  await db.setMeta('auth_ok', Date.now());
  store.set({
    auth: true,
    role,
    employeeId: identity.employee_id ?? null,
    employeeName: identity.name ?? '',
    employeeCode: identity.code ?? '',
    selectedDate: isoToday(),
  });
  try {
    if (role === 'employee') await enterEmployeeApp();
    else await enterApp();
    toast('Signed in', 'success');
  } catch (error) {
    console.error('post-login setup failed', error);
    toast(error?.message ?? 'Could not load local data', 'error');
  }
}

async function logout({ expired }) {
  if (loggingOut) return;
  loggingOut = true;
  try {
    sync.stop();
    try {
      await api.logout();
    } catch {
      // Logging out locally must still work while offline.
    }
    await db.setMeta('auth_ok', null);
    store.set({ auth: false });
    showLogin();
    if (expired) toast('Session expired. Sign in again.', 'warn');
  } finally {
    loggingOut = false;
  }
}

function handleUnauthorized() {
  if (!store.get().auth) return;
  void logout({ expired: true });
}

/* ------------------------------------------------------------------ shell */

function buildShell() {
  const root = document.getElementById('root');
  statusChip = h('span', { class: 'status-chip', 'aria-live': 'polite' }, 'Starting…');
  syncBadge = h('span', { class: 'nav-badge', hidden: true });
  const themeSelect = h(
    'select',
    { class: 'theme-select', 'aria-label': 'Theme', 'data-theme-select': '' },
    THEMES.map((theme) => h('option', { value: theme.value }, theme.label)),
  );
  themeSelect.value = readStoredTheme();
  themeSelect.addEventListener('change', () => applyTheme(themeSelect.value));

  const isEmployee = store.get().role === 'employee';
  const items = isEmployee
    ? [
        { path: '/my-day', label: 'My day' },
        { path: '/my-reports', label: 'My reports' },
      ]
    : [
        { path: '/today', label: 'Today' },
        { path: '/employees', label: 'Employees' },
        { path: '/reports', label: 'Reports' },
        { path: '/sync', label: 'Sync', badge: syncBadge },
        { path: '/settings', label: 'Settings' },
      ];
  navLinks = new Map(
    items.map((item) => {
      const link = h(
        'a',
        { class: 'tab', href: `#${item.path}`, 'data-path': item.path },
        h('span', { class: 'tab-label' }, item.label),
        item.badge ?? null,
      );
      return [item.path, link];
    }),
  );

  const tabbar = h('nav', { class: 'tabbar', 'aria-label': 'Main navigation' }, [...navLinks.values()]);
  const topbar = h(
    'header',
    { class: 'topbar' },
    h(
      'div',
      { class: 'brand' },
      h('img', { src: '/icons/icon-192.png', alt: '', width: '28', height: '28' }),
      h('span', {}, 'Daybook'),
    ),
    h(
      'div',
      { class: 'topbar-actions' },
      statusChip,
      themeSelect,
      h(
        'button',
        { class: 'btn btn-ghost btn-small', type: 'button', on: { click: () => ctx.logout() } },
        'Log out',
      ),
    ),
  );

  root.replaceChildren(
    h(
      'div',
      { class: 'app' },
      topbar,
      h('main', { id: 'view', class: 'view-root', tabindex: '-1' }),
      tabbar,
    ),
  );
  shellReady = true;
  updateShell(store.get());
}

function ensureShell() {
  if (!shellReady || !document.getElementById('view')) buildShell();
}

function updateShell(state) {
  if (!shellReady) return;
  if (state.role === 'employee') {
    statusChip.textContent = state.employeeName || 'Employee';
    statusChip.className = 'status-chip status-ok';
    statusChip.title = '';
    syncBadge.hidden = true;
  } else if (!state.online) {
    statusChip.textContent = state.outbox.length ? `Offline · ${state.outbox.length} pending` : 'Offline';
    statusChip.className = 'status-chip status-offline';
  } else if (state.syncing) {
    statusChip.textContent = 'Syncing…';
    statusChip.className = 'status-chip status-syncing';
  } else if (state.syncError) {
    statusChip.textContent = 'Sync issue';
    statusChip.className = 'status-chip status-error';
    statusChip.title = state.syncError;
  } else if (state.conflicts.length) {
    statusChip.textContent = `${state.conflicts.length} conflict${state.conflicts.length === 1 ? '' : 's'}`;
    statusChip.className = 'status-chip status-error';
    statusChip.title = 'Review in the Sync tab';
  } else if (state.outbox.length) {
    statusChip.textContent = `${state.outbox.length} pending`;
    statusChip.className = 'status-chip status-pending';
  } else {
    statusChip.textContent = 'Up to date';
    statusChip.className = 'status-chip status-ok';
  }

  const conflictCount = state.conflicts.length;
  syncBadge.textContent = String(conflictCount);
  syncBadge.hidden = conflictCount === 0;

  for (const [path, link] of navLinks) {
    const isActive = state.route === path;
    link.classList.toggle('tab-active', isActive);
    if (isActive) link.setAttribute('aria-current', 'page');
    else link.removeAttribute('aria-current');
  }
}

/* ----------------------------------------------------------------- router */

function normalizeHash() {
  const raw = location.hash.replace(/^#/, '');
  if (!raw) return store.get().role === 'employee' ? '/my-day' : '/today';
  const path = raw.startsWith('/') ? raw : `/${raw}`;
  return path.split('?')[0];
}

function navigate(path) {
  const target = `#${path}`;
  if (location.hash === target) route();
  else location.hash = target;
}

function mountView(path) {
  if (path === currentPath && viewCleanup) return;
  viewCleanup?.();
  viewCleanup = null;

  const isEmployee = store.get().role === 'employee';
  const renderers = isEmployee
    ? {
        '/my-day': renderEmployeeDay,
        '/my-reports': renderEmployeeReports,
      }
    : {
        '/today': renderToday,
        '/employees': renderEmployees,
        '/reports': renderReports,
        '/sync': renderSync,
        '/settings': renderSettings,
      };
  const render = renderers[path] ?? (isEmployee ? renderEmployeeDay : renderToday);
  const viewRoot = document.getElementById('view');
  if (!viewRoot) {
    buildShell();
    return mountView(path);
  }
  viewCleanup = render(viewRoot, ctx) ?? null;
  currentPath = path;
  window.scrollTo(0, 0);
  viewRoot.focus({ preventScroll: true });
}

function route() {
  if (!store.get().auth) {
    showLogin();
    return;
  }
  ensureShell();
  const path = normalizeHash();
  if (!location.hash) {
    // Keep the address bar shareable without adding a history entry.
    history.replaceState(null, '', `#${path}`);
  }
  store.set({ route: path });
  mountView(path);
}

function showLogin() {
  sync.stop();
  viewCleanup?.();
  viewCleanup = null;
  currentPath = null;
  shellReady = false;
  navLinks.clear();
  const root = document.getElementById('root');
  viewCleanup = renderLogin(root, ctx) ?? null;
}

async function enterApp() {
  ensureShell();
  await refreshTypes();
  await refreshFromDb();
  sync.start();
  route();
}

async function enterEmployeeApp() {
  ensureShell();
  route();
}

/* --------------------------------------------------------------- bootstrap */

function fatalMessage(error) {
  const root = document.getElementById('root');
  root.replaceChildren(
    errorState(error?.message ?? 'Daybook could not start', () => location.reload()),
  );
}

async function bootstrap() {
  const root = document.getElementById('root');
  root.replaceChildren(h('div', { class: 'boot' }, spinner('Starting Daybook…')));

  try {
    await db.openDatabase();
  } catch (error) {
    fatalMessage(
      error?.message?.includes('blocked')
        ? error
        : new Error('Local storage is unavailable. Close other tabs and reload.'),
    );
    return;
  }

  try {
    await db.pruneConflicts();
  } catch (error) {
    console.warn('conflict pruning failed', error);
  }

  let remembered = false;
  try {
    remembered = Boolean(await db.getMeta('auth_ok', null));
  } catch (error) {
    console.warn('cannot read auth state', error);
  }

  let authenticated = false;
  let me = null;
  if (navigator.onLine) {
    try {
      me = await api.me();
      authenticated = Boolean(me?.authenticated);
    } catch (error) {
      // Network failure: allow offline use if this device signed in before.
      authenticated = error?.status === 0 && remembered;
      if (error?.status !== 0) console.warn('auth check failed', error);
    }
  } else {
    authenticated = remembered;
  }

  await db.setMeta('auth_ok', authenticated ? Date.now() : null).catch(() => {});
  const role = me?.role === 'employee' ? 'employee' : 'admin';
  store.set({
    auth: authenticated,
    booting: false,
    online: navigator.onLine,
    role,
    employeeId: me?.employee_id ?? null,
    employeeName: me?.name ?? '',
    employeeCode: me?.code ?? '',
  });

  if (authenticated) {
    try {
      if (role === 'employee') await enterEmployeeApp();
      else await enterApp();
    } catch (error) {
      console.error('startup failed', error);
      fatalMessage(error);
    }
  } else {
    showLogin();
  }
}

/* -------------------------------------------------------- service worker */

function registerServiceWorker() {
  if (!('serviceWorker' in navigator)) return;
  const secure =
    location.protocol === 'https:' ||
    location.hostname === 'localhost' ||
    location.hostname === '127.0.0.1';
  if (!secure) {
    console.info(
      'Service worker not registered: installable offline mode requires HTTPS on a LAN address.',
    );
    return;
  }

  window.addEventListener(
    'load',
    async () => {
      try {
        const registration = await navigator.serviceWorker.register('/sw.js');
        registration.addEventListener('updatefound', () => {
          const worker = registration.installing;
          if (!worker) return;
          worker.addEventListener('statechange', () => {
            if (worker.state === 'installed' && navigator.serviceWorker.controller) {
              toast('A new Daybook version is ready.', 'info', {
                timeout: 20_000,
                action: { label: 'Reload', run: () => location.reload() },
              });
            }
          });
        });
      } catch (error) {
        console.warn('service worker registration failed', error);
      }
    },
    { once: true },
  );
}

/* ------------------------------------------------------------------- boot */

api.setUnauthorizedHandler(handleUnauthorized);
applyTheme(readStoredTheme(), { persist: false });
try {
  window
    .matchMedia('(prefers-color-scheme: dark)')
    .addEventListener('change', () => {
      if (readStoredTheme() === 'system') applyTheme('system', { persist: false });
    });
} catch {
  // MatchMedia is unavailable in a few embedded webviews; CSS still adapts.
}
store.subscribe(updateShell);
window.addEventListener('hashchange', () => {
  if (store.get().auth) route();
});
window.addEventListener('online', () => store.set({ online: true }));
window.addEventListener('offline', () => store.set({ online: false }));

registerServiceWorker();
bootstrap().catch((error) => {
  console.error(error);
  fatalMessage(error);
});
