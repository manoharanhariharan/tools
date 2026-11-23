// Pure day-total math. The server is authoritative; this mirrors it for the
// offline preview on the entry form.

export function computeTotals(lines, adjustments = {}) {
  const business = lines.reduce((sum, line) => sum + (Number(line.amount_minor) || 0), 0);
  const bonus = Number(adjustments.bonus_minor) || 0;
  const advance = Number(adjustments.advance_minor) || 0;
  const deduction = Number(adjustments.deduction_minor) || 0;
  const gross = business + bonus;
  return {
    business_minor: business,
    bonus_minor: bonus,
    gross_minor: gross,
    advance_minor: advance,
    deduction_minor: deduction,
    net_minor: gross - advance - deduction,
  };
}

export function lineDetail(line) {
  const text = `${line.type_name} ${(line.amount_minor / 100).toFixed(2)}`;
  return line.description ? `${text} — ${line.description}` : text;
}
