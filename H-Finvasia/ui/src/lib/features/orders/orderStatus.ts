/** Order status that means the order can no longer fill. */
const CLOSED = new Set([
  "COMPLETE",
  "COMPLETED",
  "REJECTED",
  "CANCELLED",
  "CANCELED",
  "LAPSED",
  "EXPIRED",
]);

/**
 * Shoonya has used both spellings (`CANCELED`, `CANCELLED`) and different
 * separators over time. Normalise before deciding which tab an order belongs
 * in, otherwise a cancelled order stays in Pending forever.
 */
export function isClosedOrder(status: string): boolean {
  const normalized = status
    .trim()
    .toUpperCase()
    .replace(/[\s-]+/g, "_");
  return CLOSED.has(normalized);
}
