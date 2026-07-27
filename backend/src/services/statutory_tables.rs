//! A run-scoped snapshot of every statutory rule table.
//!
//! The per-employee calculators used to take `&PgPool` and resolve each lookup
//! with its own round trip: one EPF band, one SOCSO band, one EIS band, then six
//! reliefs, a bracket list and a rebate for PCB — roughly fifteen queries per
//! employee, all with identical arguments beyond the wage, and all issued while
//! the payroll run held a write transaction open. A 200-employee run therefore
//! spent thousands of serialized round trips inside that transaction.
//!
//! The rule tables are effective-dated and a run has exactly one effective date,
//! so the whole applicable schedule can be read once and every employee resolved
//! from memory. Lookups here reproduce the SQL predicates exactly — the same
//! rows are candidates, and selection follows the same newest-schedule-wins and
//! wage-ceiling rules — so figures are unchanged; only the query count is.
//!
//! Loading is *not* gated: callers keep their existing fail-closed guard
//! (`statutory_rules::require_all_verified` for a run, `require_verified` for a
//! single domain) so error messages and gate semantics stay as they were.

use std::collections::HashMap;

use chrono::{Datelike, NaiveDate};
use sqlx::PgPool;

use crate::core::error::AppResult;
use crate::models::statutory::{EisBand, EpfBand, PcbBracketLookup, SocsoBand, VerifiedRuleSet};
use crate::repositories::{
    eis_rates, epf_rates, pcb_brackets, pcb_reliefs, socso_rates, statutory_rule_sets,
};

#[derive(Debug, Clone)]
pub struct StatutoryTables {
    pub effective_date: NaiveDate,
    pub tax_year: i32,
    epf: Vec<EpfBand>,
    /// Already narrowed to the newest applicable schedule, as the SQL does.
    socso: Vec<SocsoBand>,
    socso_ceiling: Option<i64>,
    eis: Vec<EisBand>,
    eis_ceiling: Option<i64>,
    pcb_brackets: Vec<PcbBracketLookup>,
    pcb_reliefs: HashMap<String, i64>,
    rule_sets: Vec<VerifiedRuleSet>,
}

impl StatutoryTables {
    /// Read every rule table applicable on `effective_date`, on a connection of
    /// the caller's choosing.
    ///
    /// A payroll run calls this with the connection its write transaction is
    /// already using. The pool-taking `load` below acquires a *second* pooled
    /// connection, and doing that while holding a transaction is how ten
    /// overlapping runs deadlocked a ten-connection pool: each run pinned two,
    /// and every one of them then blocked for the acquire timeout.
    ///
    /// Takes a reborrowable `&mut PgConnection` rather than `impl Executor`
    /// because it issues six sequential queries and `Executor` is consumed by
    /// value.
    pub async fn load_on(
        conn: &mut sqlx::PgConnection,
        effective_date: NaiveDate,
    ) -> AppResult<Self> {
        let tax_year = effective_date.year();

        let epf = epf_rates::list_bands(&mut *conn, effective_date).await?;
        let socso_all = socso_rates::list_bands(&mut *conn, effective_date).await?;
        let eis_all = eis_rates::list_bands(&mut *conn, effective_date).await?;
        let pcb_brackets =
            pcb_brackets::list_for_year(&mut *conn, tax_year, effective_date).await?;
        let pcb_reliefs = pcb_reliefs::list_for_year(&mut *conn, tax_year, effective_date)
            .await?
            .into_iter()
            .collect();
        let rule_sets = statutory_rule_sets::verified_for_date(&mut *conn, effective_date).await?;

        let socso = newest_schedule(socso_all, |band| band.effective_from);
        let socso_ceiling = socso.iter().map(|band| band.wage_to).max();
        let eis = newest_schedule(eis_all, |band| band.effective_from);
        let eis_ceiling = eis.iter().map(|band| band.wage_to).max();

        Ok(Self {
            effective_date,
            tax_year,
            epf,
            socso,
            socso_ceiling,
            eis,
            eis_ceiling,
            pcb_brackets,
            pcb_reliefs,
            rule_sets,
        })
    }

    /// Read the schedule on a freshly acquired pooled connection.
    ///
    /// For the one-off calculator entry points, which hold no transaction. A
    /// caller that *is* inside one must use `load_on` with its own connection.
    pub async fn load(pool: &PgPool, effective_date: NaiveDate) -> AppResult<Self> {
        let mut conn = pool.acquire().await?;
        Self::load_on(&mut conn, effective_date).await
    }

    /// EPF Third-Schedule band for a category and wage.
    ///
    /// Newest applicable band wins, matching the lookup's `ORDER BY
    /// effective_from DESC LIMIT 1`.
    pub fn epf_band(&self, category: &str, wage: i64) -> Option<&EpfBand> {
        self.epf
            .iter()
            .filter(|band| {
                band.category == category && band.wage_from <= wage && band.wage_to >= wage
            })
            .max_by_key(|band| band.effective_from)
    }

