// Pure formatting helpers shared by the PWA views.
// Mirrors src/domain/money.rs so offline previews match server calculations.

const IST = 'Asia/Kolkata';

export function groupIndian(digits) {
  if (digits.length <= 3) return digits;
  const head = digits.slice(0, -3);
  const tail = digits.slice(-3);
  const parts = [];
  let end = head.length;
  while (end > 2) {
    parts.unshift(head.slice(end - 2, end));
    end -= 2;
  }
  if (end > 0) parts.unshift(head.slice(0, end));
  return `${parts.join(',')},${tail}`;
}

/** Integer paise -> "₹1,23,456.50" (Indian grouping). */
export function formatPaise(paise, { symbol = true } = {}) {
  if (!Number.isSafeInteger(paise)) return symbol ? '₹—' : '—';
  const sign = paise < 0 ? '-' : '';
  const abs = Math.abs(paise);
  const units = Math.floor(abs / 100);
  const fraction = String(abs % 100).padStart(2, '0');
  return `${symbol ? '₹' : ''}${sign}${groupIndian(String(units))}.${fraction}`;
}

/** Integer paise -> "1234.56", for CSV-like contexts. */
export function formatDecimal(paise) {
  if (!Number.isSafeInteger(paise)) return '';
  const sign = paise < 0 ? '-' : '';
  const abs = Math.abs(paise);
  return `${sign}${Math.floor(abs / 100)}.${String(abs % 100).padStart(2, '0')}`;
}

/**
 * Parse a rupee string typed by a user into integer paise.
 * Accepts "1,234.5", "₹1234", " 12 ". Returns null for anything else.
 * Negative values are rejected (the API only accepts non-negative amounts).
 */
export function parsePaise(input) {
  if (input === null || input === undefined) return null;
  const cleaned = String(input).replace(/[₹,\s]/g, '');
  if (!/^\d+(\.\d{0,2})?$/.test(cleaned)) return null;
  const [whole, fraction = ''] = cleaned.split('.');
  const digits = whole.replace(/^0+(?=\d)/, '') + fraction.padEnd(2, '0');
  const value = Number(digits);
  return Number.isSafeInteger(value) ? value : null;
}

function istParts(date = new Date()) {
  const parts = new Intl.DateTimeFormat('en-GB', {
    timeZone: IST,
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  }).formatToParts(date);
  const pick = (type) => parts.find((part) => part.type === type)?.value ?? '';
  return { year: pick('year'), month: pick('month'), day: pick('day'), hour: pick('hour'), minute: pick('minute') };
}

/** Today's civil date in the shop timezone (IST), as YYYY-MM-DD. */
export function isoToday() {
  try {
    const { year, month, day } = istParts();
    return `${year}-${month}-${day}`;
  } catch {
    const now = new Date();
    const local = new Date(now.getTime() - now.getTimezoneOffset() * 60_000);
    return local.toISOString().slice(0, 10);
  }
}

export function currentMonth() {
  return isoToday().slice(0, 7);
}

/** YYYY-MM-DD -> DD-MM-YYYY. */
export function displayDate(iso) {
  if (typeof iso !== 'string' || !/^\d{4}-\d{2}-\d{2}$/.test(iso)) return String(iso ?? '');
  const [year, month, day] = iso.split('-');
  return `${day}-${month}-${year}`;
}

export function addDays(iso, delta) {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(String(iso));
  if (!match) return iso;
  const date = new Date(Date.UTC(Number(match[1]), Number(match[2]) - 1, Number(match[3]) + delta));
  return date.toISOString().slice(0, 10);
}

/** "2026-11" -> {from:"2026-11-01", to:"2026-11-30"}. */
export function monthBounds(month) {
  const match = /^(\d{4})-(\d{2})$/.exec(String(month ?? ''));
  if (!match) return null;
  const year = Number(match[1]);
  const monthIndex = Number(match[2]) - 1;
  if (monthIndex < 0 || monthIndex > 11) return null;
  const from = new Date(Date.UTC(year, monthIndex, 1));
  const to = new Date(Date.UTC(year, monthIndex + 1, 0));
  return { from: from.toISOString().slice(0, 10), to: to.toISOString().slice(0, 10) };
}

/** Inclusive day count for two ISO dates, or null when invalid/out of order. */
export function daysInRange(from, to) {
  if (typeof from !== 'string' || typeof to !== 'string') return null;
  const matchFrom = /^(\d{4})-(\d{2})-(\d{2})$/.exec(from);
  const matchTo = /^(\d{4})-(\d{2})-(\d{2})$/.exec(to);
  if (!matchFrom || !matchTo) return null;
  const start = Date.UTC(Number(matchFrom[1]), Number(matchFrom[2]) - 1, Number(matchFrom[3]));
  const end = Date.UTC(Number(matchTo[1]), Number(matchTo[2]) - 1, Number(matchTo[3]));
  const days = Math.round((end - start) / 86_400_000) + 1;
  return days >= 1 ? days : null;
}

/** Milliseconds since epoch -> "23-11-2026 14:05" in IST. */
export function formatTimestamp(ms) {
  if (!Number.isFinite(ms)) return '—';
  try {
    const { year, month, day, hour, minute } = istParts(new Date(ms));
    return `${day}-${month}-${year} ${hour}:${minute}`;
  } catch {
    return '—';
  }
}
