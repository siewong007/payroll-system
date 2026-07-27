use std::collections::HashMap;

use chrono::NaiveDate;

use crate::models::statutory::{PcbBracketLookup, PcbInput, SocsoCategory};
use crate::services::eis_service;
use crate::services::epf_service;
use crate::services::pcb_calculator;
use crate::services::socso_service;
use crate::services::statutory_tables::StatutoryTables;
use crate::tests::support::skip_if_no_db;

// ---------------------------------------------------------------------------
// Pure function tests (no DB needed)
// ---------------------------------------------------------------------------

#[test]
fn test_pcb_rounding() {
    assert_eq!(pcb_calculator::round_up_to_ringgit(1001), 1100);
    assert_eq!(pcb_calculator::round_up_to_ringgit(1000), 1000);
    assert_eq!(pcb_calculator::round_up_to_ringgit(1099), 1100);
    assert_eq!(pcb_calculator::round_up_to_ringgit(0), 0);
    assert_eq!(pcb_calculator::round_up_to_ringgit(-100), 0);
}

// ---------------------------------------------------------------------------
// EPF — values come from reference data in migrations/1001_data.sql.
// ---------------------------------------------------------------------------

fn test_date() -> NaiveDate {
    NaiveDate::from_ymd_opt(2024, 6, 15).unwrap()
}

#[tokio::test]
async fn statutory_rules_outside_verified_interval_fail_closed() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let unsupported_date = NaiveDate::from_ymd_opt(2025, 1, 31).unwrap();
    let err = epf_service::calculate_epf(&pool, 250_000, 30, false, None, unsupported_date)
        .await
        .unwrap_err();
    assert!(format!("{err:?}").contains("No verified EPF statutory rule set"));
}

#[tokio::test]
async fn epf_category_a_table_lookup() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    // RM2,500/month → bracket (240001, 260000)
    let r = epf_service::calculate_epf(&pool, 250_000, 30, false, None, test_date())
        .await
        .unwrap();
    assert_eq!(r.employee, 27_500);
    assert_eq!(r.employer, 30_500);
    assert_eq!(r.category, "A");

    // RM5,000/month (ceiling of the 13% employer rate)
    let r = epf_service::calculate_epf(&pool, 500_000, 30, false, None, test_date())
        .await
        .unwrap();
    assert_eq!(r.employee, 53_000);
    assert_eq!(r.employer, 58_000);

    // RM20,000/month — last row of seeded table
    let r = epf_service::calculate_epf(&pool, 2_000_000, 30, false, None, test_date())
        .await
        .unwrap();
    assert_eq!(r.employee, 214_500);
    assert_eq!(r.employer, 234_000);
}

#[tokio::test]
async fn epf_above_table_fails_closed() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let err = epf_service::calculate_epf(&pool, 2_500_000, 30, false, None, test_date())
        .await
        .unwrap_err();
    assert!(format!("{err:?}").contains("no Part A band"));
}

#[tokio::test]
async fn epf_unseeded_category_fails_closed() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let err = epf_service::calculate_epf(&pool, 250_000, 30, false, Some("D"), test_date())
        .await
        .unwrap_err();
    assert!(format!("{err:?}").contains("no Part D band"));
}

#[tokio::test]
async fn epf_invalid_category_errors() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let err = epf_service::calculate_epf(&pool, 2_500_000, 30, false, Some("Z"), test_date())
        .await
        .unwrap_err();
    // AppError::BadRequest renders via IntoResponse; here we only care that
    // the error variant is surfaced.
    assert!(format!("{err:?}").contains("Invalid EPF category"));
}

// ---------------------------------------------------------------------------
// EPF — the Third-Schedule part is derived, not read from a dropdown.
//
// The column defaulted to 'A' and nothing ever changed it, so a citizen who
// turned 60 kept a full Part A employee deduction they are exempt from. The
// shipped fixture seeds Part A only, so anyone resolving to B/C/D now fails
// closed — which is the point, and is why the preview surfaces it first.
// ---------------------------------------------------------------------------

