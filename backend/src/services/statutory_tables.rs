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
    /// Read every rule table applicable on `effective_date` in one pass.
    pub async fn load(pool: &PgPool, effective_date: NaiveDate) -> AppResult<Self> {
        let tax_year = effective_date.year();

        let epf = epf_rates::list_bands(pool, effective_date).await?;
        let socso_all = socso_rates::list_bands(pool, effective_date).await?;
        let eis_all = eis_rates::list_bands(pool, effective_date).await?;
        let pcb_brackets = pcb_brackets::list_for_year(pool, tax_year, effective_date).await?;
        let pcb_reliefs = pcb_reliefs::list_for_year(pool, tax_year, effective_date)
            .await?
            .into_iter()
            .collect();
        let rule_sets = statutory_rule_sets::verified_for_date(pool, effective_date).await?;

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
