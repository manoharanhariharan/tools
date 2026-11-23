//! Report shaping: one pass over the period's live entries produces stable
//! totals, per-employee rows, per-day rows and the detail lines used by every
//! export format. All arithmetic is integer minor units.

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::domain::money::format_hours;
use crate::repo::entries::{self, EntryRow};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportLayout {
    /// One row per employee (default printable summary).
    Employee,
    /// One row per calendar day.
    Daily,
    /// One row per entry (payroll detail).
    Entries,
}

impl ReportLayout {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "employee" => Some(ReportLayout::Employee),
            "daily" => Some(ReportLayout::Daily),
            "entries" => Some(ReportLayout::Entries),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            ReportLayout::Employee => "employee",
            ReportLayout::Daily => "daily",
            ReportLayout::Entries => "entries",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct MoneyTotals {
    pub entries: i64,
    pub regular_minutes: i64,
    pub overtime_minutes: i64,
    pub base_minor: i64,
    pub ot_minor: i64,
    pub bonus_minor: i64,
    pub gross_minor: i64,
    pub advance_minor: i64,
    pub deduction_minor: i64,
    pub net_minor: i64,
}

impl MoneyTotals {
    pub fn add_entry(&mut self, row: &EntryRow) {
        self.entries = self.entries.saturating_add(1);
        self.regular_minutes = self.regular_minutes.saturating_add(row.regular_minutes);
        self.overtime_minutes = self.overtime_minutes.saturating_add(row.overtime_minutes);
        self.base_minor = self.base_minor.saturating_add(row.base_minor);
        self.ot_minor = self.ot_minor.saturating_add(row.ot_minor);
        self.bonus_minor = self.bonus_minor.saturating_add(row.bonus_minor);
        self.gross_minor = self.gross_minor.saturating_add(row.gross_minor);
        self.advance_minor = self.advance_minor.saturating_add(row.advance_minor);
        self.deduction_minor = self.deduction_minor.saturating_add(row.deduction_minor);
        self.net_minor = self.net_minor.saturating_add(row.net_minor);
    }

    pub fn merge(&mut self, other: &MoneyTotals) {
        self.entries = self.entries.saturating_add(other.entries);
        self.regular_minutes = self.regular_minutes.saturating_add(other.regular_minutes);
        self.overtime_minutes = self.overtime_minutes.saturating_add(other.overtime_minutes);
        self.base_minor = self.base_minor.saturating_add(other.base_minor);
        self.ot_minor = self.ot_minor.saturating_add(other.ot_minor);
        self.bonus_minor = self.bonus_minor.saturating_add(other.bonus_minor);
        self.gross_minor = self.gross_minor.saturating_add(other.gross_minor);
        self.advance_minor = self.advance_minor.saturating_add(other.advance_minor);
        self.deduction_minor = self.deduction_minor.saturating_add(other.deduction_minor);
        self.net_minor = self.net_minor.saturating_add(other.net_minor);
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct EmployeeTotals {
    pub employee_id: String,
    pub name: String,
    pub code: Option<String>,
    pub pay_basis: String,
    pub totals: MoneyTotals,
}

#[derive(Debug, Clone, Serialize)]
pub struct DayTotals {
    pub work_date: String,
    pub totals: MoneyTotals,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReportData {
    pub from: String,
    pub to: String,
    pub generated_at: i64,
    pub totals: MoneyTotals,
    pub employees: Vec<EmployeeTotals>,
    pub days: Vec<DayTotals>,
    /// Detail lines, ordered by date then employee name.
    pub lines: Vec<EntryRow>,
}

impl ReportData {
    pub fn hours_label(&self) -> String {
        format!(
            "{} / {}",
            format_hours(self.totals.regular_minutes),
            format_hours(self.totals.overtime_minutes)
        )
    }
}

/// Group ordered entry lines into totals. Pure: no I/O, fully unit-tested.
pub fn build(from: &str, to: &str, generated_at: i64, lines: Vec<EntryRow>) -> ReportData {
    let mut totals = MoneyTotals::default();
    let mut employees: Vec<EmployeeTotals> = Vec::new();
    let mut employee_index: HashMap<String, usize> = HashMap::new();
    let mut days: BTreeMap<String, MoneyTotals> = BTreeMap::new();

    for line in &lines {
        totals.add_entry(line);

        let index = *employee_index.entry(line.employee_id.clone()).or_insert_with(|| {
            employees.push(EmployeeTotals {
                employee_id: line.employee_id.clone(),
                name: line.employee_name.clone(),
                code: line.employee_code.clone(),
                pay_basis: line.employee_pay_basis.clone(),
                totals: MoneyTotals::default(),
            });
            employees.len() - 1
        });
        employees[index].totals.add_entry(line);

        days.entry(line.work_date.clone())
            .or_default()
            .add_entry(line);
    }

    employees.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.employee_id.cmp(&b.employee_id))
    });

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
        lines,
    }
}