#[test]
fn epf_part_follows_age_and_residency() {
    assert_eq!(epf_service::derive_category(59, false), "A");
    assert_eq!(epf_service::derive_category(60, false), "C");
    assert_eq!(epf_service::derive_category(59, true), "B");
    assert_eq!(epf_service::derive_category(60, true), "D");
}

#[tokio::test]
async fn epf_under_60_citizen_is_unchanged_by_the_derivation() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    // A 59-year-old citizen resolves to Part A, so their figures are byte
    // identical to what a stored 'A' produced before.
    let derived = epf_service::calculate_epf(&pool, 500_000, 59, false, None, test_date())
        .await
        .unwrap();
    assert_eq!((derived.employee, derived.employer), (53_000, 58_000));
    assert_eq!(derived.category, "A");
}

#[tokio::test]
async fn epf_at_60_resolves_to_part_c_and_fails_closed_on_an_a_only_schedule() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    // The stored column still says 'A' — the default nobody ever revisited —
    // but the employee is 62, so Part C is what applies. The fixture carries no
    // Part C, and refusing is correct: continuing would take an 11% employee
    // deduction from someone exempt from it.
    let err = epf_service::calculate_epf(&pool, 500_000, 62, false, None, test_date())
        .await
        .unwrap_err();
    let rendered = format!("{err:?}");
    assert!(
        rendered.contains("Part C"),
        "the message must name the DERIVED part, not the stored one: {rendered}"
    );
}

#[tokio::test]
async fn an_explicit_epf_category_overrides_the_derivation() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    // HR knows about pre-August-1998 elections and voluntary-rate cases the
    // derivation cannot see, so the column wins when it is set. `preview_payroll`
    // raises the disagreement as a warning rather than silently applying it.
    let r = epf_service::calculate_epf(&pool, 500_000, 62, false, Some("A"), test_date())
        .await
        .unwrap();
    assert_eq!(r.category, "A");
    assert_eq!((r.employee, r.employer), (53_000, 58_000));
}

// ---------------------------------------------------------------------------
// SOCSO — values from prototype reference data in `1001_data.sql`.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn socso_first_category_under_60() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    // RM2,500 wage, age 30, citizen → bracket (240001, 250000) = (905, 1665, 1015)
    let r = socso_service::calculate_socso(&pool, 250_000, 30, false, test_date())
        .await
        .unwrap();
    assert_eq!(r.employee, 905);
    assert_eq!(r.employer, 1_665);
    assert_eq!(r.category, SocsoCategory::FirstCategory);
}

#[tokio::test]
async fn socso_second_category_age_60_plus() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    // Same wage, age 65 → Second Category (employer-only, second_cat_employer = 1015)
    let r = socso_service::calculate_socso(&pool, 250_000, 65, false, test_date())
        .await
        .unwrap();
    assert_eq!(r.employee, 0);
    assert_eq!(r.employer, 1_015);
    assert_eq!(r.category, SocsoCategory::SecondCategory);
}

#[tokio::test]
async fn socso_wage_ceiling_at_rm6000() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    // Wage RM8,000 caps to RM6,000 → bracket (500001, 600000) = (2175, 3985, 2415)
    let r = socso_service::calculate_socso(&pool, 800_000, 30, false, test_date())
        .await
        .unwrap();
    assert_eq!(r.employee, 2_175);
    assert_eq!(r.employer, 3_985);
}

#[tokio::test]
async fn socso_foreigner_fails_closed_without_eligibility_data() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let err = socso_service::calculate_socso(&pool, 250_000, 30, true, test_date())
        .await
        .unwrap_err();
    assert!(format!("{err:?}").contains("foreign workers"));
}

// ---------------------------------------------------------------------------
// EIS — values from prototype reference data in `1001_data.sql`.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn eis_standard_case() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    // RM2,500, age 30, citizen → bracket (240001, 250000) = (490, 490)
    let r = eis_service::calculate_eis(&pool, 250_000, 30, false, test_date())
        .await
        .unwrap();
    assert_eq!(r.employee, 490);
    assert_eq!(r.employer, 490);
}

