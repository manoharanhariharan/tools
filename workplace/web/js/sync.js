// Sync engine: drains the local outbox to the server, then pulls every change
// since the stored cursor. Conflicting or rejected changes are moved to a
// review queue instead of being retried forever.

import * as api from './api.js';
import * as db from './db.js';
import { mergeStrategy, nextPullCursor, outboxKey, pendingKeys, sortChanges } from './sync-logic.js';
const PUSH_BATCH = 100;
const PULL_LIMIT = 200;
const MAX_PULL_PAGES = 60;
const SYNC_INTERVAL_MS = 60_000;
const MAX_CONFLICTS = 200;

function stripLocalFlags(row) {
  const { pending: _pending, ...rest } = row;
  return rest;
}

function serverChange(change) {
  return {
    entity: change.entity,
    id: change.id,
    base_seq: change.base_seq ?? 0,
    deleted: Boolean(change.deleted),
    force: Boolean(change.force),
    data: change.data ?? null,
  };
}

export function createSyncEngine({ store, refresh, notify, onSynced }) {
  let running = false;
  let interval = null;
  const listenerCleanups = [];

  async function getDeviceId() {
    let id = await db.getMeta('device_id');
    if (!id) {
      id = `web-${globalThis.crypto?.randomUUID?.() ?? Date.now().toString(36)}`;
      await db.setMeta('device_id', id);
    }
    return id;
  }

  async function pushOutbox() {
    const outbox = await db.getAll(db.STORES.outbox);
    if (!outbox.length) return;
    const deviceId = await getDeviceId();

    for (let offset = 0; offset < outbox.length; offset += PUSH_BATCH) {
      const chunk = sortChanges(outbox.slice(offset, offset + PUSH_BATCH));
      const response = await api.push(chunk.map(serverChange), deviceId);
      const operations = [];

      for (const result of response?.results ?? []) {
        const key = outboxKey(result.entity, result.id);
        const storeName =
          result.entity === 'employee' ? db.STORES.employees : db.STORES.entries;
        const source = chunk.find(
          (change) => change.entity === result.entity && change.id === result.id,
        );

        if (result.status === 'applied' || result.status === 'noop') {
          if (result.server) {
            operations.push({ store: storeName, type: 'put', value: stripLocalFlags(result.server) });
          }
          operations.push({ store: db.STORES.outbox, type: 'delete', key });
        } else if (result.status === 'conflict' || result.status === 'invalid') {
          const localRow = await db.get(storeName, result.id);
          operations.push({
            store: db.STORES.conflicts,
            type: 'put',
            value: {
              key,
              entity: result.entity,
              id: result.id,
              type: result.status,
              code: result.conflict ?? result.error?.code ?? null,
              error: result.error ?? null,
              server: result.server ?? null,
              change: source ? stripLocalFlags(source) : null,
              local: localRow ? stripLocalFlags(localRow) : null,
              at: Date.now(),
            },
          });
          operations.push({ store: db.STORES.outbox, type: 'delete', key });
        }
        // Unknown status: keep the outbox entry so no change is silently lost.
      }

      if (operations.length) await db.batch(operations);
    }
  }

  async function dropNonPendingRows() {
    const pending = pendingKeys(await db.getAll(db.STORES.outbox));
    const operations = [];
    for (const row of await db.getAll(db.STORES.employees)) {
      if (!pending.has(outboxKey('employee', row.id))) {
        operations.push({ store: db.STORES.employees, type: 'delete', key: row.id });
      }
    }
    for (const row of await db.getAll(db.STORES.entries)) {
      if (!pending.has(outboxKey('entry', row.id))) {
        operations.push({ store: db.STORES.entries, type: 'delete', key: row.id });
      }
    }
    if (operations.length) await db.batch(operations);
    await db.setMeta('cursor', 0);
  }

  async function pullAll() {
    let cursor = Number(await db.getMeta('cursor', 0)) || 0;

    for (let pageIndex = 0; pageIndex < MAX_PULL_PAGES; pageIndex += 1) {
      const page = await api.pull(cursor, PULL_LIMIT);
      const { reset, cursor: nextCursor } = nextPullCursor(page.latest, cursor, page.cursor);

      if (reset) {
        // The server was restored from an older backup. Rebuild all non-pending
        // rows from scratch; pending local edits stay queued.
        await dropNonPendingRows();
        cursor = 0;
        continue;
      }

      const pending = pendingKeys(await db.getAll(db.STORES.outbox));
      const localEmployees = new Map(
        (await db.getAll(db.STORES.employees)).map((row) => [row.id, row]),
      );
      const localEntries = new Map((await db.getAll(db.STORES.entries)).map((row) => [row.id, row]));
      const operations = [];

      for (const row of page.employees ?? []) {
        if (mergeStrategy(localEmployees.get(row.id), row, 'employee', pending) === 'apply') {
          operations.push({
            store: db.STORES.employees,
            type: 'put',
            value: stripLocalFlags(row),
          });
        }
      }
      for (const row of page.entries ?? []) {
        if (mergeStrategy(localEntries.get(row.id), row, 'entry', pending) === 'apply') {
          operations.push({ store: db.STORES.entries, type: 'put', value: stripLocalFlags(row) });
        }
      }
      if (operations.length) await db.batch(operations);

      cursor = nextCursor;
      await db.setMeta('cursor', cursor);
      if (!page.has_more) break;
    }
  }

  async function run({ manual = false } = {}) {
    if (running) return;
    if (!navigator.onLine) {
      store.set({ online: false, syncing: false });
      if (manual) notify('You are offline. Changes are saved on this device.', 'warn');
      return;
    }

    running = true;
    store.set({ syncing: true, syncError: null });
    try {
      await pushOutbox();
      await pullAll();
      if (onSynced) await onSynced();
      const conflicts = (await db.getAll(db.STORES.conflicts)).slice(-MAX_CONFLICTS);
      const lastSyncAt = Date.now();
      await db.setMeta('last_sync', lastSyncAt);
      store.set({ lastSyncAt, conflicts, syncing: false, syncError: null, online: true });
      await refresh();
      if (manual) {
        notify(
          conflicts.length
            ? 'Synced. Some changes need your review.'
            : 'All changes synced.',
          conflicts.length ? 'warn' : 'success',
        );
      }
    } catch (error) {
      console.error('sync failed', error);
      store.set({ syncing: false, syncError: error?.message ?? 'Sync failed' });
      if (manual) notify(error?.message ?? 'Sync failed', 'error');
    } finally {
      running = false;
      if (store.get().syncing) store.set({ syncing: false });
    }
  }

  function start() {
    stop();
    const onOnline = () => {
      store.set({ online: true });
      run();
    };
    const onOffline = () => store.set({ online: false });
    const onVisible = () => {
      if (document.visibilityState === 'visible' && navigator.onLine) run();
    };

    window.addEventListener('online', onOnline);
    window.addEventListener('offline', onOffline);
    document.addEventListener('visibilitychange', onVisible);
    listenerCleanups.push(
      () => window.removeEventListener('online', onOnline),
      () => window.removeEventListener('offline', onOffline),
      () => document.removeEventListener('visibilitychange', onVisible),
    );

    interval = setInterval(() => {
      if (navigator.onLine && document.visibilityState === 'visible') run();
    }, SYNC_INTERVAL_MS);

    run();
  }

  function stop() {
    if (interval) {
      clearInterval(interval);
      interval = null;
    }
    while (listenerCleanups.length) listenerCleanups.pop()();
  }

  return { run, start, stop, isRunning: () => running };
}
