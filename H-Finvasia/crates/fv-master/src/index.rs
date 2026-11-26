//! Portable, platform-neutral instrument index.
//!
//! Everything here is pure Rust with no database dependency, so it builds and
//! tests identically on Windows, macOS and Linux, and in CI without Mongo.

use fv_core::{Price, SymbolKey};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, thiserror::Error)]
pub enum MasterError {
    #[error("invalid symbol `{0}`")]
    InvalidSymbol(String),
    #[error("master is empty")]
    Empty,
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("mongo: {0}")]
    Mongo(String),
}

/// Optionability of an instrument.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum StrikeType {
    Call,
    Put,
    #[default]
    Other,
}

impl StrikeType {
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_uppercase().as_str() {
            "CE" | "CALL" => StrikeType::Call,
            "PE" | "PUT" => StrikeType::Put,
            _ => StrikeType::Other,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            StrikeType::Call => "CE",
            StrikeType::Put => "PE",
            StrikeType::Other => "XX",
        }
    }
}

/// One tradable instrument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instrument {
    pub key: SymbolKey,
    pub trading_symbol: String,
    pub name: Option<String>,
    pub underlying: Option<String>,
    /// `YYYY-MM-DD` where known.
    pub expiry: Option<String>,
    pub strike: Price,
    pub strike_type: StrikeType,
    pub lot_size: i64,
    /// `EQ`, `FUT`, `CE`, `PE`, `INDEX`, …
    pub instrument_type: String,
    pub segment: Option<String>,
    pub tick_size: Option<Price>,
}

impl Instrument {
    pub fn is_option(&self) -> bool {
        matches!(self.strike_type, StrikeType::Call | StrikeType::Put)
    }

    pub fn is_future(&self) -> bool {
        self.instrument_type.eq_ignore_ascii_case("FUT")
    }
}

/// A search result with its ranking signal, exposed for diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MatchRank {
    ExactSymbol = 0,
    SymbolPrefix = 1,
    SymbolContains = 2,
    UnderlyingPrefix = 3,
    NameContains = 4,
}

#[derive(Debug)]
pub struct MemoryMaster {
    items: Vec<Instrument>,
    by_key: HashMap<SymbolKey, usize>,
    by_underlying: HashMap<String, Vec<usize>>,
}

impl MemoryMaster {
    /// Build the index. Items are sorted deterministically so search results and
    /// serialized output never depend on database return order.
    pub fn build(mut items: Vec<Instrument>) -> Result<Self, MasterError> {
        if items.is_empty() {
            return Err(MasterError::Empty);
        }
        items.sort_by(|a, b| {
            a.trading_symbol
                .cmp(&b.trading_symbol)
                .then_with(|| a.key.cmp(&b.key))
        });

        let mut by_key = HashMap::with_capacity(items.len());
        let mut by_underlying: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, item) in items.iter().enumerate() {
            by_key.insert(item.key.clone(), i);
            if let Some(u) = item.underlying.as_deref().filter(|u| !u.is_empty()) {
                by_underlying
                    .entry(u.to_ascii_uppercase())
                    .or_default()
                    .push(i);
            }
        }

        Ok(MemoryMaster {
            items,
            by_key,
            by_underlying,
        })
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn get(&self, key: &SymbolKey) -> Option<&Instrument> {
        self.by_key.get(key).map(|&i| &self.items[i])
    }

    pub fn get_by_trading_symbol(&self, symbol: &str) -> Option<&Instrument> {
        let target = symbol.trim().to_ascii_uppercase();
        self.items
            .iter()
            .find(|i| i.trading_symbol.eq_ignore_ascii_case(&target))
    }

    /// Substring/prefix search over symbol, underlying and name.
    ///
    /// Deterministic: ranked, then alphabetically by trading symbol.
    pub fn search(&self, query: &str, limit: usize) -> Vec<(&Instrument, MatchRank)> {
        let q = query.trim().to_ascii_uppercase();
        if q.is_empty() || limit == 0 {
            return Vec::new();
        }

        let mut hits: Vec<(&Instrument, MatchRank)> = Vec::new();
        for item in &self.items {
            let sym = item.trading_symbol.to_ascii_uppercase();
            let rank = if sym == q {
                Some(MatchRank::ExactSymbol)
            } else if sym.starts_with(&q) {
                Some(MatchRank::SymbolPrefix)
            } else if sym.contains(&q) {
                Some(MatchRank::SymbolContains)
            } else if item
                .underlying
                .as_deref()
                .map(|u| u.to_ascii_uppercase().starts_with(&q))
                .unwrap_or(false)
            {
                Some(MatchRank::UnderlyingPrefix)
            } else if item
                .name
                .as_deref()
                .map(|n| n.to_ascii_uppercase().contains(&q))
                .unwrap_or(false)
            {
                Some(MatchRank::NameContains)
            } else {
                None
            };
            if let Some(rank) = rank {
                hits.push((item, rank));
            }
        }

        hits.sort_by(|(a, ra), (b, rb)| {
            ra.cmp(rb)
                .then_with(|| a.trading_symbol.cmp(&b.trading_symbol))
                .then_with(|| a.key.cmp(&b.key))
        });
        hits.truncate(limit);
        hits
    }

