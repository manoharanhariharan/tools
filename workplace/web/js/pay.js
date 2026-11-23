// Offline preview of the server's pay calculation.
// The server always recomputes and is authoritative; this module exists so the
// form can show accurate estimates while offline. Mirrors src/domain/pay.rs.

export const FULL_DAY_BP = 10000;
export const DEFAULT_OT_MULTIPLIER_BP = 15000;

function roundDivProduct(factors, denominator) {
  let numerator = 1n;
  for (const factor of factors) numerator *= BigInt(Math.trunc(factor));
  const den = BigInt(denominator);
  const result = (numerator + den / 2n) / den;
  return Number(result);
}

export function normalizeAmounts(basis, input = {}) {
  const amounts = {
    day_fraction_bp: input.day_fraction_bp ?? FULL_DAY_BP,
    regular_minutes: input.regular_minutes ?? 0,
    overtime_minutes: input.overtime_minutes ?? 0,
    ot_multiplier_bp: input.ot_multiplier_bp ?? null,
    override_amount_minor: input.override_amount_minor ?? null,
    bonus_minor: input.bonus_minor ?? 0,
    advance_minor: input.advance_minor ?? 0,
    deduction_minor: input.deduction_minor ?? 0,
  };
  if (basis === 'day_rate') {
    amounts.regular_minutes = 0;
  } else if (basis === 'hourly') {
    amounts.day_fraction_bp = FULL_DAY_BP;
  } else if (basis === 'fixed') {
    amounts.day_fraction_bp = FULL_DAY_BP;
    amounts.regular_minutes = 0;
    amounts.overtime_minutes = 0;
    amounts.ot_multiplier_bp = null;
  }
  return amounts;
}

export function computeEntry(employee, input = {}) {
  const basis = employee.pay_basis;
  const amounts = normalizeAmounts(basis, input);
  const rate = Number(employee.rate_minor) || 0;

  let base;
  if (basis === 'fixed') {
    base = amounts.override_amount_minor ?? rate;
  } else if (basis === 'day_rate') {
    base = amounts.override_amount_minor ?? roundDivProduct([rate, amounts.day_fraction_bp], 10000);
  } else {
    base = amounts.override_amount_minor ?? roundDivProduct([rate, amounts.regular_minutes], 60);
  }

  let overtime = 0;
  if (basis !== 'fixed' && amounts.overtime_minutes > 0) {
    const multiplier = amounts.ot_multiplier_bp ?? employee.ot_multiplier_bp ?? DEFAULT_OT_MULTIPLIER_BP;
    if (basis === 'day_rate') {
      const standard = employee.standard_minutes_per_day || 480;
      overtime = roundDivProduct([rate, amounts.overtime_minutes, multiplier], standard * 10000);
    } else {
      overtime = roundDivProduct([rate, amounts.overtime_minutes, multiplier], 60 * 10000);
    }
  }

  const gross = base + overtime + amounts.bonus_minor;
  const net = gross - amounts.advance_minor - amounts.deduction_minor;
  return {
    ...amounts,
    base_minor: base,
    ot_minor: overtime,
    gross_minor: gross,
    net_minor: net,
  };
}

/** Human summary for a basis, e.g. "₹800.00 / day". */
export function rateLabel(employee) {
  const amount = employee.rate_minor ?? 0;
  const formatted = `₹${(amount / 100).toFixed(2)}`;
  if (employee.pay_basis === 'hourly') return `${formatted} / hour`;
  if (employee.pay_basis === 'fixed') return `${formatted} / entry`;
  return `${formatted} / day`;
}

export function basisLabel(basis) {
  if (basis === 'hourly') return 'Hourly';
  if (basis === 'fixed') return 'Fixed per entry';
  return 'Day rate';
}
