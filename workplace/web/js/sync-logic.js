// Pure sync helpers. Keeping the decision logic here makes it testable under
// Node without IndexedDB or a browser.

export function outboxKey(entity, id) {
  return `${entity}:${id}`;
}

/** Employees must reach the server before entries that reference them. */
export function sortChanges(changes) {
  return [...changes].sort((a, b) => {
    const rank = (change) => (change.entity === 'employee' ? 0 : 1);
    return rank(a) - rank(b);
  });
}

export function pendingKeys(outbox) {
  return new Set(outbox.map((change) => outboxKey(change.entity, change.id)));
}

/**
 * Decide what to do with a server row that arrived during a pull.
 * Local pending edits always win until the push resolves them.
 */
export function mergeStrategy(localRow, serverRow, entity, pending) {
  if (!serverRow) return 'skip';
  if (pending.has(outboxKey(entity, serverRow.id))) return 'keep-local';
  if (!localRow) return 'apply';
  if ((serverRow.seq ?? 0) >= (localRow.seq ?? 0)) return 'apply';
  return 'skip';
}

/**
 * Advance the pull cursor, detecting a server that was restored from an older
 * backup (latest < since) so the client can rebuild from scratch.
 */
export function nextPullCursor(latest, since, pageCursor) {
  if (Number.isFinite(latest) && latest < since) {
    return { reset: true, cursor: 0 };
  }
  const safePage = Number.isFinite(pageCursor) && pageCursor >= since ? pageCursor : since;
  return { reset: false, cursor: safePage };
}

export function isPermanentConflict(code) {
  return code === 'duplicate_code' || code === 'duplicate_entry_date';
}