    /// All options for `underlying` and `expiry`, ordered by strike then CE/PE.
    pub fn option_chain(&self, underlying: &str, expiry: &str) -> Vec<&Instrument> {
        let key = underlying.trim().to_ascii_uppercase();
        let Some(indices) = self.by_underlying.get(&key) else {
            return Vec::new();
        };
        let mut out: Vec<&Instrument> = indices
            .iter()
            .map(|&i| &self.items[i])
            .filter(|i| i.is_option())
            .filter(|i| i.expiry.as_deref() == Some(expiry))
            .collect();
        out.sort_by(|a, b| {
            a.strike
                .cmp(&b.strike)
                .then_with(|| a.strike_type.cmp(&b.strike_type))
                .then_with(|| a.key.cmp(&b.key))
        });
        out
    }

    /// Distinct expiries for an underlying, ascending (`YYYY-MM-DD` sorts
    /// lexicographically, so this is chronological).
    pub fn expiries(&self, underlying: &str) -> Vec<String> {
        let key = underlying.trim().to_ascii_uppercase();
        let Some(indices) = self.by_underlying.get(&key) else {
            return Vec::new();
        };
        let mut set: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for &i in indices {
            if let Some(e) = self.items[i].expiry.as_deref() {
                if !e.is_empty() {
                    set.insert(e.to_string());
                }
            }
        }
        set.into_iter().collect()
    }

    /// Distinct underlyings (used to populate filter pickers).
    pub fn underlyings(&self) -> Vec<String> {
        let mut v: Vec<String> = self.by_underlying.keys().cloned().collect();
        v.sort();
        v
    }

    /// Instruments traded on `exchange`.
    pub fn on_exchange(&self, exchange: &str) -> Vec<&Instrument> {
        let target = exchange.trim().to_ascii_uppercase();
        self.items
            .iter()
            .filter(|i| i.key.exchange().eq_ignore_ascii_case(&target))
            .collect()
    }

    pub fn items(&self) -> &[Instrument] {
        &self.items
    }
}

/// Wire shape used by the loader/tests. Both source collections are normalised
/// into this before indexing, which keeps the mapping logic in one place.
#[derive(Debug, Clone, Deserialize)]
pub struct RawInstrument {
    pub exchange: String,
    pub token: String,
    #[serde(default)]
    pub trading_symbol: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub underlying: Option<String>,
    #[serde(default)]
    pub expiry: Option<String>,
    #[serde(default)]
    pub strike: Option<String>,
    #[serde(default)]
    pub strike_type: Option<String>,
    #[serde(default)]
    pub lot_size: Option<i64>,
    #[serde(default)]
    pub instrument_type: Option<String>,
    #[serde(default)]
    pub segment: Option<String>,
    #[serde(default)]
    pub tick_size: Option<String>,
}

impl RawInstrument {
    pub fn into_instrument(self) -> Result<Instrument, MasterError> {
        let key = SymbolKey::new(&self.exchange, &self.token)
            .map_err(|_| MasterError::InvalidSymbol(format!("{}|{}", self.exchange, self.token)))?;
        let strike = self
            .strike
            .as_deref()
            .and_then(|s| Price::parse_rounded(s).ok())
            .unwrap_or(Price::ZERO);
        let tick_size = self
            .tick_size
            .as_deref()
            .and_then(|s| Price::parse_rounded(s).ok());
        let strike_type = self
            .strike_type
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .map(StrikeType::parse)
            .or_else(|| {
                self.instrument_type
                    .as_deref()
                    .map(StrikeType::parse)
                    .filter(|t| *t != StrikeType::Other)
            })
            .unwrap_or(StrikeType::Other);

        Ok(Instrument {
            key,
            trading_symbol: if self.trading_symbol.trim().is_empty() {
                self.name.clone().unwrap_or_default()
            } else {
                self.trading_symbol
            },
            name: self.name,
            underlying: self.underlying,
            expiry: self.expiry.filter(|e| !e.trim().is_empty()),
            strike,
            strike_type,
            lot_size: self.lot_size.unwrap_or(0),
            instrument_type: self.instrument_type.unwrap_or_default(),
            segment: self.segment,
            tick_size,
        })
    }
}

