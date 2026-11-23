//! Pure sync-decision logic, kept free of I/O so every branch is unit-testable.
//!
//! Policy summary
//!  * `base_seq` is the server sequence the device last saw for that row.
//!  * Matching `base_seq` means the write is clean and is applied.
//!  * A stale write whose content already matches the server is a no-op
//!    (this makes retries after a dropped response safe).
//!  * Any other stale write becomes an explicit conflict that the device must
//!    show to the user; a per-change `force` flag overrides it.
//!  * Deleting an absent/already-deleted row is a no-op, so delete retries are
//!    idempotent.

use crate::repo::employees::{EmployeeRow, ValidatedEmployee};
use crate::repo::entries::{EntryLineRow, EntryRow, PreparedEntry, PreparedLine};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Apply,
    Noop,
    Conflict(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExistingState {
    pub seq: i64,
    pub deleted: bool,
}

impl From<&EmployeeRow> for ExistingState {
    fn from(row: &EmployeeRow) -> Self {
        Self {
            seq: row.seq,
            deleted: row.deleted_at.is_some(),
        }
    }
}

impl From<&EntryRow> for ExistingState {
    fn from(row: &EntryRow) -> Self {
        Self {
            seq: row.seq,
            deleted: row.deleted_at.is_some(),
        }
    }
}

pub fn decide(
    existing: Option<ExistingState>,
    base_seq: i64,
    force: bool,
    incoming_deleted: bool,
    content_equal: bool,
) -> Decision {
    match existing {
        None => {
            if incoming_deleted {
                Decision::Noop
            } else if base_seq > 0 && !force {
                Decision::Conflict("missing_on_server")
            } else {
                Decision::Apply
            }
        }
        Some(row) => {
            if incoming_deleted && row.deleted {
                return Decision::Noop;
            }
            if row.seq == base_seq {
                return Decision::Apply;
            }
            if content_equal && !row.deleted && !incoming_deleted {
                return Decision::Noop;
            }
            if force {
                return Decision::Apply;
            }
            Decision::Conflict(if base_seq == 0 {
                "already_exists"
            } else {
                "stale"
            })
        }
    }
}

/// Compare the semantic (client-editable) employee fields, ignoring timestamps,
/// sequence numbers and the tombstone flag.
pub fn employee_content_equal(row: &EmployeeRow, v: &ValidatedEmployee) -> bool {
    row.name == v.name && row.code == v.code && row.note == v.note && row.active == v.active
}

/// Compare day-sheet inputs. Server-computed totals are derived, so only the
/// lines and adjustments participate.
pub fn entry_content_equal(row: &EntryRow, p: &PreparedEntry) -> bool {
    row.employee_id == p.employee_id
        && row.work_date == p.work_date
        && row.bonus_minor == p.bonus_minor
        && row.advance_minor == p.advance_minor
        && row.deduction_minor == p.deduction_minor
        && row.note == p.note
        && lines_equal(&row.lines, &p.lines)
}

fn lines_equal(stored: &[EntryLineRow], prepared: &[PreparedLine]) -> bool {
    if stored.len() != prepared.len() {
        return false;
    }
    let mut stored: Vec<(&str, &str, i64)> = stored
        .iter()
        .map(|line| {
            (
                line.type_id.as_str(),
                line.description.as_str(),
                line.amount_minor,
            )
        })
        .collect();
    let mut prepared: Vec<(&str, &str, i64)> = prepared
        .iter()
        .map(|line| {
            (
                line.type_id.as_str(),
                line.description.as_str(),
                line.amount_minor,
            )
        })
        .collect();
    stored.sort_unstable();
    prepared.sort_unstable();
    stored == prepared
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::totals::DayTotals;

    fn existing(seq: i64, deleted: bool) -> Option<ExistingState> {
        Some(ExistingState { seq, deleted })
    }

    fn row(seq: i64) -> EmployeeRow {
        EmployeeRow {
            id: "e1".into(),
            name: "Asha".into(),
            code: Some("E1".into()),
            pay_basis: "fixed".into(),
            rate_minor: 0,
            ot_multiplier_bp: 15000,
            standard_minutes_per_day: 480,
            note: String::new(),
            active: true,
            created_at: 1,
            updated_at: 1,
            deleted_at: None,
            seq,
        }
    }

    fn validated() -> ValidatedEmployee {
        ValidatedEmployee {
            name: "Asha".into(),
            code: Some("E1".into()),
            note: String::new(),
            active: true,
        }
    }

    fn entry_row() -> EntryRow {
        EntryRow {
            id: "x1".into(),
            employee_id: "e1".into(),
            work_date: "2026-11-23".into(),
            business_minor: 80_000,
            bonus_minor: 0,
            advance_minor: 0,
            deduction_minor: 0,
            gross_minor: 80_000,
            net_minor: 80_000,
            note: String::new(),
            created_at: 1,
            updated_at: 1,
            deleted_at: None,
            seq: 3,
            employee_name: String::new(),
            employee_code: None,
            lines: vec![EntryLineRow {
                id: "line1".into(),
                entry_id: "x1".into(),
                type_id: "sales".into(),
                type_name: "Sales".into(),
                description: "3 jobs".into(),
                amount_minor: 80_000,
                position: 0,
                created_at: 1,
            }],
        }
    }

    fn prepared() -> PreparedEntry {
        PreparedEntry {
            employee_id: "e1".into(),
            work_date: "2026-11-23".into(),
            lines: vec![PreparedLine {
                type_id: "sales".into(),
                type_name: "Sales".into(),
                description: "3 jobs".into(),
                amount_minor: 80_000,
            }],
            bonus_minor: 0,
            advance_minor: 0,
            deduction_minor: 0,
            note: String::new(),
            totals: DayTotals {
                business_minor: 80_000,
                bonus_minor: 0,
                gross_minor: 80_000,
                advance_minor: 0,
                deduction_minor: 0,
                net_minor: 80_000,
            },
        }
    }

    #[test]
    fn new_row_applies() {
        assert_eq!(decide(None, 0, false, false, false), Decision::Apply);
        assert_eq!(decide(None, 0, false, true, false), Decision::Noop);
    }

    #[test]
    fn creating_over_existing_id_conflicts_without_force() {
        assert_eq!(
            decide(existing(4, false), 0, false, false, false),
            Decision::Conflict("already_exists")
        );
        assert_eq!(
            decide(existing(4, false), 0, true, false, false),
            Decision::Apply
        );
    }

    #[test]
    fn matching_base_seq_applies_even_if_content_differs() {
        assert_eq!(
            decide(existing(7, false), 7, false, false, false),
            Decision::Apply
        );
    }

    #[test]
    fn stale_write_with_identical_content_is_noop() {
        assert_eq!(
            decide(existing(9, false), 7, false, false, true),
            Decision::Noop
        );
        assert_eq!(
            decide(existing(9, false), 7, false, true, true),
            Decision::Conflict("stale")
        );
    }

    #[test]
    fn stale_write_conflicts_unless_forced() {
        assert_eq!(
            decide(existing(9, false), 7, false, false, false),
            Decision::Conflict("stale")
        );
        assert_eq!(
            decide(existing(9, false), 7, true, false, false),
            Decision::Apply
        );
    }

    #[test]
    fn update_of_unknown_row_conflicts_unless_forced() {
        assert_eq!(
            decide(None, 7, false, false, false),
            Decision::Conflict("missing_on_server")
        );
        assert_eq!(decide(None, 7, true, false, false), Decision::Apply);
    }

    #[test]
    fn deleting_is_idempotent() {
        assert_eq!(
            decide(existing(9, true), 7, false, true, false),
            Decision::Noop
        );
        assert_eq!(
            decide(existing(9, true), 9, false, true, false),
            Decision::Noop
        );
        assert_eq!(
            decide(existing(4, false), 4, false, true, false),
            Decision::Apply
        );
    }

    #[test]
    fn resurrect_requires_force() {
        assert_eq!(
            decide(existing(9, true), 9, false, false, false),
            Decision::Apply
        );
        assert_eq!(
            decide(existing(9, true), 4, false, false, false),
            Decision::Conflict("stale")
        );
    }

    #[test]
    fn employee_content_comparison_ignores_volatile_fields() {
        let mut employee = row(3);
        assert!(employee_content_equal(&employee, &validated()));
        employee.updated_at = 999;
        employee.seq = 42;
        assert!(employee_content_equal(&employee, &validated()));
        employee.name = "Other".into();
        assert!(!employee_content_equal(&employee, &validated()));
    }

    #[test]
    fn entry_content_comparison_covers_lines_and_ignores_totals() {
        let mut entry = entry_row();
        assert!(entry_content_equal(&entry, &prepared()));
        entry.business_minor = 999_999;
        assert!(entry_content_equal(&entry, &prepared()));
        entry.lines[0].description = "different".into();
        assert!(!entry_content_equal(&entry, &prepared()));
    }

    #[test]
    fn line_order_does_not_matter() {
        let mut entry = entry_row();
        let mut prepared = prepared();
        prepared.lines.push(PreparedLine {
            type_id: "service".into(),
            type_name: "Service".into(),
            description: String::new(),
            amount_minor: 1_000,
        });
        entry.lines.push(EntryLineRow {
            id: "line2".into(),
            entry_id: "x1".into(),
            type_id: "service".into(),
            type_name: "Service".into(),
            description: String::new(),
            amount_minor: 1_000,
            position: 1,
            created_at: 1,
        });
        assert!(entry_content_equal(&entry, &prepared));
        entry.lines.swap(0, 1);
        assert!(entry_content_equal(&entry, &prepared));
    }
}
