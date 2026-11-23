//! Pay rules and the authoritative money calculation.
//!
//! Clients may propose amounts, but the server always recomputes from the
//! employee's pay rule and the raw inputs (minutes, fraction, overrides). This
//! keeps offline clients honest and keeps rounding consistent everywhere.

use serde::{Deserialize, Serialize};

use crate::domain::money::{MAX_AMOUNT_MINOR, round_div_half_up};
use crate::error::FieldError;

pub const DEFAULT_OT_MULTIPLIER_BP: i64 = 15_000;
pub const MIN_OT_MULTIPLIER_BP: i64 = 10_000;
pub const MAX_OT_MULTIPLIER_BP: i64 = 30_000;
pub const FULL_DAY_BP: i64 = 10_000;
pub const MAX_DAY_FRACTION_BP: i64 = 20_000;
pub const MAX_MINUTES_PER_DAY: i64 = 1_440;
pub const DEFAULT_STANDARD_MINUTES: i64 = 480;
pub const MIN_STANDARD_MINUTES: i64 = 60;
pub const MAX_STANDARD_MINUTES: i64 = 1_440;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PayBasis {
    DayRate,
    Hourly,
    Fixed,
}

impl PayBasis {
    pub const fn as_str(self) -> &'static str {
        match self {
            PayBasis::DayRate => "day_rate",
            PayBasis::Hourly => "hourly",
            PayBasis::Fixed => "fixed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "day_rate" => Some(PayBasis::DayRate),
            "hourly" => Some(PayBasis::Hourly),
            "fixed" => Some(PayBasis::Fixed),
            _ => None,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            PayBasis::DayRate => "Day rate",
            PayBasis::Hourly => "Hourly",
            PayBasis::Fixed => "Fixed per entry",
        }
    }
}

/// The employee-level rule used at entry save time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PayRule {
    pub basis: PayBasis,
    pub rate_minor: i64,
    pub ot_multiplier_bp: i64,
    pub standard_minutes_per_day: i64,
}

/// Raw, client-supplied entry inputs. Amounts derived from these are always
/// recomputed on the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntryAmounts {
    pub day_fraction_bp: i64,
    pub regular_minutes: i64,
    pub overtime_minutes: i64,
    pub ot_multiplier_bp: Option<i64>,
    pub override_amount_minor: Option<i64>,
    pub bonus_minor: i64,
    pub advance_minor: i64,
    pub deduction_minor: i64,
}

impl Default for EntryAmounts {
    fn default() -> Self {
        Self {
            day_fraction_bp: FULL_DAY_BP,
            regular_minutes: 0,
            overtime_minutes: 0,
            ot_multiplier_bp: None,
            override_amount_minor: None,
            bonus_minor: 0,
            advance_minor: 0,
            deduction_minor: 0,
        }
    }
}

