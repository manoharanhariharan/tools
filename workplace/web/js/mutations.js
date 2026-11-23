// Local write path: every mutation updates IndexedDB optimistically and queues
// a sync change in the same transaction, so a crash cannot queue half a write.

import * as db from './db.js';
import { computeTotals } from './totals.js';
import { outboxKey } from './sync-logic.js';

export function newId() {
  if (globalThis.crypto?.randomUUID) return globalThis.crypto.randomUUID();
  const bytes = globalThis.crypto.getRandomValues(new Uint8Array(16));
  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  const hex = [...bytes].map((byte) => byte.toString(16).padStart(2, '0')).join('');
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

function employeePayload(row) {
  return {
    name: row.name,
    code: row.code,
    note: row.note,
    active: row.active,
  };
}

function entryPayload(row) {
  return {
    employee_id: row.employee_id,
    work_date: row.work_date,
    lines: row.lines.map((line) => ({
      type_id: line.type_id,
      description: line.description,
      amount_minor: line.amount_minor,
    })),
    bonus_minor: row.bonus_minor,
    advance_minor: row.advance_minor,
    deduction_minor: row.deduction_minor,
    note: row.note,
  };
}

export function stripLocalFlags(row) {
  const { pending: _pending, ...rest } = row;
  return rest;
}

export async function saveEmployee(input, { existing = null, conflict = null } = {}) {
  const id = existing?.id ?? newId();
  const now = Date.now();
  const base = existing?.seq ?? 0;
  const force = Boolean(conflict);
  const serverSeq = conflict?.server?.seq ?? base;

  const row = {
    id,
    name: input.name,
    code: input.code || null,
    note: input.note ?? '',
    active: input.active !== false,
    created_at: existing?.created_at ?? now,
    updated_at: now,
    deleted_at: null,
    seq: base,
    pending: true,
  };

  const change = {
    key: outboxKey('employee', id),
    entity: 'employee',
    id,
    base_seq: serverSeq,
    deleted: false,
    force,
    data: employeePayload(row),
    queued_at: now,
  };

  const operations = [
    { store: db.STORES.employees, type: 'put', value: row },
    { store: db.STORES.outbox, type: 'put', value: change },
  ];
  if (conflict) {
    operations.push({ store: db.STORES.conflicts, type: 'delete', key: conflict.key });
  }
  await db.batch(operations);
  return row;
}

function localLines(lines) {
  return lines
    .filter((line) => (line.amount_minor ?? 0) > 0)
    .map((line, position) => ({
      id: line.id ?? newId(),
      entry_id: line.entry_id ?? '',
      type_id: line.type_id,
      type_name: line.type_name,
      description: line.description ?? '',
      amount_minor: line.amount_minor,
      position,
      created_at: line.created_at ?? Date.now(),
    }));
}

export async function saveEntry({
  employee,
  date,
  existing = null,
  lines,
  adjustments = {},
  note = '',
  conflict = null,
}) {
  const id = existing?.id ?? newId();
  const now = Date.now();
  const base = existing?.seq ?? 0;
  const force = Boolean(conflict);
  const serverSeq = conflict?.server?.seq ?? base;
  const storedLines = localLines(lines).map((line) => ({ ...line, entry_id: id }));
  const totals = computeTotals(storedLines, adjustments);

  const row = {
    id,
    employee_id: employee.id,
    work_date: date,
    lines: storedLines,
    bonus_minor: totals.bonus_minor,
    advance_minor: totals.advance_minor,
    deduction_minor: totals.deduction_minor,
    business_minor: totals.business_minor,
    gross_minor: totals.gross_minor,
    net_minor: totals.net_minor,
    note,
    created_at: existing?.created_at ?? now,
    updated_at: now,
    deleted_at: null,
    seq: base,
    pending: true,
  };

  const change = {
    key: outboxKey('entry', id),
    entity: 'entry',
    id,
    base_seq: serverSeq,
    deleted: false,
    force,
    data: entryPayload(row),
    queued_at: now,
  };

  const operations = [
    { store: db.STORES.entries, type: 'put', value: row },
    { store: db.STORES.outbox, type: 'put', value: change },
  ];
  if (conflict) {
    operations.push({ store: db.STORES.conflicts, type: 'delete', key: conflict.key });
  }
  await db.batch(operations);
  return row;
}

export async function deleteRow(entity, row, { conflict = null } = {}) {
  const store = entity === 'employee' ? db.STORES.employees : db.STORES.entries;
  const key = outboxKey(entity, row.id);

  // Never-synced rows have no server identity: drop them locally, no tombstone.
  if (!row.seq) {
    await db.batch([
      { store, type: 'delete', key: row.id },
      { store: db.STORES.outbox, type: 'delete', key },
    ]);
    return;
  }

  const now = Date.now();
  const force = Boolean(conflict);
  const tombstone = {
    ...stripLocalFlags(row),
    deleted_at: now,
    updated_at: now,
    pending: true,
  };
  const change = {
    key,
    entity,
    id: row.id,
    base_seq: conflict?.server?.seq ?? row.seq,
    deleted: true,
    force,
    data: null,
    queued_at: now,
  };
  const operations = [
    { store, type: 'put', value: tombstone },
    { store: db.STORES.outbox, type: 'put', value: change },
  ];
  if (conflict) {
    operations.push({ store: db.STORES.conflicts, type: 'delete', key: conflict.key });
  }
  await db.batch(operations);
}

/** Accept the server's version (or discard a rejected local create). */
export async function useServerVersion(conflict) {
  const store = conflict.entity === 'employee' ? db.STORES.employees : db.STORES.entries;
  const operations = [{ store: db.STORES.conflicts, type: 'delete', key: conflict.key }];
  if (conflict.server) {
    operations.push({ store, type: 'put', value: stripLocalFlags(conflict.server) });
  } else {
    operations.push({ store, type: 'delete', key: conflict.id });
  }
  await db.batch(operations);
}

/** Discard the rejected local row (duplicate code/date or invalid payload). */
export async function discardLocalVersion(conflict) {
  const store = conflict.entity === 'employee' ? db.STORES.employees : db.STORES.entries;
  await db.batch([
    { store, type: 'delete', key: conflict.id },
    { store: db.STORES.conflicts, type: 'delete', key: conflict.key },
  ]);
}

/** Re-queue the local change with force so it overwrites the server row. */
export async function keepMyVersion(conflict) {
  if (!conflict.change) return;
  const change = {
    ...conflict.change,
    force: true,
    base_seq: conflict.server?.seq ?? conflict.change.base_seq ?? 0,
    queued_at: Date.now(),
  };
  await db.batch([
    { store: db.STORES.outbox, type: 'put', value: change },
    { store: db.STORES.conflicts, type: 'delete', key: conflict.key },
  ]);
}
