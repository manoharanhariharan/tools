//! Report shaping: one pass over the period's live day sheets produces stable
//! totals, per-employee rows, per-day rows, a per-type business breakdown, and
//! the detail lines used by every export format.

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::repo::entries::{self, EntryRow};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportLayout {
    /// One row per employee with carried-forward and Sunday earnings (default).
    Sunday,
    /// One row per employee (detailed printable summary).
    Employee,
    /// One row per calendar day.
    Daily,
    /// One row per employee-day sheet.
    Entries,
}

impl ReportLayout {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "sunday" => Some(ReportLayout::Sunday),
            "employee" => Some(ReportLayout::Employee),
            "daily" => Some(ReportLayout::Daily),
            "entries" => Some(ReportLayout::Entries),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            ReportLayout::Sunday => "sunday",
            ReportLayout::Employee => "employee",
            ReportLayout::Daily => "daily",
            ReportLayout::Entries => "entries",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct MoneyTotals {
    pub entries: i64,
    pub business_minor: i64,
    pub bonus_minor: i64,
    pub gross_minor: i64,
    pub advance_minor: i64,
    pub deduction_minor: i64,
    pub net_minor: i64,
}

impl MoneyTotals {
    pub fn add_entry(&mut self, row: &EntryRow) {
        self.entries = self.entries.saturating_add(1);
        self.business_minor = self.business_minor.saturating_add(row.business_minor);
        self.bonus_minor = self.bonus_minor.saturating_add(row.bonus_minor);
        self.gross_minor = self.gross_minor.saturating_add(row.gross_minor);
        self.advance_minor = self.advance_minor.saturating_add(row.advance_minor);
        self.deduction_minor = self.deduction_minor.saturating_add(row.deduction_minor);
        self.net_minor = self.net_minor.saturating_add(row.net_minor);
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct EmployeeTotals {
    pub employee_id: String,
    pub name: String,
    pub code: Option<String>,
    pub totals: MoneyTotals,
}

#[derive(Debug, Clone, Serialize)]
pub struct DayTotals {
    pub work_date: String,
    pub totals: MoneyTotals,
}

#[derive(Debug, Clone, Serialize)]
pub struct TypeTotals {
    pub type_id: String,
    pub type_name: String,
    pub amount_minor: i64,
}

/// Carried-forward balance plus Sunday earnings for one employee.
#[derive(Debug, Clone, Serialize)]
pub struct LedgerRow {
    pub employee_id: String,
    pub name: String,
    pub previous_minor: i64,
    pub sunday_minor: i64,
    pub current_minor: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReportData {
    pub from: String,
    pub to: String,
    pub generated_at: i64,
    pub totals: MoneyTotals,
    pub employees: Vec<EmployeeTotals>,
    pub days: Vec<DayTotals>,
    /// Business generated per earning type across the whole range.
    pub types: Vec<TypeTotals>,
    /// Carried-forward totals and Sunday earnings per employee.
    pub ledger: Vec<LedgerRow>,
    pub previous_total_minor: i64,
    pub sunday_total_minor: i64,
    pub current_total_minor: i64,
    /// Detail rows, ordered by date then employee name.
    pub lines: Vec<EntryRow>,
}

/// Group ordered day sheets into totals. Pure: no I/O, fully unit-tested.
pub fn build(from: &str, to: &str, generated_at: i64, lines: Vec<EntryRow>) -> ReportData {
    let mut totals = MoneyTotals::default();
    let mut employees: Vec<EmployeeTotals> = Vec::new();
    let mut employee_index: HashMap<String, usize> = HashMap::new();
    let mut days: BTreeMap<String, MoneyTotals> = BTreeMap::new();
    let mut types: BTreeMap<String, TypeTotals> = BTreeMap::new();
    let mut sunday_by_employee: HashMap<String, i64> = HashMap::new();

    for line in &lines {
        totals.add_entry(line);

        let index = *employee_index
            .entry(line.employee_id.clone())
            .or_insert_with(|| {
                employees.push(EmployeeTotals {
                    employee_id: line.employee_id.clone(),
                    name: line.employee_name.clone(),
                    code: line.employee_code.clone(),
                    totals: MoneyTotals::default(),
                });
                employees.len() - 1
            });
        employees[index].totals.add_entry(line);

        if crate::domain::date::is_sunday(&line.work_date) {
            let sunday = sunday_by_employee
                .entry(line.employee_id.clone())
                .or_default();
            *sunday = sunday.saturating_add(line.net_minor);
        }

        days.entry(line.work_date.clone())
            .or_default()
            .add_entry(line);

        for entry_line in &line.lines {
            let aggregate = types
                .entry(entry_line.type_id.clone())
                .or_insert_with(|| TypeTotals {
                    type_id: entry_line.type_id.clone(),
                    type_name: entry_line.type_name.clone(),
                    amount_minor: 0,
                });
            aggregate.amount_minor = aggregate
                .amount_minor
                .saturating_add(entry_line.amount_minor);
        }
    }

    employees.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.employee_id.cmp(&b.employee_id))
    });