impl EntryAmounts {
    /// Drop fields that do not apply to the employee's pay basis. Callers must
    /// normalize before validating so a client that sends irrelevant defaults
    /// is not punished, while truly inconsistent values still fail validation.
    pub fn normalized_for(mut self, basis: PayBasis) -> Self {
        match basis {
            PayBasis::DayRate => {
                self.regular_minutes = 0;
            }
            PayBasis::Hourly => {
                self.day_fraction_bp = FULL_DAY_BP;
            }
            PayBasis::Fixed => {
                self.day_fraction_bp = FULL_DAY_BP;
                self.regular_minutes = 0;
                self.overtime_minutes = 0;
                self.ot_multiplier_bp = None;
            }
        }
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Computed {
    pub base_minor: i64,
    pub ot_minor: i64,
    pub gross_minor: i64,
    pub net_minor: i64,
}

/// Compute base pay, overtime, gross and net for one employee-day.
///
/// Rounding is applied once per component, half-up, in minor units.
pub fn compute(rule: &PayRule, input: &EntryAmounts) -> Result<Computed, Vec<FieldError>> {
    let mut errors = Vec::new();

    check_minutes("day_fraction_bp", input.day_fraction_bp, MAX_DAY_FRACTION_BP, &mut errors);
    check_minutes(
        "regular_minutes",
        input.regular_minutes,
        MAX_MINUTES_PER_DAY,
        &mut errors,
    );
    check_minutes(
        "overtime_minutes",
        input.overtime_minutes,
        MAX_MINUTES_PER_DAY,
        &mut errors,
    );

    for (field, value) in [
        ("override_amount_minor", input.override_amount_minor),
        ("bonus_minor", Some(input.bonus_minor)),
        ("advance_minor", Some(input.advance_minor)),
        ("deduction_minor", Some(input.deduction_minor)),
    ] {
        if let Some(value) = value {
            if !(0..=MAX_AMOUNT_MINOR).contains(&value) {
                errors.push(FieldError::new(
                    field,
                    format!("must be between 0 and {MAX_AMOUNT_MINOR} minor units"),
                ));
            }
        }
    }

    let multiplier = input.ot_multiplier_bp.unwrap_or(rule.ot_multiplier_bp);
    if input.ot_multiplier_bp.is_some()
        && !(MIN_OT_MULTIPLIER_BP..=MAX_OT_MULTIPLIER_BP).contains(&multiplier)
    {
        errors.push(FieldError::new(
            "ot_multiplier_bp",
            format!(
                "must be between {MIN_OT_MULTIPLIER_BP} and {MAX_OT_MULTIPLIER_BP} (basis points)"
            ),
        ));
    }

    match rule.basis {
        PayBasis::DayRate => {
            if input.regular_minutes != 0 {
                errors.push(FieldError::new(
                    "regular_minutes",
                    "not used for day-rate employees",
                ));
            }
        }
        PayBasis::Hourly => {
            if input.day_fraction_bp != FULL_DAY_BP {
                errors.push(FieldError::new(
                    "day_fraction_bp",
                    "not used for hourly employees",
                ));
            }
        }
        PayBasis::Fixed => {
            if input.overtime_minutes != 0 {
                errors.push(FieldError::new(
                    "overtime_minutes",
                    "overtime is not paid for fixed-per-entry employees; use a bonus",
                ));
            }
        }
    }

    if rule.rate_minor < 0 || rule.rate_minor > MAX_AMOUNT_MINOR {
        errors.push(FieldError::new("rate_minor", "employee rate is out of range"));
    }
    if !(MIN_STANDARD_MINUTES..=MAX_STANDARD_MINUTES).contains(&rule.standard_minutes_per_day) {
        errors.push(FieldError::new(
            "standard_minutes_per_day",
            "employee standard day is out of range",
        ));
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    let rate = rule.rate_minor as i128;
    let override_amount = input.override_amount_minor;

    let base: i64 = match rule.basis {
        PayBasis::Fixed => override_amount.unwrap_or(rule.rate_minor),
        PayBasis::DayRate => override_amount.unwrap_or_else(|| {
            round_div_half_up(rate * input.day_fraction_bp as i128, FULL_DAY_BP as i128)
        }),
        PayBasis::Hourly => override_amount
            .unwrap_or_else(|| round_div_half_up(rate * input.regular_minutes as i128, 60)),
    };

    let ot: i64 = if input.overtime_minutes == 0 || rule.basis == PayBasis::Fixed {
        0
    } else {
        let minutes = input.overtime_minutes as i128;
        let multiplier = multiplier as i128;
        match rule.basis {
            PayBasis::DayRate => round_div_half_up(
                rate * minutes * multiplier,
                rule.standard_minutes_per_day as i128 * 10_000,
            ),
            PayBasis::Hourly => round_div_half_up(rate * minutes * multiplier, 60 * 10_000),
            PayBasis::Fixed => 0,
        }
    };

    let bonus = input.bonus_minor as i128;
    let advance = input.advance_minor as i128;
    let deduction = input.deduction_minor as i128;

    let gross = base as i128 + ot as i128 + bonus;
    let net = gross - advance - deduction;

    Ok(Computed {
        base_minor: base,
        ot_minor: ot,
        gross_minor: clamp_money(gross),
        net_minor: clamp_money(net),
    })
}

fn clamp_money(value: i128) -> i64 {
    value.clamp(i64::MIN as i128, i64::MAX as i128) as i64
}

fn check_minutes(field: &str, value: i64, max: i64, errors: &mut Vec<FieldError>) {
    if !(0..=max).contains(&value) {
        errors.push(FieldError::new(field, format!("must be between 0 and {max}")));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day_rate(rate: i64) -> PayRule {
        PayRule {
            basis: PayBasis::DayRate,
            rate_minor: rate,
            ot_multiplier_bp: DEFAULT_OT_MULTIPLIER_BP,
            standard_minutes_per_day: DEFAULT_STANDARD_MINUTES,
        }
    }

    fn hourly(rate: i64) -> PayRule {
        PayRule {
            basis: PayBasis::Hourly,
            rate_minor: rate,
            ot_multiplier_bp: DEFAULT_OT_MULTIPLIER_BP,
            standard_minutes_per_day: DEFAULT_STANDARD_MINUTES,
        }
    }

    fn fixed(rate: i64) -> PayRule {
        PayRule {
            basis: PayBasis::Fixed,
            rate_minor: rate,
            ot_multiplier_bp: DEFAULT_OT_MULTIPLIER_BP,
            standard_minutes_per_day: DEFAULT_STANDARD_MINUTES,
        }
    }

    fn amounts() -> EntryAmounts {
        EntryAmounts::default()
    }

    #[test]
    fn day_rate_full_and_half_day() {
        let full = compute(&day_rate(80_000), &amounts()).unwrap();
        assert_eq!(full.base_minor, 80_000);
        assert_eq!(full.net_minor, 80_000);

        let half = amounts().normalized_for(PayBasis::DayRate);
        let half = EntryAmounts {
            day_fraction_bp: 5_000,
            ..half
        };
        assert_eq!(compute(&day_rate(80_000), &half).unwrap().base_minor, 40_000);
    }

    #[test]
    fn day_rate_overtime_uses_standard_day() {
        let input = EntryAmounts {
            overtime_minutes: 120,
            ..amounts().normalized_for(PayBasis::DayRate)
        };
        // 800 * (120 / 480) * 1.5 = 300
        let result = compute(&day_rate(80_000), &input).unwrap();
        assert_eq!(result.ot_minor, 30_000);
        assert_eq!(result.gross_minor, 110_000);
    }

    #[test]
    fn hourly_base_and_overtime() {
        let input = EntryAmounts {
            regular_minutes: 450,
            overtime_minutes: 90,
            ..amounts().normalized_for(PayBasis::Hourly)
        };
        // 150 * 7.5 = 1125; OT 150 * 1.5h * 1.5 = 337.50
        let result = compute(&hourly(15_000), &input).unwrap();
        assert_eq!(result.base_minor, 112_500);
        assert_eq!(result.ot_minor, 33_750);
        assert_eq!(result.gross_minor, 146_250);
    }

    #[test]
    fn hourly_rounding_is_half_up_and_single_step() {
        // 101 paise/hour for 30 min = 50.5 -> 51
        let input = EntryAmounts {
            regular_minutes: 30,
            ..amounts()
        };
        assert_eq!(compute(&hourly(101), &input).unwrap().base_minor, 51);
    }

    #[test]
    fn fixed_basis_uses_rate_or_override() {
        let base = compute(&fixed(70_000), &amounts()).unwrap();
        assert_eq!(base.base_minor, 70_000);

        let with_override = compute(
            &fixed(70_000),
            &EntryAmounts {
                override_amount_minor: Some(95_000),
                ..amounts()
            },
        )
        .unwrap();
        assert_eq!(with_override.base_minor, 95_000);
    }

    #[test]
    fn entry_override_replaces_base_for_any_basis() {
        let result = compute(
            &day_rate(80_000),
            &EntryAmounts {
                override_amount_minor: Some(123_456),
                ..amounts().normalized_for(PayBasis::DayRate)
            },
        )
        .unwrap();
        assert_eq!(result.base_minor, 123_456);
    }

    #[test]
    fn bonus_advance_and_deduction_flow_into_net() {
        let input = EntryAmounts {
            bonus_minor: 10_000,
            advance_minor: 20_000,
            deduction_minor: 5_000,
            ..amounts().normalized_for(PayBasis::DayRate)
        };
        let result = compute(&day_rate(80_000), &input).unwrap();
        assert_eq!(result.gross_minor, 90_000);
        assert_eq!(result.net_minor, 65_000);
    }

    #[test]
    fn advance_may_exceed_gross_resulting_in_negative_net() {
        let input = EntryAmounts {
            advance_minor: 100_000,
            ..amounts().normalized_for(PayBasis::DayRate)
        };
        assert_eq!(compute(&day_rate(80_000), &input).unwrap().net_minor, -20_000);
    }

    #[test]
    fn rejects_invalid_basis_specific_inputs() {
        let bad = EntryAmounts {
            regular_minutes: 60,
            ..amounts()
        };
        let errors = compute(&day_rate(80_000), &bad).unwrap_err();
        assert!(errors.iter().any(|e| e.field == "regular_minutes"));

        let hourly_bad = EntryAmounts {
            day_fraction_bp: 5_000,
            ..amounts()
        };
        assert!(
            compute(&hourly(10_000), &hourly_bad)
                .unwrap_err()
                .iter()
                .any(|e| e.field == "day_fraction_bp")
        );

        let fixed_bad = EntryAmounts {
            overtime_minutes: 60,
            ..amounts()
        };
        assert!(
            compute(&fixed(10_000), &fixed_bad)
                .unwrap_err()
                .iter()
                .any(|e| e.field == "overtime_minutes")
        );
    }

    #[test]
    fn rejects_out_of_range_values() {
        let input = EntryAmounts {
            overtime_minutes: 2_000,
            bonus_minor: MAX_AMOUNT_MINOR + 1,
            ..amounts()
        };
        let errors = compute(&day_rate(80_000), &input).unwrap_err();
        assert!(errors.iter().any(|e| e.field == "overtime_minutes"));
        assert!(errors.iter().any(|e| e.field == "bonus_minor"));
    }

    #[test]
    fn normalization_drops_irrelevant_fields() {
        let normalized = EntryAmounts {
            regular_minutes: 300,
            day_fraction_bp: 5_000,
            ..amounts()
        }
        .normalized_for(PayBasis::DayRate);
        assert_eq!(normalized.regular_minutes, 0);
        assert_eq!(normalized.day_fraction_bp, 5_000);

        let normalized = EntryAmounts {
            day_fraction_bp: 5_000,
            ..amounts()
        }
        .normalized_for(PayBasis::Hourly);
        assert_eq!(normalized.day_fraction_bp, FULL_DAY_BP);
    }
}
