import test from 'node:test';
import assert from 'node:assert/strict';

import { computeEntry, normalizeAmounts, rateLabel } from '../../web/js/pay.js';

const dayRate = {
  pay_basis: 'day_rate',
  rate_minor: 80000,
  ot_multiplier_bp: 15000,
  standard_minutes_per_day: 480,
};

const hourly = {
  pay_basis: 'hourly',
  rate_minor: 15000,
  ot_multiplier_bp: 15000,
  standard_minutes_per_day: 480,
};

const fixed = {
  pay_basis: 'fixed',
  rate_minor: 70000,
  ot_multiplier_bp: 15000,
  standard_minutes_per_day: 480,
};

test('day rate full day and half day', () => {
  assert.equal(computeEntry(dayRate, {}).base_minor, 80000);
  assert.equal(computeEntry(dayRate, { day_fraction_bp: 5000 }).base_minor, 40000);
});

test('day rate overtime uses the standard day', () => {
  const result = computeEntry(dayRate, { overtime_minutes: 120 });
  assert.equal(result.ot_minor, 30000);
  assert.equal(result.gross_minor, 110000);
});

test('hourly base and overtime', () => {
  const result = computeEntry(hourly, { regular_minutes: 450, overtime_minutes: 90 });
  assert.equal(result.base_minor, 112500);
  assert.equal(result.ot_minor, 33750);
  assert.equal(result.gross_minor, 146250);
});

test('hourly rounding matches the server half-up rule', () => {
  assert.equal(computeEntry(hourly, { regular_minutes: 30 }).base_minor, 7500);
  assert.equal(
    computeEntry({ ...hourly, rate_minor: 101 }, { regular_minutes: 30 }).base_minor,
    51,
  );
});

test('fixed basis uses rate or override', () => {
  assert.equal(computeEntry(fixed, {}).base_minor, 70000);
  assert.equal(computeEntry(fixed, { override_amount_minor: 95000 }).base_minor, 95000);
  assert.equal(computeEntry(fixed, { overtime_minutes: 60 }).ot_minor, 0);
});

test('entry override replaces base for day rate', () => {
  assert.equal(computeEntry(dayRate, { override_amount_minor: 123456 }).base_minor, 123456);
});

test('bonus, advance and deduction flow into net', () => {
  const result = computeEntry(dayRate, {
    bonus_minor: 10000,
    advance_minor: 20000,
    deduction_minor: 5000,
  });
  assert.equal(result.gross_minor, 90000);
  assert.equal(result.net_minor, 65000);
});

test('advance may exceed gross producing negative net', () => {
  assert.equal(computeEntry(dayRate, { advance_minor: 100000 }).net_minor, -20000);
});

test('irrelevant fields are dropped per basis', () => {
  assert.equal(normalizeAmounts('day_rate', { regular_minutes: 300 }).regular_minutes, 0);
  assert.equal(normalizeAmounts('hourly', { day_fraction_bp: 5000 }).day_fraction_bp, 10000);
  const normalized = normalizeAmounts('fixed', { overtime_minutes: 60, regular_minutes: 60 });
  assert.equal(normalized.overtime_minutes, 0);
  assert.equal(normalized.regular_minutes, 0);
});

test('rate labels explain the basis', () => {
  assert.equal(rateLabel(dayRate), '₹800.00 / day');
  assert.equal(rateLabel(hourly), '₹150.00 / hour');
  assert.equal(rateLabel(fixed), '₹700.00 / entry');
});