    let mut types: Vec<TypeTotals> = types.into_values().collect();
    types.sort_by(|a, b| {
        b.amount_minor
            .cmp(&a.amount_minor)
            .then_with(|| a.type_name.to_lowercase().cmp(&b.type_name.to_lowercase()))
    });

    let ledger: Vec<LedgerRow> = employees
        .iter()
        .map(|employee| {
            let sunday_minor = sunday_by_employee
                .get(&employee.employee_id)
                .copied()
                .unwrap_or(0);
            LedgerRow {
                employee_id: employee.employee_id.clone(),
                name: employee.name.clone(),
                previous_minor: 0,
                sunday_minor,
                current_minor: sunday_minor,
            }
        })
        .collect();
    let sunday_total_minor = ledger
        .iter()
        .map(|row| row.sunday_minor)
        .fold(0i64, i64::saturating_add);

    ReportData {
        from: from.to_string(),
        to: to.to_string(),
        generated_at,
        totals,
        employees,
        days: days
            .into_iter()
            .map(|(work_date, totals)| DayTotals { work_date, totals })
            .collect(),
        types,
        ledger,
        previous_total_minor: 0,
        sunday_total_minor,
        current_total_minor: sunday_total_minor,
        lines,
    }
}

impl ReportData {
    /// Merge balances carried in from before the report range. Pure apart from
    /// the supplied data, so the ledger rules are unit-testable without SQL.
    pub fn apply_previous(&mut self, prior: Vec<crate::repo::entries::PriorTotal>) {
        let mut remaining: HashMap<String, crate::repo::entries::PriorTotal> = prior
            .into_iter()
            .map(|row| (row.employee_id.clone(), row))
            .collect();

        for row in &mut self.ledger {
            if let Some(previous) = remaining.remove(&row.employee_id) {
                row.previous_minor = previous.net_minor;
            }
            row.current_minor = row.previous_minor.saturating_add(row.sunday_minor);
        }

        for previous in remaining.into_values() {
            self.ledger.push(LedgerRow {
                employee_id: previous.employee_id,
                name: previous.name,
                previous_minor: previous.net_minor,
                sunday_minor: 0,
                current_minor: previous.net_minor,
            });
        }

        self.ledger.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.employee_id.cmp(&b.employee_id))
        });
        self.previous_total_minor = self
            .ledger
            .iter()
            .map(|row| row.previous_minor)
            .fold(0i64, i64::saturating_add);
        self.sunday_total_minor = self
            .ledger
            .iter()
            .map(|row| row.sunday_minor)
            .fold(0i64, i64::saturating_add);
        self.current_total_minor = self
            .ledger
            .iter()
            .map(|row| row.current_minor)
            .fold(0i64, i64::saturating_add);
    }
}