    /// SOCSO band for a wage, clamped to the schedule's ceiling.
    ///
    /// Wages above the top band contribute at the ceiling rate rather than
    /// falling through to "no band", which is what the SQL's `LEAST(wage,
    /// ceiling)` expresses.
    pub fn socso_band(&self, wage: i64) -> Option<&SocsoBand> {
        let capped = wage.min(self.socso_ceiling?);
        self.socso
            .iter()
            .find(|band| band.wage_from <= capped && band.wage_to >= capped)
    }

    /// EIS band for a wage, clamped to the schedule's ceiling.
    pub fn eis_band(&self, wage: i64) -> Option<&EisBand> {
        let capped = wage.min(self.eis_ceiling?);
        self.eis
            .iter()
            .find(|band| band.wage_from <= capped && band.wage_to >= capped)
    }

    pub fn pcb_brackets(&self) -> &[PcbBracketLookup] {
        &self.pcb_brackets
    }

    pub fn pcb_relief(&self, relief_type: &str) -> Option<i64> {
        self.pcb_reliefs.get(relief_type).copied()
    }

    /// Provenance of the rule sets these figures came from.
    pub fn rule_sets(&self) -> &[VerifiedRuleSet] {
        &self.rule_sets
    }

    /// The reliefs `pcb_calculator` actually reads that this schedule does not
    /// carry.
    ///
    /// A miss turns into `AppError::Validation` inside the per-employee
    /// calculation, so a schedule missing one relief produced N identical
    /// failures with no run-level statement of what was wrong. Checked once at
    /// load instead, so the preview says it once.
    ///
    /// The EPF cap is satisfied by either key — see `pcb_calculator`, where
    /// `life_insurance` is a compatibility fallback for `epf_additional`.
    pub fn missing_required_reliefs(&self) -> Vec<&'static str> {
        use crate::services::pcb_calculator::{EPF_RELIEF_TYPE, EPF_RELIEF_TYPE_LEGACY};

        let mut missing = Vec::new();
        for relief in [
            "individual",
            "socso_relief",
            "eis_relief",
            "spouse",
            "child_under_18",
            "tax_rebate_individual",
        ] {
            if self.pcb_relief(relief).is_none() {
                missing.push(relief);
            }
        }

        let epf_cap = self
            .pcb_relief(EPF_RELIEF_TYPE)
            .or_else(|| self.pcb_relief(EPF_RELIEF_TYPE_LEGACY));
        if epf_cap.is_none() {
            missing.push(EPF_RELIEF_TYPE);
        }
        missing
    }

    /// Whether the PCB brackets cover every chargeable income without a gap.
    ///
    /// Contiguity is a whole-set property: neither a per-row CHECK nor the GiST
    /// exclusion that forbids overlapping bands can express it, so it is
    /// deliberately not a database constraint. Without this a gap between two
    /// bands is taxed at 0% and everything above a finite top band is untaxed —
    /// silently, and in the employee's favour until LHDN disagrees.
    ///
    /// The top band must be open-ended in practice. LHDN's is; the shipped
    /// fixture expresses that as `9_999_999_999` sen (RM99,999,999.99), which is
    /// the sentinel below.
    pub fn validate_pcb_brackets(&self) -> Result<(), String> {
        const OPEN_ENDED_CEILING_SEN: i64 = 9_999_999_999;

        let Some(first) = self.pcb_brackets.first() else {
            return Err(format!(
                "no PCB tax brackets are configured for year {}",
                self.tax_year
            ));
        };
        if first.chargeable_income_from != 0 {
            return Err(format!(
                "the lowest PCB bracket for year {} starts at {} sen rather than 0, so incomes below it are uncovered",
                self.tax_year, first.chargeable_income_from
            ));
        }

        for pair in self.pcb_brackets.windows(2) {
            let (previous, next) = (&pair[0], &pair[1]);
            if next.chargeable_income_from != previous.chargeable_income_to + 1 {
                return Err(format!(
                    "PCB brackets for year {} leave {} sen to {} sen uncovered, which would be taxed at 0%",
                    self.tax_year,
                    previous.chargeable_income_to + 1,
                    next.chargeable_income_from - 1
                ));
            }
        }

        let last = self.pcb_brackets.last().expect("non-empty checked above");
        if last.chargeable_income_to < OPEN_ENDED_CEILING_SEN {
            return Err(format!(
                "the highest PCB bracket for year {} ends at {} sen; the top band must be open-ended (at least {} sen) or every ringgit above it goes untaxed",
                self.tax_year, last.chargeable_income_to, OPEN_ENDED_CEILING_SEN
            ));
        }

        Ok(())
    }

    /// Build a schedule in memory, for tests that exercise the pure calculators
    /// without a database.
    #[cfg(test)]
    pub(crate) fn for_tests(
        effective_date: NaiveDate,
        pcb_brackets: Vec<PcbBracketLookup>,
        pcb_reliefs: HashMap<String, i64>,
    ) -> Self {
        Self {
            effective_date,
            tax_year: effective_date.year(),
            epf: Vec::new(),
            socso: Vec::new(),
            socso_ceiling: None,
            eis: Vec::new(),
            eis_ceiling: None,
            pcb_brackets,
            pcb_reliefs,
            rule_sets: Vec::new(),
        }
    }
}