/// Load live entries for the range and shape them into a report.
pub async fn load(
    pool: &SqlitePool,
    from: &str,
    to: &str,
    employee_id: Option<&str>,
) -> Result<ReportData, sqlx::Error> {
    let lines = entries::list_live(pool, from, to, employee_id).await?;
    Ok(build(from, to, crate::domain::now_millis(), lines))
}

/// `0.50` style label for a day fraction given in basis points.
pub fn fraction_label(bp: i64) -> String {
    let hundredths = bp / 100;
    format!("{}.{:02}", hundredths / 100, hundredths % 100)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(
        id: &str,
        employee: &str,
        name: &str,
        date: &str,
        base: i64,
        ot: i64,
        bonus: i64,
        advance: i64,
        deduction: i64,
        regular_minutes: i64,
        overtime_minutes: i64,
        pay_basis: &str,
    ) -> EntryRow {
        let gross = base + ot + bonus;
        EntryRow {
            id: id.into(),
            employee_id: employee.into(),
            work_date: date.into(),
            day_fraction_bp: 10_000,
            regular_minutes,
            overtime_minutes,
            ot_multiplier_bp: None,
            override_amount_minor: None,
            bonus_minor: bonus,
            advance_minor: advance,
            deduction_minor: deduction,
            base_minor: base,
            ot_minor: ot,
            gross_minor: gross,
            net_minor: gross - advance - deduction,
            note: String::new(),
            created_at: 0,
            updated_at: 0,
            deleted_at: None,
            seq: 0,
            employee_name: name.into(),
            employee_code: Some(format!("CODE-{employee}")),
            employee_pay_basis: pay_basis.into(),
        }
    }

    #[test]
    fn groups_by_employee_and_day_with_accurate_totals() {
        let lines = vec![
            line("1", "e1", "Asha", "2026-11-02", 80_000, 0, 0, 0, 0, 0, 0, "day_rate"),
            line("2", "e1", "Asha", "2026-11-01", 80_000, 30_000, 10_000, 20_000, 5_000, 0, 120, "day_rate"),
            line("3", "e2", "Ravi", "2026-11-01", 112_500, 0, 0, 0, 0, 450, 0, "hourly"),
        ];
        let report = build("2026-11-01", "2026-11-30", 1_000, lines);

        assert_eq!(report.totals.entries, 3);
        assert_eq!(report.totals.base_minor, 272_500);
        assert_eq!(report.totals.ot_minor, 30_000);
        assert_eq!(report.totals.bonus_minor, 10_000);
        assert_eq!(report.totals.gross_minor, 312_500);
        assert_eq!(report.totals.advance_minor, 20_000);
        assert_eq!(report.totals.deduction_minor, 5_000);
        assert_eq!(report.totals.net_minor, 287_500);
        assert_eq!(report.totals.regular_minutes, 450);
        assert_eq!(report.totals.overtime_minutes, 120);

        assert_eq!(report.employees.len(), 2);
        assert_eq!(report.employees[0].name, "Asha");
        assert_eq!(report.employees[0].totals.entries, 2);
        assert_eq!(report.employees[0].totals.net_minor, 80_000 + 95_000);
        assert_eq!(report.employees[1].name, "Ravi");
        assert_eq!(report.employees[1].pay_basis, "hourly");

        assert_eq!(report.days.len(), 2);
        assert_eq!(report.days[0].work_date, "2026-11-01");
        assert_eq!(report.days[0].totals.entries, 2);
        assert_eq!(report.days[1].work_date, "2026-11-02");

        // Every grouping reconciles with the grand total.
        let employee_net: i64 = report
            .employees
            .iter()
            .map(|e| e.totals.net_minor)
            .sum();
        let day_net: i64 = report.days.iter().map(|d| d.totals.net_minor).sum();
        assert_eq!(employee_net, report.totals.net_minor);
        assert_eq!(day_net, report.totals.net_minor);
    }

    #[test]
    fn empty_report_is_all_zeroes() {
        let report = build("2026-11-01", "2026-11-30", 0, Vec::new());
        assert_eq!(report.totals, MoneyTotals::default());
        assert!(report.employees.is_empty());
        assert!(report.days.is_empty());
        assert_eq!(report.totals.net_minor, 0);
    }

    #[test]
    fn negative_net_is_preserved() {
        let lines = vec![line(
            "1", "e1", "Asha", "2026-11-01", 10_000, 0, 0, 50_000, 0, 0, 0, "fixed",
        )];
        let report = build("2026-11-01", "2026-11-01", 0, lines);
        assert_eq!(report.totals.net_minor, -40_000);
    }

    #[test]
    fn layouts_parse_strictly() {
        assert_eq!(ReportLayout::parse("entries"), Some(ReportLayout::Entries));
        assert_eq!(ReportLayout::parse("EMPLOYEE"), None);
        assert_eq!(ReportLayout::parse("x"), None);
        assert_eq!(fraction_label(5_000), "0.50");
        assert_eq!(fraction_label(10_000), "1.00");
        assert_eq!(fraction_label(20_000), "2.00");
    }
}
