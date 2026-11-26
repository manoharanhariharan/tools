//! Fixed-point money.
//!
//! Prices arrive from the feed as strings such as `"1287.30"` (2 dp) but implied
//! volatility uses up to 6 dp, so the internal scale is 6 decimal places.
//! Six dp also covers every exchange tick size observed in the legacy master
//! (`0.05`, `0.01`, `0.0001` for CDS).
//!
//! Never use `f64` for money: the legacy terminal's `PriceConvertor` performed
//! paisa↔rupee float round-trips and the resulting drift is a known source of
//! display bugs.

use crate::CoreError;

/// Number of decimal places carried internally.
pub const SCALE_DIGITS: u32 = 6;
/// Multiplier between a rupee and the internal unit.
pub const SCALE: i64 = 1_000_000;

/// A fixed-point price with 6 decimal places of precision.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(transparent)]
pub struct Price(i64);

impl Price {
    pub const ZERO: Price = Price(0);

    /// Wrap a raw internally-scaled value.
    pub const fn from_scaled(value: i64) -> Self {
        Price(value)
    }

    /// The raw internally-scaled value.
    pub const fn scaled(self) -> i64 {
        self.0
    }

    /// Build from whole paisa (1 rupee = 100 paisa).
    pub const fn from_paisa(paisa: i64) -> Self {
        Price(paisa.saturating_mul(10_000))
    }

    /// Whole-paisa value, rounded half away from zero.
    pub fn to_paisa(self) -> i64 {
        self.round_to(2).0 / 10_000
    }

    /// Strict parse: rejects more than 6 fractional digits.
    pub fn parse(input: &str) -> Result<Self, CoreError> {
        parse_impl(input, false).map(Price)
    }

    /// Lenient parse: rounds excess fractional digits (used for feed floats
    /// such as `1287.3000000000002` that JSON number round-trips produce).
    pub fn parse_rounded(input: &str) -> Result<Self, CoreError> {
        parse_impl(input, true).map(Price)
    }

    /// Round to `decimals` places, half away from zero.
    pub fn round_to(self, decimals: u8) -> Self {
        let d = decimals.min(SCALE_DIGITS as u8) as u32;
        let pow = 10i64.pow(SCALE_DIGITS - d);
        let abs = self.0.unsigned_abs();
        let half = (pow / 2) as u64;
        let rounded = ((abs + half) / pow as u64) * pow as u64;
        let out = if self.0 < 0 {
            -(rounded as i64)
        } else {
            rounded as i64
        };
        Price(out)
    }

    /// Render with exactly `decimals` decimal places (capped at 6).
    pub fn format(self, decimals: u8) -> String {
        let d = decimals.min(SCALE_DIGITS as u8) as u32;
        let pow = 10i128.pow(SCALE_DIGITS - d);
        let abs = (self.0 as i128).abs();
        let rounded = (abs + pow / 2) / pow;
        let mut digits = rounded.to_string();
        if d > 0 {
            let d = d as usize;
            if digits.len() <= d {
                digits = format!("{}{}", "0".repeat(d + 1 - digits.len()), digits);
            }
            let split = digits.len() - d;
            digits.insert(split, '.');
        }
        if self.0 < 0 && rounded != 0 {
            format!("-{digits}")
        } else {
            digits
        }
    }

    pub fn checked_add(self, other: Price) -> Option<Price> {
        self.0.checked_add(other.0).map(Price)
    }

    pub fn checked_sub(self, other: Price) -> Option<Price> {
        self.0.checked_sub(other.0).map(Price)
    }

    pub fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// Display-only helper. Never use the result for arithmetic.
    pub fn as_f64(self) -> f64 {
        self.0 as f64 / SCALE as f64
    }
}

impl std::str::FromStr for Price {
    type Err = CoreError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Price::parse(s)
    }
}