/// Keep only the rows belonging to the newest effective schedule.
///
/// SOCSO and EIS bands are versioned as whole schedules: mixing bands from two
/// effective dates would let a wage fall into an old band that a newer schedule
/// re-cut, so the SQL picks one `effective_from` and reads only its rows.
fn newest_schedule<T>(rows: Vec<T>, effective_from: impl Fn(&T) -> NaiveDate) -> Vec<T> {
    let Some(newest) = rows.iter().map(&effective_from).max() else {
        return rows;
    };
    rows.into_iter()
        .filter(|row| effective_from(row) == newest)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn epf_band(category: &str, from: i64, to: i64, effective: (i32, u32, u32)) -> EpfBand {
        EpfBand {
            category: category.to_string(),
            wage_from: from,
            wage_to: to,
            employee_contribution: from / 10,
            employer_contribution: from / 8,
            effective_from: NaiveDate::from_ymd_opt(effective.0, effective.1, effective.2).unwrap(),
        }
    }

    fn socso_band(from: i64, to: i64, effective: (i32, u32, u32)) -> SocsoBand {
        SocsoBand {
            wage_from: from,
            wage_to: to,
            first_cat_employee: from / 100,
            first_cat_employer: from / 50,
            second_cat_employer: from / 80,
            effective_from: NaiveDate::from_ymd_opt(effective.0, effective.1, effective.2).unwrap(),
        }
    }

    fn tables(epf: Vec<EpfBand>, socso: Vec<SocsoBand>) -> StatutoryTables {
        let socso = newest_schedule(socso, |band| band.effective_from);
        let socso_ceiling = socso.iter().map(|band| band.wage_to).max();
        StatutoryTables {
            effective_date: NaiveDate::from_ymd_opt(2026, 6, 30).unwrap(),
            tax_year: 2026,
            epf,
            socso,
            socso_ceiling,
            eis: Vec::new(),
            eis_ceiling: None,
            pcb_brackets: Vec::new(),
            pcb_reliefs: HashMap::new(),
            rule_sets: Vec::new(),
        }
    }

    #[test]
    fn epf_lookup_matches_category_and_wage_band() {
        let t = tables(
            vec![
                epf_band("A", 0, 100_000, (2024, 1, 1)),
                epf_band("A", 100_001, 500_000, (2024, 1, 1)),
                epf_band("B", 0, 100_000, (2024, 1, 1)),
            ],
            vec![],
        );

        assert_eq!(t.epf_band("A", 250_000).unwrap().wage_from, 100_001);
        assert_eq!(t.epf_band("B", 50_000).unwrap().wage_from, 0);
        assert!(t.epf_band("A", 900_000).is_none());
        assert!(t.epf_band("C", 50_000).is_none());
    }

    #[test]
    fn epf_lookup_prefers_the_newest_overlapping_band() {
        let t = tables(
            vec![
                epf_band("A", 0, 100_000, (2020, 1, 1)),
                epf_band("A", 0, 100_000, (2025, 10, 1)),
            ],
            vec![],
        );

        assert_eq!(
            t.epf_band("A", 50_000).unwrap().effective_from,
            NaiveDate::from_ymd_opt(2025, 10, 1).unwrap()
        );
    }

    #[test]
    fn socso_clamps_wages_above_the_schedule_ceiling_to_the_top_band() {
        let t = tables(
            vec![],
            vec![
                socso_band(0, 300_000, (2022, 9, 1)),
                socso_band(300_001, 600_000, (2022, 9, 1)),
            ],
        );

        // A wage far above the ceiling still contributes at the top band rather
        // than resolving to "no band configured".
        assert_eq!(t.socso_band(5_000_000).unwrap().wage_from, 300_001);
        assert_eq!(t.socso_band(150_000).unwrap().wage_from, 0);
    }

    #[test]
    fn socso_ignores_bands_from_a_superseded_schedule() {
        let t = tables(
            vec![],
            vec![
                socso_band(0, 400_000, (2019, 1, 1)),
                socso_band(0, 300_000, (2022, 9, 1)),
                socso_band(300_001, 600_000, (2022, 9, 1)),
            ],
        );

        // 350_000 falls in the old single band and in the new second band; the
        // newer schedule must win, and the old ceiling must not be considered.
        assert_eq!(t.socso_band(350_000).unwrap().wage_from, 300_001);
        assert_eq!(t.socso_ceiling, Some(600_000));
    }

    #[test]
    fn empty_schedule_resolves_to_no_band_rather_than_panicking() {
        let t = tables(vec![], vec![]);
        assert!(t.socso_band(250_000).is_none());
        assert!(t.eis_band(250_000).is_none());
        assert!(t.epf_band("A", 250_000).is_none());
    }
}