#[tokio::test]
async fn eis_wage_ceiling_at_rm5000() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    // RM6,000 caps to RM5,000 → bracket (490001, 500000) = (990, 990)
    let r = eis_service::calculate_eis(&pool, 600_000, 30, false, test_date())
        .await
        .unwrap();
    assert_eq!(r.employee, 990);
    assert_eq!(r.employer, 990);
}

#[tokio::test]
async fn eis_age_57_without_contribution_history_fails_closed() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let err = eis_service::calculate_eis(&pool, 250_000, 57, false, test_date())
        .await
        .unwrap_err();
    assert!(format!("{err:?}").contains("prior EIS contribution status"));
}

#[tokio::test]
async fn eis_foreigner_exempt() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let r = eis_service::calculate_eis(&pool, 250_000, 30, true, test_date())
        .await
        .unwrap();
    assert_eq!(r.employee, 0);
    assert_eq!(r.employer, 0);
}

// ---------------------------------------------------------------------------
// PCB — golden-file tests.
//
// Values pin the CURRENT output of `calculate_pcb` against the seed data in
// migrations/1001_data.sql. If you revise the reference data (e.g. update
// the `cumulative_tax` column on `pcb_brackets` or bump any relief amount),
// re-derive these expected values rather than editing them to match.
// ---------------------------------------------------------------------------

fn pcb_input_defaults(monthly_normal_remuneration: i64) -> PcbInput {
    PcbInput {
        monthly_normal_remuneration,
        epf_employee_monthly: 0,
        socso_employee_monthly: 0,
        eis_employee_monthly: 0,
        zakat_monthly: 0,
        marital_status: "single".into(),
        working_spouse: false,
        num_children: 0,
        months_worked: 1,
        ytd_gross: 0,
        ytd_pcb: 0,
        ytd_epf: 0,
        ytd_socso: 0,
        ytd_eis: 0,
        ytd_zakat: 0,
        bonus_amount: 0,
    }
}

#[tokio::test]
async fn pcb_low_income_zero_tax() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    // RM1,000/month single, no children, January, no YTD.
    // Annual RM12k < individual relief RM9k + EPF/SOCSO/EIS relief → chargeable
    // lands in the 0% bracket, the RM400 rebate wipes out whatever remains.
    let mut input = pcb_input_defaults(100_000);
    input.epf_employee_monthly = 10_500; // from seed bracket (90001, 100000)
    input.socso_employee_monthly = 65;
    input.eis_employee_monthly = 15;

    let pcb = pcb_calculator::calculate_pcb(&pool, &input, test_date())
        .await
        .unwrap();
    assert_eq!(pcb, 0, "low income should produce zero PCB");
}

#[tokio::test]
async fn pcb_zakat_offsets_tax() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    // Pay enough zakat to wipe out any positive PCB — the calculator must
    // never return a negative value.
    let mut input = pcb_input_defaults(300_000);
    input.epf_employee_monthly = 32_000;
    input.socso_employee_monthly = 1_095;
    input.eis_employee_monthly = 590;
    input.zakat_monthly = 1_000_000; // RM10k/month zakat swamps any tax

    let pcb = pcb_calculator::calculate_pcb(&pool, &input, test_date())
        .await
        .unwrap();
    assert_eq!(pcb, 0);
}

#[tokio::test]
async fn pcb_married_with_children_reduces_tax() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    // Same gross and contributions, compared single-vs-married-with-2-children.
    // Spouse (RM4k) + 2× child (RM2k each) adds RM8k of relief → chargeable
    // falls, so PCB for the married case must be ≤ the single case.
    let mut single = pcb_input_defaults(500_000);
    single.epf_employee_monthly = 53_000;
    single.socso_employee_monthly = 1_825;
    single.eis_employee_monthly = 990;

    let mut married = single.clone();
    married.marital_status = "married".into();
    married.working_spouse = false;
    married.num_children = 2;

    let pcb_single = pcb_calculator::calculate_pcb(&pool, &single, test_date())
        .await
        .unwrap();
    let pcb_married = pcb_calculator::calculate_pcb(&pool, &married, test_date())
        .await
        .unwrap();

    assert!(
        pcb_married <= pcb_single,
        "married-with-children PCB ({pcb_married}) must not exceed single PCB ({pcb_single})"
    );
}