fn parse_impl(input: &str, allow_round: bool) -> Result<i64, CoreError> {
    let s = input.trim();
    if s.is_empty() {
        return Err(CoreError::InvalidDecimal(input.to_string()));
    }

    let (negative, body) = match s.as_bytes()[0] {
        b'-' => (true, &s[1..]),
        b'+' => (false, &s[1..]),
        _ => (false, s),
    };
    if body.is_empty() {
        return Err(CoreError::InvalidDecimal(input.to_string()));
    }

    let mut parts = body.splitn(2, '.');
    let int_part = parts.next().unwrap_or("");
    let frac_part = parts.next().unwrap_or("");

    if int_part.is_empty() && frac_part.is_empty() {
        return Err(CoreError::InvalidDecimal(input.to_string()));
    }
    if !int_part.bytes().all(|b| b.is_ascii_digit()) {
        return Err(CoreError::InvalidDecimal(input.to_string()));
    }
    if !frac_part.bytes().all(|b| b.is_ascii_digit()) {
        return Err(CoreError::InvalidDecimal(input.to_string()));
    }

    let int_val: i128 = if int_part.is_empty() {
        0
    } else {
        int_part
            .parse::<i128>()
            .map_err(|_| CoreError::InvalidDecimal(input.to_string()))?
    };

    let mut frac = frac_part.to_string();
    let mut round_up = false;
    if frac.len() > SCALE_DIGITS as usize {
        if !allow_round {
            return Err(CoreError::InvalidDecimal(input.to_string()));
        }
        round_up = frac.as_bytes()[SCALE_DIGITS as usize] >= b'5';
        frac.truncate(SCALE_DIGITS as usize);
    }
    while frac.len() < SCALE_DIGITS as usize {
        frac.push('0');
    }
    let frac_val: i128 = if frac.is_empty() {
        0
    } else {
        frac.parse::<i128>()
            .map_err(|_| CoreError::InvalidDecimal(input.to_string()))?
    };

    let mut magnitude = int_val
        .checked_mul(SCALE as i128)
        .and_then(|v| v.checked_add(frac_val))
        .ok_or_else(|| CoreError::InvalidDecimal(input.to_string()))?;
    if round_up {
        magnitude += 1;
    }
    if negative {
        magnitude = -magnitude;
    }
    let clamped = magnitude.clamp(i64::MIN as i128, i64::MAX as i128);
    Ok(clamped as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_two_place_prices_exactly() {
        assert_eq!(Price::parse("1287.30").unwrap().scaled(), 1_287_300_000);
        assert_eq!(Price::parse("0.05").unwrap().scaled(), 50_000);
        assert_eq!(Price::parse("260").unwrap().scaled(), 260_000_000);
    }

    #[test]
    fn parses_six_place_iv() {
        assert_eq!(Price::parse("0.123456").unwrap().scaled(), 123_456);
    }

    #[test]
    fn rejects_excess_precision_in_strict_mode() {
        assert!(Price::parse("1.2345678").is_err());
    }

    #[test]
    fn rounds_excess_precision_in_lenient_mode() {
        assert_eq!(
            Price::parse_rounded("1.2345678").unwrap().scaled(),
            1_234_568
        );
        assert_eq!(
            Price::parse_rounded("1287.3000000000002").unwrap().scaled(),
            1_287_300_000
        );
    }

    #[test]
    fn handles_negative_and_leading_dot() {
        assert_eq!(Price::parse("-2.5").unwrap().scaled(), -2_500_000);
        assert_eq!(Price::parse(".5").unwrap().scaled(), 500_000);
    }

    #[test]
    fn formats_with_requested_precision() {
        let p = Price::parse("1287.3").unwrap();
        assert_eq!(p.format(2), "1287.30");
        assert_eq!(p.format(0), "1287");
        assert_eq!(Price::parse("0.123456").unwrap().format(6), "0.123456");
        assert_eq!(Price::ZERO.format(2), "0.00");
        assert_eq!(Price::parse("-2.5").unwrap().format(2), "-2.50");
    }

    #[test]
    fn rounds_half_away_from_zero() {
        assert_eq!(Price::parse("1.005").unwrap().format(2), "1.01");
        assert_eq!(Price::parse("-1.005").unwrap().format(2), "-1.01");
    }

    #[test]
    fn paisa_round_trip() {
        let p = Price::from_paisa(128_730);
        assert_eq!(p.format(2), "1287.30");
        assert_eq!(p.to_paisa(), 128_730);
    }

    #[test]
    fn rejects_junk() {
        for bad in ["", " ", "abc", "1.2.3", "1,5", "--1"] {
            assert!(Price::parse(bad).is_err(), "should reject {bad:?}");
        }
    }
}
