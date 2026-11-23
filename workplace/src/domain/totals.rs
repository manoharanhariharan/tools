//! Pure day-total math: business generated across earning types, plus the
//! adjustments that produce the final money to present.

use serde::Serialize;

use crate::domain::money::MAX_AMOUNT_MINOR;
use crate::error::FieldError;

pub const MAX_LINES: usize = 50;
pub const MAX_DESCRIPTION_LEN: usize = 300;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineAmount {
    pub type_id: String,
    pub description: String,
    pub amount_minor: i64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct DayTotals {
    pub business_minor: i64,
    pub bonus_minor: i64,
    pub gross_minor: i64,
    pub advance_minor: i64,
    pub deduction_minor: i64,
    pub net_minor: i64,
}

/// Validate and total one employee-day.
///
/// `business = Σ line amounts`; `gross = business + bonus`;
/// `net = gross − advance − deduction`. All amounts are non-negative paise;
/// net may be negative when advances exceed gross.
pub fn compute(
    lines: &[LineAmount],
    bonus_minor: i64,
    advance_minor: i64,
    deduction_minor: i64,
) -> Result<DayTotals, Vec<FieldError>> {
    let mut errors = Vec::new();

    if lines.is_empty() {
        errors.push(FieldError::new(
            "lines",
            "add at least one earning type with an amount",
        ));
    }
    if lines.len() > MAX_LINES {
        errors.push(FieldError::new(
            "lines",
            format!("at most {MAX_LINES} earning rows are allowed"),
        ));
    }

    let mut seen: Vec<&str> = Vec::with_capacity(lines.len());
    let mut business: i64 = 0;
    for line in lines {
        if line.type_id.trim().is_empty() {
            errors.push(FieldError::new("lines", "every row needs an earning type"));
        } else if seen.iter().any(|existing| *existing == line.type_id) {
            errors.push(FieldError::new(
                "lines",
                "each earning type may appear only once per day",
            ));
        } else {
            seen.push(&line.type_id);
        }

        if !(0..=MAX_AMOUNT_MINOR).contains(&line.amount_minor) {
            errors.push(FieldError::new(
                "amount_minor",
                format!("must be between 0 and {MAX_AMOUNT_MINOR} paise"),
            ));
        } else {
            business = business.saturating_add(line.amount_minor);
        }

        if line.description.chars().count() > MAX_DESCRIPTION_LEN {
            errors.push(FieldError::new(
                "description",
                format!("must be at most {MAX_DESCRIPTION_LEN} characters"),
            ));
        }
        if line
            .description
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
        {
            errors.push(FieldError::new(
                "description",
                "must not contain control characters",
            ));
        }
    }

    for (field, value) in [
        ("bonus_minor", bonus_minor),
        ("advance_minor", advance_minor),
        ("deduction_minor", deduction_minor),
    ] {
        if !(0..=MAX_AMOUNT_MINOR).contains(&value) {
            errors.push(FieldError::new(
                field,
                format!("must be between 0 and {MAX_AMOUNT_MINOR} paise"),
            ));
        }
    }

    if business == 0 && errors.is_empty() {
        errors.push(FieldError::new(
            "lines",
            "enter at least one earning amount greater than zero",
        ));
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    let gross = business.saturating_add(bonus_minor);
    Ok(DayTotals {
        business_minor: business,
        bonus_minor,
        gross_minor: gross,
        advance_minor,
        deduction_minor,
        net_minor: gross
            .saturating_sub(advance_minor)
            .saturating_sub(deduction_minor),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(kind: &str, amount: i64) -> LineAmount {
        LineAmount {
            type_id: kind.into(),
            description: String::new(),
            amount_minor: amount,
        }
    }

    #[test]
    fn sums_business_and_adjustments() {
        let totals = compute(
            &[line("sales", 50_000), line("service", 30_000)],
            10_000,
            20_000,
            5_000,
        )
        .unwrap();
        assert_eq!(totals.business_minor, 80_000);
        assert_eq!(totals.gross_minor, 90_000);
        assert_eq!(totals.net_minor, 65_000);
    }

    #[test]
    fn allows_zero_valued_type_rows_but_requires_some_business() {
        assert!(compute(&[line("sales", 0)], 0, 0, 0).is_err());
        let totals = compute(&[line("sales", 0), line("service", 5_000)], 0, 0, 0).unwrap();
        assert_eq!(totals.business_minor, 5_000);
    }

    #[test]
    fn advance_may_exceed_gross() {
        let totals = compute(&[line("sales", 10_000)], 0, 25_000, 1_000).unwrap();
        assert_eq!(totals.net_minor, -16_000);
    }

    #[test]
    fn rejects_duplicate_types_and_bad_amounts() {
        let errors = compute(
            &[line("sales", 100), line("sales", 200), line("service", -1)],
            0,
            0,
            0,
        )
        .unwrap_err();
        assert!(errors.iter().any(|e| e.field == "lines"));
        assert!(errors.iter().any(|e| e.field == "amount_minor"));
    }

    #[test]
    fn rejects_blank_type_and_long_description() {
        let mut bad = line("", 100);
        bad.description = "x".repeat(MAX_DESCRIPTION_LEN + 1);
        let errors = compute(&[bad], 0, 0, 0).unwrap_err();
        assert!(errors.iter().any(|e| e.message.contains("earning type")));
        assert!(errors.iter().any(|e| e.field == "description"));
    }

    #[test]
    fn rejects_bad_adjustments() {
        let errors = compute(&[line("sales", 100)], MAX_AMOUNT_MINOR + 1, -1, 0).unwrap_err();
        assert!(errors.iter().any(|e| e.field == "bonus_minor"));
        assert!(errors.iter().any(|e| e.field == "advance_minor"));
    }
}
