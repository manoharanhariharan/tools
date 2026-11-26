//! Symbol identity.
//!
//! The feed addresses instruments as `EXCHANGE|TOKEN` (e.g. `NSE|22`, `NFO|158614`).
//! Tokens are **strings**, not integers: `FINVASIA_SYMBOL.token` is a string in
//! Mongo while `INSTRUMENT.instrument_token` is numeric, and treating them as
//! numbers loses the leading-digit distinction the vendor uses.

use crate::CoreError;
use std::fmt;

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct SymbolKey {
    exchange: String,
    token: String,
}

impl SymbolKey {
    /// Build from parts. The exchange is upper-cased; the token is trimmed but
    /// otherwise preserved verbatim.
    pub fn new(exchange: impl AsRef<str>, token: impl AsRef<str>) -> Result<Self, CoreError> {
        let exchange = exchange.as_ref().trim().to_ascii_uppercase();
        let token = token.as_ref().trim().to_string();
        let joined = format!("{exchange}|{token}");
        if exchange.is_empty() || token.is_empty() {
            return Err(CoreError::EmptySymbolPart(joined));
        }
        Ok(SymbolKey { exchange, token })
    }

    /// Parse the canonical `EXCHANGE|TOKEN` form.
    pub fn parse(s: &str) -> Result<Self, CoreError> {
        let (exchange, token) = s
            .split_once('|')
            .ok_or_else(|| CoreError::InvalidSymbolKey(s.to_string()))?;
        SymbolKey::new(exchange, token).map_err(|_| CoreError::InvalidSymbolKey(s.to_string()))
    }

    pub fn exchange(&self) -> &str {
        &self.exchange
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    /// Canonical wire form, suitable for `{"t":"t","k":"..."}` payloads.
    pub fn as_wire(&self) -> String {
        format!("{}|{}", self.exchange, self.token)
    }
}

impl fmt::Display for SymbolKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}|{}", self.exchange, self.token)
    }
}

impl std::str::FromStr for SymbolKey {
    type Err = CoreError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        SymbolKey::parse(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_wire_form() {
        let k = SymbolKey::parse("NSE|22").unwrap();
        assert_eq!(k.exchange(), "NSE");
        assert_eq!(k.token(), "22");
        assert_eq!(k.as_wire(), "NSE|22");
        assert_eq!(format!("{k}"), "NSE|22");
    }

    #[test]
    fn normalises_exchange_case() {
        let k = SymbolKey::parse("nfo|158614").unwrap();
        assert_eq!(k.as_wire(), "NFO|158614");
    }

    #[test]
    fn keeps_token_as_string() {
        // A token with a leading zero must not be coerced to a number.
        let k = SymbolKey::parse("BSE|000123").unwrap();
        assert_eq!(k.token(), "000123");
    }

    #[test]
    fn rejects_missing_separator_or_parts() {
        assert!(SymbolKey::parse("NSE22").is_err());
        assert!(SymbolKey::parse("|22").is_err());
        assert!(SymbolKey::parse("NSE|").is_err());
    }

    #[test]
    fn sorts_deterministically() {
        let mut v: Vec<SymbolKey> = ["NSE|22", "BSE|1", "NSE|11"]
            .iter()
            .map(|s| SymbolKey::parse(s).unwrap())
            .collect();
        v.sort();
        let wire: Vec<String> = v.iter().map(|k| k.as_wire()).collect();
        assert_eq!(wire, vec!["BSE|1", "NSE|11", "NSE|22"]);
    }
}
