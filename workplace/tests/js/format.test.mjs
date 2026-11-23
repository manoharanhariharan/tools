import test from 'node:test';
import assert from 'node:assert/strict';

import {
  addDays,
  currentMonth,
  displayDate,
  formatDecimal,
  formatHours,
  formatPaise,
  formatTimestamp,
  fractionLabel,
  groupIndian,
  isoToday,
  monthBounds,
  parseHoursToMinutes,
  parsePaise,
} from '../../web/js/format.js';

test('Indian digit grouping', () => {
  assert.equal(groupIndian('1'), '1');
  assert.equal(groupIndian('999'), '999');
  assert.equal(groupIndian('1000'), '1,000');
  assert.equal(groupIndian('100000'), '1,00,000');
  assert.equal(groupIndian('1234567'), '12,34,567');
  assert.equal(groupIndian('10000000'), '1,00,00,000');
});

test('formatPaise uses rupees, paise and Indian grouping', () => {
  assert.equal(formatPaise(0), '₹0.00');
  assert.equal(formatPaise(5), '₹0.05');
  assert.equal(formatPaise(80000), '₹800.00');
  assert.equal(formatPaise(123456789), '₹12,34,567.89');
  assert.equal(formatPaise(-500), '₹-5.00');
  assert.equal(formatPaise(1234, { symbol: false }), '12.34');
  assert.equal(formatPaise(Number.NaN), '₹—');
});

test('formatDecimal is spreadsheet friendly', () => {
  assert.equal(formatDecimal(0), '0.00');
  assert.equal(formatDecimal(123456), '1234.56');
  assert.equal(formatDecimal(-123456), '-1234.56');
});

test('parsePaise accepts human input and rejects junk', () => {
  assert.equal(parsePaise('1,234.5'), 123450);
  assert.equal(parsePaise('₹1234'), 123400);
  assert.equal(parsePaise(' 0.05 '), 5);
  assert.equal(parsePaise('0'), 0);
  assert.equal(parsePaise('12.345'), null);
  assert.equal(parsePaise('1.2.3'), null);
  assert.equal(parsePaise('-5'), null);
  assert.equal(parsePaise('abc'), null);
  assert.equal(parsePaise(''), null);
  assert.equal(parsePaise(null), null);
});

test('hours conversion matches server rounding', () => {
  assert.equal(formatHours(0), '0.00');
  assert.equal(formatHours(30), '0.50');
  assert.equal(formatHours(450), '7.50');
  assert.equal(formatHours(445), '7.42');
  assert.equal(parseHoursToMinutes('7.5'), 450);
  assert.equal(parseHoursToMinutes('7.25'), 435);
  assert.equal(parseHoursToMinutes('0'), 0);
  assert.equal(parseHoursToMinutes('25'), null);
  assert.equal(parseHoursToMinutes('7.999'), null);
  assert.equal(parseHoursToMinutes('abc'), null);
});

test('day fractions display as decimals', () => {
  assert.equal(fractionLabel(5000), '0.50');
  assert.equal(fractionLabel(10000), '1.00');
  assert.equal(fractionLabel(15000), '1.50');
});

test('dates are civil ISO strings with dh-mm-yyyy display', () => {
  assert.equal(displayDate('2026-11-23'), '23-11-2026');
  assert.equal(displayDate('bad'), 'bad');
  assert.equal(addDays('2026-11-30', 1), '2026-12-01');
  assert.equal(addDays('2026-01-01', -1), '2025-12-31');
  assert.equal(addDays('2028-02-28', 1), '2028-02-29');
  assert.equal(addDays('2026-11-23', 0), '2026-11-23');
});

test('month bounds handle leap years and invalid input', () => {
  assert.deepEqual(monthBounds('2026-11'), { from: '2026-11-01', to: '2026-11-30' });
  assert.deepEqual(monthBounds('2028-02'), { from: '2028-02-01', to: '2028-02-29' });
  assert.deepEqual(monthBounds('2026-12'), { from: '2026-12-01', to: '2026-12-31' });
  assert.equal(monthBounds('2026-13'), null);
  assert.equal(monthBounds('nonsense'), null);
});

test('IST today and month are ISO-shaped', () => {
  assert.match(isoToday(), /^\d{4}-\d{2}-\d{2}$/);
  assert.match(currentMonth(), /^\d{4}-\d{2}$/);
});

test('timestamps render in IST', () => {
  // 1700000000000 ms = 2023-11-14T22:13:20Z = 2023-11-15 03:43 IST
  assert.equal(formatTimestamp(1700000000000), '15-11-2023 03:43');
  assert.equal(formatTimestamp(Number.NaN), '—');
});
