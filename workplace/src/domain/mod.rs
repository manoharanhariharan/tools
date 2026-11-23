//! Pure domain logic: money arithmetic, civil dates, and pay computation.
//!
//! Nothing in this module touches the database or HTTP so it can be exhaustively
//! unit-tested.

pub mod date;
pub mod money;
pub mod pay;

use time::OffsetDateTime;

/// Wall-clock timestamp in Unix milliseconds (UTC). All `created_at`,
/// `updated_at` and `deleted_at` values use this representation.
pub fn now_millis() -> i64 {
    let nanos = OffsetDateTime::now_utc().unix_timestamp_nanos();
    (nanos / 1_000_000) as i64
}
