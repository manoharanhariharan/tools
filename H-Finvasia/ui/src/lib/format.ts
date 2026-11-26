/** Display-only money formatting. Calculations stay on exact broker strings. */

const CLEAN = /[₹,\s]/g;

/** Parse a broker amount such as `"1,23,456.70"`, `"+1950.00"` or `" (500.00) "`. */
export function parseAmount(raw: string | number | null | undefined): number | null {
  if (raw === null || raw === undefined) return null;
  if (typeof raw === "number") return Number.isFinite(raw) ? raw : null;
  let text = raw.trim();
  if (text === "") return null;
  let negative = false;
  if (text.startsWith("(") && text.endsWith(")")) {
    negative = true;
    text = text.slice(1, -1);
  }
  text = text.replace(CLEAN, "");
  if (text.startsWith("+")) text = text.slice(1);
  const value = Number(text);
  if (!Number.isFinite(value)) return null;
  return negative ? -Math.abs(value) : value;
}

function groupIndian(digits: string): string {
  if (digits.length <= 3) return digits;
  const last = digits.slice(-3);
  let head = digits.slice(0, -3);
  const parts: string[] = [];
  while (head.length > 2) {
    parts.unshift(head.slice(-2));
    head = head.slice(0, -2);
  }
  if (head !== "") parts.unshift(head);
  return `${parts.join(",")},${last}`;
}

export interface AmountOptions {
  /** Add a `+` for positive values (P&L display). */
  signed?: boolean;
  /** Fraction digits, default 2. */
  decimals?: number;
}

/** Indian digit grouping: `12345678.9` -> `1,23,45,678.90`. */
export function formatIndianAmount(
  raw: string | number | null | undefined,
  options: AmountOptions = {},
): string {
  const value = parseAmount(raw);
  if (value === null) return "-";
  const decimals = Math.max(0, Math.min(6, options.decimals ?? 2));
  const fixed = Math.abs(value).toFixed(decimals);
  const [whole, fraction] = fixed.split(".");
  const body = groupIndian(whole) + (decimals > 0 ? `.${fraction}` : "");
  if (value < 0 && Number(fixed) !== 0) return `-${body}`;
  return options.signed && value > 0 ? `+${body}` : body;
}

export function amountTone(raw: string | number | null | undefined): "up" | "down" | "flat" {
  const value = parseAmount(raw);
  if (value === null || value === 0) return "flat";
  return value > 0 ? "up" : "down";
}
