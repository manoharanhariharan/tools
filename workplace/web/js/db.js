// IndexedDB persistence. One long-lived connection per tab; every mutation is
// awaited so no transaction or connection is left dangling.

const DB_NAME = 'daybook';
const DB_VERSION = 1;

export const STORES = {
  employees: 'employees',
  entries: 'entries',
  outbox: 'outbox',
  conflicts: 'conflicts',
  meta: 'meta',
};

let connectionPromise = null;

export function openDatabase() {
  if (connectionPromise) return connectionPromise;
  connectionPromise = new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, DB_VERSION);
    request.onupgradeneeded = () => {
      const db = request.result;
      if (!db.objectStoreNames.contains(STORES.employees)) {
        db.createObjectStore(STORES.employees, { keyPath: 'id' });
      }
      if (!db.objectStoreNames.contains(STORES.entries)) {
        const store = db.createObjectStore(STORES.entries, { keyPath: 'id' });
        store.createIndex('by_date', 'work_date', { unique: false });
      }
      if (!db.objectStoreNames.contains(STORES.outbox)) {
        db.createObjectStore(STORES.outbox, { keyPath: 'key' });
      }
      if (!db.objectStoreNames.contains(STORES.conflicts)) {
        db.createObjectStore(STORES.conflicts, { keyPath: 'key' });
      }
      if (!db.objectStoreNames.contains(STORES.meta)) {
        db.createObjectStore(STORES.meta, { keyPath: 'key' });
      }
    };
    request.onsuccess = () => {
      const db = request.result;
      // Another tab upgrading the schema must not deadlock: close and let the
      // next call reopen with the new version.
      db.onversionchange = () => {
        db.close();
        connectionPromise = null;
      };
      resolve(db);
    };
    request.onerror = () => reject(request.error ?? new Error('Cannot open local database'));
    request.onblocked = () => reject(new Error('Local database upgrade blocked by another tab'));
  });
  return connectionPromise;
}

export async function getAll(store) {
  const db = await openDatabase();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(store, 'readonly');
    const request = tx.objectStore(store).getAll();
    request.onsuccess = () => resolve(request.result ?? []);
    request.onerror = () => reject(request.error);
  });
}

export async function get(store, key) {
  const db = await openDatabase();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(store, 'readonly');
    const request = tx.objectStore(store).get(key);
    request.onsuccess = () => resolve(request.result ?? null);
    request.onerror = () => reject(request.error);
  });
}

export async function put(store, value) {
  await batch([{ store, type: 'put', value }]);
}

export async function putMany(store, values) {
  if (!values.length) return;
  await batch(values.map((value) => ({ store, type: 'put', value })));
}

export async function remove(store, key) {
  await batch([{ store, type: 'delete', key }]);
}

export async function clear(store) {
  const db = await openDatabase();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(store, 'readwrite');
    tx.objectStore(store).clear();
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
    tx.onabort = () => reject(tx.error);
  });
}

/** Atomically apply put/delete operations across one or more stores. */
export async function batch(operations) {
  if (!operations.length) return;
  const db = await openDatabase();
  const storeNames = [...new Set(operations.map((operation) => operation.store))];
  return new Promise((resolve, reject) => {
    const tx = db.transaction(storeNames, 'readwrite');
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
    tx.onabort = () => reject(tx.error ?? new Error('Local database transaction aborted'));
    for (const operation of operations) {
      const store = tx.objectStore(operation.store);
      if (operation.type === 'put') store.put(operation.value);
      else store.delete(operation.key);
    }
  });
}

export async function getMeta(key, fallback = null) {
  const rows = await getAll(STORES.meta);
  const row = rows.find((entry) => entry.key === key);
  return row ? row.value : fallback;
}

export async function setMeta(key, value) {
  await put(STORES.meta, { key, value });
}

/** Drop expired conflicts and cap the queue so storage cannot grow forever. */
export async function pruneConflicts(now = Date.now(), maxAgeMs = 30 * 86_400_000, maxCount = 200) {
  const all = await getAll(STORES.conflicts);
  if (!all.length) return;
  const recent = all
    .filter((conflict) => now - (conflict.at ?? 0) < maxAgeMs)
    .sort((a, b) => (b.at ?? 0) - (a.at ?? 0))
    .slice(0, maxCount);
  const keep = new Set(recent.map((conflict) => conflict.key));
  const stale = all.filter((conflict) => !keep.has(conflict.key));
  if (!stale.length) return;
  await batch(stale.map((conflict) => ({ store: STORES.conflicts, type: 'delete', key: conflict.key })));
}
