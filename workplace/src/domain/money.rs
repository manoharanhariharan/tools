//! Integer money arithmetic in minor units (paise for INR).
//!
//! The API accepts and returns only integer minor units; rupee strings exist
//! solely for display/export so that no binary floating point ever touches a
//! payroll amount.

/// Upper bound for a single amount/rate: ₹100 crore in paise. Guards against
/// accidental extra zeros and keeps all computations comfortably inside i64.
pub const MAX_AMOUNT_MINOR: i64 = 100_000_000_000;

/// Divide non-negative values, rounding halves away from zero.
pub fn round_div_half_up(num: i128, den: i128) -> i64 {
    debug_assert!(num >= 0, "money arithmetic is non-negative");
    debug_assert!(den > 0, "denominator must be positive");
    let rounded = (num + den / 2) / den;
    i64::try_from(rounded).unwrap_or(i64::MAX)
}

/// `1234567` -> `"12345.67"` (plain decimal, CSV/spreadsheet friendly).
pub fn format_decimal(minor: i64) -> String {
    let sign = if minor < 0 { "-" } else { "" };
    let abs = minor.unsigned_abs();
    format!("{sign}{}.{:02}", abs / 100, abs % 100)
}

/// `123456789` -> `"₹12,34,567.89"` using Indian digit grouping.
pub fn format_inr(minor: i64) -> String {
    format!("₹{}", format_grouped(minor))
}

/// Same as [`format_inr`] but without the currency symbol, for PDF fallback
/// fonts that lack U+20B9.
pub fn format_grouped(minor: i64) -> String {
    let sign = if minor < 0 { "-" } else { "" };
    let abs = minor.unsigned_abs();
    let units = (abs / 100).to_string();
    let paise = abs % 100;
    format!("{sign}{}.{paise:02}", group_indian_digits(&units))
}

/// Group digits the Indian way: last three, then pairs.
/// e.g. `1234567` -> `12,34,567`.
pub fn group_indian_digits(digits: &str) -> String {
    if digits.len() <= 3 {
        return digits.to_string();
    }
    let split_at = digits.len() - 3;
    let (head, tail) = digits.split_at(split_at);

    let mut parts: Vec<&str> = Vec::new();
    let mut end = head.len();
    while end > 2 {
        parts.push(&head[end - 2..end]);
        end -= 2;
    }
    if end > 0 {
        parts.push(&head[..end]);
    }
    parts.reverse();
    format!("{},{}", parts.join(","), tail)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_half_up() {
        assert_eq!(round_div_half_up(0, 60), 0);
        assert_eq!(round_div_half_up(30, 60), 1);
        assert_eq!(round_div_half_up(29, 60), 0);
        assert_eq!(round_div_half_up(90, 60), 2);
    }

    #[test]
    fn groups_digits_indian_style() {
        assert_eq!(group_indian_digits("1"), "1");
        assert_eq!(group_indian_digits("999"), "999");
        assert_eq!(group_indian_digits("1000"), "1,000");
        assert_eq!(group_indian_digits("10000"), "10,000");
        assert_eq!(group_indian_digits("100000"), "1,00,000");
        assert_eq!(group_indian_digits("1234567"), "12,34,567");
        assert_eq!(group_indian_digits("10000000"), "1,00,00,000");
    }

    #[test]
    fn formats_decimal_amounts() {
        assert_eq!(format_decimal(0), "0.00");
        assert_eq!(format_decimal(5), "0.05");
        assert_eq!(format_decimal(123456), "1234.56");
        assert_eq!(format_decimal(-123456), "-1234.56");
    }

    #[test]
    fn formats_inr_with_symbol_and_grouping() {
        assert_eq!(format_inr(80000), "₹800.00");
        assert_eq!(format_inr(123456789), "₹12,34,567.89");
        assert_eq!(format_inr(-500), "₹-5.00");
        assert_eq!(format_grouped(123456789), "12,34,567.89");
    }
}
