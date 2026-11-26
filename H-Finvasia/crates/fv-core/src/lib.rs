//! Core domain types shared by every other crate.
//!
//! Design rules enforced here (see `fv-features.md` §7.8):
//! * money is a fixed-point integer, never `f64`;
//! * symbol identity is `EXCHANGE|TOKEN` exactly as the feed spells it;
//! * no platform-specific code.

pub mod money;
pub mod quote;
pub mod symbol;

pub use money::{Price, SCALE, SCALE_DIGITS};
pub use quote::Quote;
pub use symbol::SymbolKey;

/// Errors produced by the domain layer.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CoreError {
    #[error("invalid decimal `{0}`")]
    InvalidDecimal(String),
    #[error("invalid symbol key `{0}` (expected EXCHANGE|TOKEN)")]
    InvalidSymbolKey(String),
    #[error("empty exchange or token in `{0}`")]
    EmptySymbolPart(String),
}