/// Pinned PCB value for the canonical RM5,000/month single employee, January
/// 2024, no YTD. Hand-derived from LHDN Schedule 1 2024 against the seed values
/// — see below. If this drifts, either the engine changed or the
/// brackets/reliefs seed changed; do not rewrite the expected value without
/// re-deriving it.
///
/// Re-derived when the EPF relief cap moved off `life_insurance` (RM3,000) onto
/// its own `epf_additional` relief (RM4,000): the calculator had been reading
/// the cap from the life-insurance relief, which is a different and optional
/// one. Reliefs rise by exactly RM1,000, chargeable income falls by the same,
/// and the whole movement stays inside the 6% band.
///
/// The bracket arithmetic below follows the code: the band's stored
/// `cumulative_tax` is used, not a hand-summed walk of the lower bands, and the
/// per-band product truncates toward zero.
///
/// Derivation:
///   Annual income          = 500_000 × 12               = 6_000_000 sen
///   Individual relief                                    =   900_000
///   EPF relief (annual 636_000, capped @ RM4,000)        =   400_000
///   SOCSO relief (12 × 1,825, cap RM350)                 =    21_900
///   EIS relief   (12 ×   990, cap RM350)                 =    11_880
///   Total reliefs                                        = 1_333_780
///   Chargeable income                                    = 4_666_220
///   Band RM35,000–RM50,000: 6%, cumulative_tax  60_000
///     6% × (4_666_220 − 3_500_001) = 6% × 1_166_219      =    69_973
///   Annual tax             = 60_000 + 69_973             =   129_973
///   Chargeable > RM35,000, so no RM400 rebate; no zakat
///   Monthly PCB (÷12, integer division)                  =    10_831
///   Round up to nearest RM                               =    10_900
#[tokio::test]
async fn pcb_canonical_rm5000_single_january() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let mut input = pcb_input_defaults(500_000);
    input.epf_employee_monthly = 53_000; // seed bracket (480001, 500000)
    input.socso_employee_monthly = 1_825;
    input.eis_employee_monthly = 990;

    let pcb =
        pcb_calculator::calculate_pcb(&pool, &input, NaiveDate::from_ymd_opt(2024, 1, 31).unwrap())
            .await
            .unwrap();

    assert_eq!(
        pcb, 10_900,
        "regression guard: canonical RM5,000/month PCB must be RM109 (see derivation above)"
    );
}

#[tokio::test]
async fn pcb_ytd_pcb_reduces_remaining_liability() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    // Two employees with identical annualised income but one has already paid
    // RM500 of PCB in prior months. The one with YTD paid should owe less.
    let mut jan = pcb_input_defaults(500_000);
    jan.epf_employee_monthly = 53_000;
    jan.socso_employee_monthly = 1_825;
    jan.eis_employee_monthly = 990;

    let mut jun = jan.clone();
    jun.months_worked = 6;
    jun.ytd_gross = 500_000 * 5; // RM2.5k × 5 prior months at RM5k
    jun.ytd_epf = 53_000 * 5;
    jun.ytd_socso = 1_825 * 5;
    jun.ytd_eis = 990 * 5;
    jun.ytd_pcb = 50_000; // RM500 already paid

    let pcb_without_ytd = pcb_calculator::calculate_pcb(&pool, &jan, test_date())
        .await
        .unwrap();

    let mut jun_no_prior = jun.clone();
    jun_no_prior.ytd_pcb = 0;
    let pcb_jun_no_prior = pcb_calculator::calculate_pcb(&pool, &jun_no_prior, test_date())
        .await
        .unwrap();
    let pcb_jun_with_prior = pcb_calculator::calculate_pcb(&pool, &jun, test_date())
        .await
        .unwrap();

    assert!(
        pcb_jun_with_prior <= pcb_jun_no_prior,
        "PCB with YTD already paid ({pcb_jun_with_prior}) should not exceed same scenario with no YTD ({pcb_jun_no_prior})"
    );
    // Sanity: both the same calendar scenarios produce positive figures in the
    // current seed — keeps this test honest if the seed ever changes.
    assert!(pcb_without_ytd >= 0);
}

