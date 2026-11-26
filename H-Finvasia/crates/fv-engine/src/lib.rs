//! Rendering and submission pacing.
//!
//! Two independent problems, both solved by *bounded* pacing rather than by
//! touching the network:
//!
//! * **Tick coalescing** — the legacy terminal dispatched every property change
//!   to a DevExpress grid. Here ticks are absorbed into a [`QuoteBook`] and the UI
//!   is fed a bounded batch at a fixed rate.
//! * **Order submission** — the venue allows roughly ten order requests per
//!   second. [`TokenBucket`] makes that limit explicit instead of discovering it
//!   through `Rate_Limited` errors.

pub mod budget;
pub mod coalesce;

pub use budget::TokenBucket;
pub use coalesce::{Coalescer, DeltaBatch};
