//! Instrument master.
//!
//! Replaces two legacy behaviours:
//! * parsing a 90.8 MB single-line JSON file on every cold start;
//! * issuing `Get Scrips Info` per strike while opening an option chain.
//!
//! The master is loaded once from the local MongoDB instance (`FINVASIA_SYMBOL`,
//! or the Kite-shaped `INSTRUMENT` as a cross-reference) into a compact in-memory
//! index, then every lookup/search/chain query is local.

pub mod index;

#[cfg(feature = "mongo")]
pub mod mongo;

pub use index::{is_index, Instrument};
pub use index::{MasterError, MemoryMaster, StrikeType};