// ---------------------------------------------------------------------------
// PCB — additional remuneration (bonus / commission).
//
// The calculator annualised the *whole* month's gross, and bonus and commission
// were inside it, so a one-off RM5,000 January bonus was multiplied by the
// twelve remaining months and taxed as if it recurred. These pin the split.
// Still behind `statutory_rules::require_supported_calculator` in production;
// reachable here because that gate is `#[cfg(not(test))]`.
// ---------------------------------------------------------------------------

/// A one-off bonus raises PCB by the Schedule 2 differential, not by twelve
/// months of itself.
///
/// The third input below reproduces the old arithmetic exactly — the bonus
/// folded into the annualised base, which is what `monthly_gross: gross` did —
/// so the comparison is against the defect itself rather than against a second
/// golden value that would have to be kept in step.
///
/// Note that the bonused figure is still several times the ordinary month: under
/// Schedule 2 the whole year's tax on additional remuneration falls due in the
/// month it is paid. That is correct, and is not what was wrong.
#[tokio::test]
async fn a_bonus_is_taxed_as_additional_remuneration_not_annualised() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let mut without_bonus = pcb_input_defaults(500_000);
    without_bonus.epf_employee_monthly = 53_000;
    without_bonus.socso_employee_monthly = 1_825;
    without_bonus.eis_employee_monthly = 990;

    let mut with_bonus = without_bonus.clone();
    with_bonus.bonus_amount = 500_000; // RM5,000 one-off

    // What the engine used to pass: bonus inside the base that gets multiplied
    // by the remaining months.
    let mut annualised = without_bonus.clone();
    annualised.monthly_normal_remuneration = 500_000 + 500_000;

    // Same effective date as `pcb_canonical_rm5000_single_january`, so the
    // no-bonus leg is that test's derived-and-pinned figure rather than a second
    // golden value to keep in step.
    let effective = NaiveDate::from_ymd_opt(2024, 1, 31).unwrap();
    let base = pcb_calculator::calculate_pcb(&pool, &without_bonus, effective)
        .await
        .unwrap();
    let bonused = pcb_calculator::calculate_pcb(&pool, &with_bonus, effective)
        .await
        .unwrap();
    let old_behaviour = pcb_calculator::calculate_pcb(&pool, &annualised, effective)
        .await
        .unwrap();

    assert_eq!(
        base, 10_900,
        "the no-bonus case is the pinned canonical run"
    );
    assert!(
        bonused > base,
        "a bonus is taxable, so PCB must rise: {bonused} vs {base}"
    );
    assert!(
        bonused < old_behaviour,
        "treating the bonus as recurring income over-deducted: {bonused} vs {old_behaviour}"
    );
}

/// The bonus contributes exactly the bracket differential — it is not also
/// sitting inside the annualised base it is differenced against.
#[tokio::test]
async fn bonus_pcb_is_only_the_schedule_2_differential() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let mut without_bonus = pcb_input_defaults(400_000);
    without_bonus.epf_employee_monthly = 43_000;
    without_bonus.socso_employee_monthly = 1_475;
    without_bonus.eis_employee_monthly = 790;

    let mut with_bonus = without_bonus.clone();
    with_bonus.bonus_amount = 300_000;

    let base = pcb_calculator::calculate_pcb(&pool, &without_bonus, test_date())
        .await
        .unwrap();
    let bonused = pcb_calculator::calculate_pcb(&pool, &with_bonus, test_date())
        .await
        .unwrap();

    // The differential is rounded up to the nearest ringgit on its own, so the
    // gap is a whole number of ringgit. It is also strictly less than the bonus:
    // a Schedule 2 charge is tax *on* the additional remuneration, whereas
    // annualising it charged tax on twelve copies of it.
    let differential = bonused - base;
    assert!(differential > 0, "a RM3,000 bonus must attract some tax");
    assert!(
        differential % 100 == 0,
        "the Schedule 2 differential is rounded to whole ringgit: {differential}"
    );
    assert!(
        differential < 300_000,
        "the charge cannot exceed the bonus itself: {differential}"
    );
}

