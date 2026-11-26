// Civil-date conversion used by the build stamp.
//
// This file is `include!`d by `build.rs` (which cannot depend on the crate) and
// also compiled under `#[cfg(test)]` here, so the algorithm that names release
// artifacts is actually covered by tests.
//
// Plain `//` comments are used deliberately: `//!` inner docs are illegal at the
// point `include!` pastes this file into `build.rs`.
//
// Algorithm: https://howardhinnant.github.io/date_algorithms.html#civil_from_days

/// Convert days since 1970-01-01 into a civil `(year, month, day)`.
#[allow(dead_code)]
pub(crate) fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::civil_from_days;

    #[test]
    fn epoch_is_day_zero() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(1), (1970, 1, 2));
    }

    #[test]
    fn known_dates_convert() {
        // 1970-01-31 + 1970-02-28 => 1970-03-01
        assert_eq!(civil_from_days(59), (1970, 3, 1));
        assert_eq!(civil_from_days(19_000), (2022, 1, 8));
        assert_eq!(civil_from_days(20_783), (2026, 11, 26));
    }

    #[test]
    fn leap_day_is_handled() {
        assert_eq!(civil_from_days(19_782), (2024, 2, 29));
        assert_eq!(civil_from_days(19_783), (2024, 3, 1));
    }

    #[test]
    fn every_day_of_2024_is_monotonic_and_valid() {
        let mut previous = (2024, 1, 1);
        assert_eq!(civil_from_days(19_723), previous);
        for day in 19_724..=20_088 {
            let (y, m, d) = civil_from_days(day);
            assert_eq!(y, 2024, "day {day} left 2024");
            assert!((1..=12).contains(&m), "bad month {m} on day {day}");
            assert!((1..=31).contains(&d), "bad day {d} on day {day}");
            assert!(
                (y, m, d) > previous,
                "dates must strictly increase at day {day}"
            );
            previous = (y, m, d);
        }
    }
}
