// Local write path: every mutation updates IndexedDB optimistically and queues
// a sync change in the same transaction, so a crash cannot queue half a write.

import * as db from './db.js';
import { computeEntry } from './pay.js';
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
    pay_basis: row.pay_basis,
    rate_minor: row.rate_minor,
    ot_multiplier_bp: row.ot_multiplier_bp,
    standard_minutes_per_day: row.standard_minutes_per_day,
    note: row.note,
    active: row.active,
  };
}

function entryPayload(row) {
  return {
    employee_id: row.employee_id,
    work_date: row.work_date,
    day_fraction_bp: row.day_fraction_bp,
    regular_minutes: row.regular_minutes,
    overtime_minutes: row.overtime_minutes,
    ot_multiplier_bp: row.ot_multiplier_bp,
    override_amount_minor: row.override_amount_minor,
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
    pay_basis: input.pay_basis,
    rate_minor: input.rate_minor,
    ot_multiplier_bp: input.ot_multiplier_bp,
    standard_minutes_per_day: input.standard_minutes_per_day,
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

export async function saveEntry({
  employee,
  date,
  existing = null,
  amounts,
  note = '',
  conflict = null,
}) {
  const id = existing?.id ?? newId();
  const now = Date.now();
  const base = existing?.seq ?? 0;
  const force = Boolean(conflict);
  const serverSeq = conflict?.server?.seq ?? base;
  const computed = computeEntry(employee, amounts);

  const row = {
    id,
    employee_id: employee.id,
    work_date: date,
    day_fraction_bp: computed.day_fraction_bp,
    regular_minutes: computed.regular_minutes,
    overtime_minutes: computed.overtime_minutes,
    ot_multiplier_bp: computed.ot_multiplier_bp,
    override_amount_minor: computed.override_amount_minor,
    bonus_minor: computed.bonus_minor,
    advance_minor: computed.advance_minor,
    deduction_minor: computed.deduction_minor,
    base_minor: computed.base_minor,
    ot_minor: computed.ot_minor,
    gross_minor: computed.gross_minor,
    net_minor: computed.net_minor,
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