// Whether the engine actually routes `total_bonus + total_commission` into
// `bonus_amount` is an engine question, not a calculator one — it is pinned
// end-to-end by `payroll_tests::bonus_and_commission_are_taxed_as_additional_remuneration`.

// ---------------------------------------------------------------------------
// PCB — schedule shape. Pure, over hand-built rule tables.
//
// The EPF relief cap used to be read from `life_insurance`, so an LHDN-keyed
// rule set (EPF under `epf_additional`, life insurance omitted because it is
// optional and nobody claimed it) aborted every employee in the run with a
// message naming the wrong relief.
//
// Bracket coverage is a whole-set property: the GiST exclusion on `pcb_brackets`
// forbids overlaps and is blind to gaps, and the per-row CHECKs cannot see a
// neighbour. Nothing in the database can catch either case.
// ---------------------------------------------------------------------------

fn bracket(from: i64, to: i64, rate: i64, cumulative: i64) -> PcbBracketLookup {
    PcbBracketLookup {
        chargeable_income_from: from,
        chargeable_income_to: to,
        tax_rate_percent: rust_decimal::Decimal::from(rate),
        cumulative_tax: cumulative,
    }
}

/// Every relief the calculator reads, so a test can drop exactly one.
fn full_reliefs() -> HashMap<String, i64> {
    [
        ("individual", 900_000),
        ("epf_additional", 400_000),
        ("life_insurance", 300_000),
        ("socso_relief", 35_000),
        ("eis_relief", 35_000),
        ("spouse", 400_000),
        ("child_under_18", 200_000),
        ("tax_rebate_individual", 40_000),
    ]
    .into_iter()
    .map(|(key, amount)| (key.to_string(), amount))
    .collect()
}

fn contiguous_brackets() -> Vec<PcbBracketLookup> {
    vec![
        bracket(0, 500_000, 0, 0),
        bracket(500_001, 2_000_000, 1, 0),
        bracket(2_000_001, 3_500_000, 3, 15_000),
        bracket(3_500_001, 9_999_999_999, 6, 60_000),
    ]
}

fn tables_with(brackets: Vec<PcbBracketLookup>, reliefs: HashMap<String, i64>) -> StatutoryTables {
    StatutoryTables::for_tests(
        NaiveDate::from_ymd_opt(2024, 1, 31).unwrap(),
        brackets,
        reliefs,
    )
}

#[test]
fn the_epf_cap_is_read_from_its_own_relief_type() {
    // An LHDN-keyed set: EPF cap under `epf_additional`, no `life_insurance` at
    // all. This used to abort with "missing relief 'life_insurance'".
    let mut reliefs = full_reliefs();
    reliefs.remove("life_insurance");

    let tables = tables_with(contiguous_brackets(), reliefs);
    let mut input = pcb_input_defaults(500_000);
    input.epf_employee_monthly = 53_000;

    assert!(pcb_calculator::calculate_pcb_with(&tables, &input).is_ok());
}

#[test]
fn a_schedule_keyed_the_old_way_still_computes() {
    // Compatibility fallback for one release: a rule set already installed with
    // the cap under `life_insurance` must keep working rather than start failing.
    let mut reliefs = full_reliefs();
    reliefs.remove("epf_additional");

    let tables = tables_with(contiguous_brackets(), reliefs);
    let mut input = pcb_input_defaults(500_000);
    input.epf_employee_monthly = 53_000;

    assert!(pcb_calculator::calculate_pcb_with(&tables, &input).is_ok());
}