/// Load live day sheets for the range and shape them into a report.
pub async fn load(
    pool: &SqlitePool,
    from: &str,
    to: &str,
    employee_id: Option<&str>,
) -> Result<ReportData, sqlx::Error> {
    let lines = entries::list_live(pool, from, to, employee_id).await?;
    let prior = entries::totals_before(pool, from, employee_id).await?;
    let mut data = build(from, to, crate::domain::now_millis(), lines);
    data.apply_previous(prior);
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::entries::EntryLineRow;

    #[allow(clippy::too_many_arguments)]
    fn day(
        id: &str,
        employee: &str,
        name: &str,
        date: &str,
        sales: i64,
        service: i64,
        bonus: i64,
        advance: i64,
        deduction: i64,
    ) -> EntryRow {
        let business = sales + service;
        let gross = business + bonus;
        let mut lines = Vec::new();
        if sales > 0 {
            lines.push(EntryLineRow {
                id: format!("{id}-s"),
                entry_id: id.into(),
                type_id: "sales".into(),
                type_name: "Sales".into(),
                description: String::new(),
                amount_minor: sales,
                position: 0,
                created_at: 0,
            });
        }
        if service > 0 {
            lines.push(EntryLineRow {
                id: format!("{id}-v"),
                entry_id: id.into(),
                type_id: "service".into(),
                type_name: "Service".into(),
                description: String::new(),
                amount_minor: service,
                position: 1,
                created_at: 0,
            });
        }
        EntryRow {
            id: id.into(),
            employee_id: employee.into(),
            work_date: date.into(),
            business_minor: business,
            bonus_minor: bonus,
            advance_minor: advance,
            deduction_minor: deduction,
            gross_minor: gross,
            net_minor: gross - advance - deduction,
            note: String::new(),
            created_at: 0,
            updated_at: 0,
            deleted_at: None,
            seq: 0,
            employee_name: name.into(),
            employee_code: Some(format!("CODE-{employee}")),
            lines,
        }
    }

    #[test]
    fn groups_by_employee_day_and_type_with_reconciled_totals() {
        let lines = vec![
            day("1", "e1", "Asha", "2026-11-02", 50_000, 0, 0, 0, 0),
            day(
                "2",
                "e1",
                "Asha",
                "2026-11-01",
                40_000,
                20_000,
                10_000,
                20_000,
                5_000,
            ),
            day("3", "e2", "Ravi", "2026-11-01", 0, 60_000, 0, 0, 0),
        ];
        let report = build("2026-11-01", "2026-11-30", 1_000, lines);

        assert_eq!(report.totals.entries, 3);
        assert_eq!(report.totals.business_minor, 170_000);
        assert_eq!(report.totals.bonus_minor, 10_000);
        assert_eq!(report.totals.gross_minor, 180_000);
        assert_eq!(report.totals.advance_minor, 20_000);
        assert_eq!(report.totals.deduction_minor, 5_000);
        assert_eq!(report.totals.net_minor, 155_000);

        assert_eq!(report.employees.len(), 2);
        assert_eq!(report.employees[0].name, "Asha");
        assert_eq!(report.employees[0].totals.entries, 2);
        assert_eq!(report.employees[0].totals.net_minor, 50_000 + 45_000);
        assert_eq!(report.employees[1].name, "Ravi");

        assert_eq!(report.days.len(), 2);
        assert_eq!(report.days[0].work_date, "2026-11-01");
        assert_eq!(report.days[0].totals.entries, 2);

        assert_eq!(report.types.len(), 2);
        let sales = report
            .types
            .iter()
            .find(|t| t.type_name == "Sales")
            .unwrap();
        assert_eq!(sales.amount_minor, 90_000);
        let service = report
            .types
            .iter()
            .find(|t| t.type_name == "Service")
            .unwrap();
        assert_eq!(service.amount_minor, 80_000);

        let employee_net: i64 = report.employees.iter().map(|e| e.totals.net_minor).sum();
        let day_net: i64 = report.days.iter().map(|d| d.totals.net_minor).sum();
        let type_total: i64 = report.types.iter().map(|t| t.amount_minor).sum();
        assert_eq!(employee_net, report.totals.net_minor);
        assert_eq!(day_net, report.totals.net_minor);
        assert_eq!(type_total, report.totals.business_minor);
    }

    #[test]
    fn empty_report_is_all_zeroes() {
        let report = build("2026-11-01", "2026-11-30", 0, Vec::new());
        assert_eq!(report.totals, MoneyTotals::default());
        assert!(report.employees.is_empty());
        assert!(report.days.is_empty());
        assert!(report.types.is_empty());
    }

    #[test]
    fn negative_net_is_preserved() {
        let lines = vec![day(
            "1",
            "e1",
            "Asha",
            "2026-11-01",
            10_000,
            0,
            0,
            50_000,
            0,
        )];
        let report = build("2026-11-01", "2026-11-01", 0, lines);
        assert_eq!(report.totals.net_minor, -40_000);
    }

    #[test]
    fn layouts_parse_strictly() {
        assert_eq!(ReportLayout::parse("sunday"), Some(ReportLayout::Sunday));
        assert_eq!(ReportLayout::parse("entries"), Some(ReportLayout::Entries));
        assert_eq!(ReportLayout::parse("EMPLOYEE"), None);
        assert_eq!(ReportLayout::parse("x"), None);
    }

    #[test]
    fn sunday_ledger_adds_prior_balances_to_sunday_earnings_only() {
        let lines = vec![
            day("1", "e1", "Asha", "2026-11-01", 40_000, 0, 0, 0, 0),
            day("2", "e1", "Asha", "2026-11-02", 10_000, 0, 0, 0, 0),
            day("3", "e2", "Ravi", "2026-11-01", 0, 60_000, 0, 0, 0),
        ];
        let mut report = build("2026-11-01", "2026-11-30", 0, lines);
        report.apply_previous(vec![
            crate::repo::entries::PriorTotal {
                employee_id: "e1".into(),
                name: "Asha".into(),
                net_minor: 100_000,
            },
            crate::repo::entries::PriorTotal {
                employee_id: "e3".into(),
                name: "Rekha".into(),
                net_minor: 110_000,
            },
        ]);

        assert_eq!(report.ledger.len(), 3);
        assert_eq!(report.ledger[0].name, "Asha");
        assert_eq!(report.ledger[0].previous_minor, 100_000);
        assert_eq!(report.ledger[0].sunday_minor, 40_000);
        assert_eq!(report.ledger[0].current_minor, 140_000);
        assert_eq!(report.ledger[1].name, "Ravi");
        assert_eq!(report.ledger[1].previous_minor, 0);
        assert_eq!(report.ledger[1].current_minor, 60_000);
        assert_eq!(report.ledger[2].name, "Rekha");
        assert_eq!(report.ledger[2].previous_minor, 110_000);
        assert_eq!(report.ledger[2].sunday_minor, 0);

        assert_eq!(report.previous_total_minor, 210_000);
        assert_eq!(report.sunday_total_minor, 100_000);
        assert_eq!(report.current_total_minor, 310_000);
    }

    #[test]
    fn ledger_skips_non_sunday_range_earnings() {
        let lines = vec![day("1", "e1", "Asha", "2026-11-02", 50_000, 0, 0, 0, 0)];
        let report = build("2026-11-01", "2026-11-30", 0, lines);
        assert_eq!(report.ledger.len(), 1);
        assert_eq!(report.ledger[0].sunday_minor, 0);
        assert_eq!(report.ledger[0].current_minor, 0);
        assert_eq!(report.sunday_total_minor, 0);
    }
}