/// Build a master from a JSON array of [`RawInstrument`] (fixtures, exports).
pub fn load_from_json(text: &str) -> Result<MemoryMaster, MasterError> {
    let raw: Vec<RawInstrument> = serde_json::from_str(text)?;
    let items = raw
        .into_iter()
        .map(|r| r.into_instrument())
        .collect::<Result<Vec<_>, _>>()?;
    MemoryMaster::build(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../tests/fixtures/instruments.json");

    fn master() -> MemoryMaster {
        load_from_json(FIXTURE).expect("fixture loads")
    }

    #[test]
    fn fixture_builds_and_indexes() {
        let m = master();
        assert!(m.len() >= 8);
        let k = SymbolKey::parse("NFO|158614").unwrap();
        let inst = m.get(&k).expect("instrument present");
        assert_eq!(inst.trading_symbol, "ZYDUSLIFE23FEB27P1680");
        assert_eq!(inst.lot_size, 450);
        assert_eq!(inst.strike.format(2), "1680.00");
        assert_eq!(inst.strike_type, StrikeType::Put);
    }

    #[test]
    fn search_ranks_exact_then_prefix() {
        let m = master();
        let hits = m.search("RELIANCE", 10);
        assert!(!hits.is_empty());
        // `BSE|500325` is literally "RELIANCE"; `NSE|2885` is "RELIANCE-EQ".
        assert_eq!(hits[0].1, MatchRank::ExactSymbol);
        assert_eq!(hits[0].0.trading_symbol, "RELIANCE");
        assert!(hits
            .iter()
            .any(|(i, r)| { i.trading_symbol == "RELIANCE-EQ" && *r == MatchRank::SymbolPrefix }));
    }

    #[test]
    fn search_is_case_insensitive_and_matches_underlying() {
        let m = master();
        let lower = m.search("nifty", 20);
        let upper = m.search("NIFTY", 20);
        assert_eq!(lower.len(), upper.len());
        assert!(lower
            .iter()
            .any(|(i, _)| i.trading_symbol.contains("NIFTY")));
    }

    #[test]
    fn search_respects_limit_and_returns_empty_for_blank() {
        let m = master();
        assert_eq!(m.search("N", 3).len(), 3);
        assert!(m.search("   ", 10).is_empty());
        assert!(m.search("ZZZZZNOTFOUND", 10).is_empty());
    }

    #[test]
    fn option_chain_is_ordered_by_strike_then_side() {
        let m = master();
        let chain = m.option_chain("NIFTY", "2026-11-26");
        assert!(chain.len() >= 4);
        let strikes: Vec<Price> = chain.iter().map(|i| i.strike).collect();
        let mut sorted = strikes.clone();
        sorted.sort();
        assert_eq!(strikes, sorted, "chain must be strike-ordered");
    }

    #[test]
    fn option_chain_filters_by_expiry() {
        let m = master();
        let chain = m.option_chain("NIFTY", "2026-12-31");
        assert!(chain
            .iter()
            .all(|i| i.expiry.as_deref() == Some("2026-12-31")));
    }

    #[test]
    fn expiries_are_sorted_and_unique() {
        let m = master();
        let e = m.expiries("NIFTY");
        assert_eq!(e, vec!["2026-11-26", "2026-12-31"]);
    }

    #[test]
    fn underlyings_are_sorted() {
        let m = master();
        let u = m.underlyings();
        let mut sorted = u.clone();
        sorted.sort();
        assert_eq!(u, sorted);
        assert!(u.contains(&"NIFTY".to_string()));
    }

    #[test]
    fn unknown_underlying_returns_empty() {
        let m = master();
        assert!(m.option_chain("DOESNOTEXIST", "2026-11-26").is_empty());
        assert!(m.expiries("DOESNOTEXIST").is_empty());
    }

    #[test]
    fn empty_master_is_rejected() {
        assert!(matches!(
            MemoryMaster::build(vec![]),
            Err(MasterError::Empty)
        ));
    }
}