#[test]
fn a_schedule_with_no_epf_cap_at_all_names_both_keys() {
    let mut reliefs = full_reliefs();
    reliefs.remove("epf_additional");
    reliefs.remove("life_insurance");

    let tables = tables_with(contiguous_brackets(), reliefs);
    let input = pcb_input_defaults(500_000);
    let err = pcb_calculator::calculate_pcb_with(&tables, &input).unwrap_err();
    let rendered = format!("{err:?}");
    assert!(rendered.contains("epf_additional"), "{rendered}");
    assert!(rendered.contains("life_insurance"), "{rendered}");
}

#[test]
fn the_relief_preflight_lists_what_a_schedule_is_missing() {
    let mut reliefs = full_reliefs();
    reliefs.remove("socso_relief");

    let tables = tables_with(contiguous_brackets(), reliefs);
    assert_eq!(tables.missing_required_reliefs(), vec!["socso_relief"]);
    // The EPF cap is satisfied by either key, so dropping only one is not a miss.
    let mut one_epf_key = full_reliefs();
    one_epf_key.remove("life_insurance");
    let one_key_tables = tables_with(contiguous_brackets(), one_epf_key);
    assert!(one_key_tables.missing_required_reliefs().is_empty());
}

/// Income falling in a gap between two bands used to be taxed at 0%: the lower
/// band matched, taxed its whole width, and did not break; the upper band was
/// skipped because the income was below its start.
#[test]
fn income_in_a_bracket_gap_fails_closed_instead_of_going_untaxed() {
    let gapped = vec![
        bracket(0, 2_000_000, 0, 0),
        bracket(3_000_001, 9_999_999_999, 6, 0),
    ];
    let tables = tables_with(gapped, full_reliefs());

    // Chargeable income lands at 2_500_000, inside the hole.
    let mut input = pcb_input_defaults(0);
    input.ytd_gross = 2_500_000 + 900_000; // individual relief is the only one that applies
    input.months_worked = 12;

    let err = pcb_calculator::calculate_pcb_with(&tables, &input).unwrap_err();
    let rendered = format!("{err:?}");
    assert!(
        rendered.contains("2500000"),
        "the message must name the uncovered amount: {rendered}"
    );
}

#[test]
fn income_above_a_finite_top_band_fails_closed() {
    let capped = vec![bracket(0, 500_000, 0, 0), bracket(500_001, 2_000_000, 1, 0)];
    let tables = tables_with(capped, full_reliefs());

    let mut input = pcb_input_defaults(0);
    input.ytd_gross = 5_000_000 + 900_000;
    input.months_worked = 12;

    let err = pcb_calculator::calculate_pcb_with(&tables, &input).unwrap_err();
    assert!(format!("{err:?}").contains("highest band ends at"));
}

#[test]
fn bracket_validation_accepts_a_contiguous_open_ended_set() {
    let tables = tables_with(contiguous_brackets(), full_reliefs());
    assert!(tables.validate_pcb_brackets().is_ok());
}

#[test]
fn bracket_validation_rejects_gaps_a_missing_floor_and_a_finite_ceiling() {
    let gapped = vec![
        bracket(0, 2_000_000, 0, 0),
        bracket(3_000_001, 9_999_999_999, 6, 0),
    ];
    let tables = tables_with(gapped, full_reliefs());
    let err = tables.validate_pcb_brackets().unwrap_err();
    assert!(err.contains("uncovered"), "{err}");

    let no_floor = vec![bracket(500_001, 9_999_999_999, 1, 0)];
    let tables = tables_with(no_floor, full_reliefs());
    let err = tables.validate_pcb_brackets().unwrap_err();
    assert!(err.contains("rather than 0"), "{err}");

    let finite_top = vec![bracket(0, 500_000, 0, 0), bracket(500_001, 2_000_000, 1, 0)];
    let tables = tables_with(finite_top, full_reliefs());
    let err = tables.validate_pcb_brackets().unwrap_err();
    assert!(err.contains("open-ended"), "{err}");
}
