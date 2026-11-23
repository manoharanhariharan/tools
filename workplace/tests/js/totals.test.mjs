import test from 'node:test';
import assert from 'node:assert/strict';

import { computeTotals, lineDetail } from '../../web/js/totals.js';

const sales = (amount, description = '') => ({
  type_id: 'sales',
  type_name: 'Sales',
  description,
  amount_minor: amount,
});

test('business is the sum of earning rows', () => {
  const totals = computeTotals([sales(50_000), { ...sales(30_000), type_id: 'service', type_name: 'Service' }]);
  assert.equal(totals.business_minor, 80_000);
  assert.equal(totals.gross_minor, 80_000);
  assert.equal(totals.net_minor, 80_000);
});

test('bonus, advance and deduction drive final money', () => {
  const totals = computeTotals([sales(80_000)], {
    bonus_minor: 10_000,
    advance_minor: 20_000,
    deduction_minor: 5_000,
  });
  assert.equal(totals.business_minor, 80_000);
  assert.equal(totals.gross_minor, 90_000);
  assert.equal(totals.net_minor, 65_000);
});

test('advance may exceed gross producing negative final money', () => {
  const totals = computeTotals([sales(10_000)], { advance_minor: 25_000 });
  assert.equal(totals.net_minor, -15_000);
});

test('missing and string amounts are treated as zero', () => {
  const totals = computeTotals([{ type_id: 'x', type_name: 'X' }, { type_id: 'y', type_name: 'Y', amount_minor: '5000' }]);
  assert.equal(totals.business_minor, 5_000);
});

test('line detail carries the type, amount and description', () => {
  assert.equal(lineDetail(sales(80_000, '3 jobs')), 'Sales 800.00 — 3 jobs');
  assert.equal(lineDetail(sales(80_000)), 'Sales 800.00');
});
