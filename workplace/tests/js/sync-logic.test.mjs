import test from 'node:test';
import assert from 'node:assert/strict';

import {
  isPermanentConflict,
  mergeStrategy,
  nextPullCursor,
  outboxKey,
  pendingKeys,
  sortChanges,
} from '../../web/js/sync-logic.js';

const change = (entity, id) => ({ entity, id, base_seq: 0, deleted: false });

test('outbox keys are namespaced by entity', () => {
  assert.equal(outboxKey('employee', 'a'), 'employee:a');
  assert.equal(outboxKey('entry', 'a'), 'entry:a');
});

test('changes are ordered employees first but otherwise stable', () => {
  const sorted = sortChanges([
    change('entry', 'e1'),
    change('employee', 'emp1'),
    change('entry', 'e2'),
    change('employee', 'emp2'),
  ]);
  assert.deepEqual(
    sorted.map((c) => c.id),
    ['emp1', 'emp2', 'e1', 'e2'],
  );
});

test('pending keys include both entity types', () => {
  const keys = pendingKeys([change('employee', 'a'), change('entry', 'b')]);
  assert.ok(keys.has('employee:a'));
  assert.ok(keys.has('entry:b'));
  assert.ok(!keys.has('entry:a'));
});

test('pending local edits are never clobbered by a pull', () => {
  const pending = new Set(['entry:x']);
  assert.equal(mergeStrategy({ seq: 1 }, { seq: 2, id: 'x' }, 'entry', pending), 'keep-local');
});

test('newer server rows are applied, older ones skipped', () => {
  const pending = new Set();
  assert.equal(mergeStrategy(null, { seq: 2, id: 'x' }, 'entry', pending), 'apply');
  assert.equal(mergeStrategy({ seq: 2 }, { seq: 3, id: 'x' }, 'entry', pending), 'apply');
  assert.equal(mergeStrategy({ seq: 5 }, { seq: 4, id: 'x' }, 'entry', pending), 'skip');
  assert.equal(mergeStrategy({ seq: 5 }, null, 'entry', pending), 'skip');
});

test('pull cursor advances and detects a restored server', () => {
  assert.deepEqual(nextPullCursor(10, 5, 8), { reset: false, cursor: 8 });
  assert.deepEqual(nextPullCursor(10, 5, null), { reset: false, cursor: 5 });
  assert.deepEqual(nextPullCursor(3, 9, 9), { reset: true, cursor: 0 });
});

test('permanent conflicts are recognised', () => {
  assert.ok(isPermanentConflict('duplicate_code'));
  assert.ok(isPermanentConflict('duplicate_entry_date'));
  assert.ok(!isPermanentConflict('stale'));
});
