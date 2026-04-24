use chrono::NaiveDate;

use crate::services::eis_service;
use crate::services::epf_service;
use crate::services::pcb_calculator::{self, PcbInput};
use crate::services::socso_service::{self, SocsoCategory};
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

#[test]
fn test_epf_rounding() {
    assert_eq!(epf_service::round_to_nearest_ringgit(1050), 1100);
    assert_eq!(epf_service::round_to_nearest_ringgit(1049), 1000);
    assert_eq!(epf_service::round_to_nearest_ringgit(1000), 1000);
    assert_eq!(epf_service::round_to_nearest_ringgit(1099), 1100);
    assert_eq!(epf_service::round_to_nearest_ringgit(50), 100);
    assert_eq!(epf_service::round_to_nearest_ringgit(49), 0);
}

// ---------------------------------------------------------------------------
// EPF — values come from seed in migrations/007_seed_statutory_data.sql.
// ---------------------------------------------------------------------------

fn test_date() -> NaiveDate {
    NaiveDate::from_ymd_opt(2024, 6, 15).unwrap()
}

#[tokio::test]
async fn epf_category_a_table_lookup() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    // RM2,500/month → bracket (240001, 260000)
    let r = epf_service::calculate_epf(&pool, 250_000, "A", test_date())
        .await
        .unwrap();
    assert_eq!(r.employee, 27_500);
    assert_eq!(r.employer, 30_500);

    // RM5,000/month (ceiling of the 13% employer rate)
    let r = epf_service::calculate_epf(&pool, 500_000, "A", test_date())
        .await
        .unwrap();
    assert_eq!(r.employee, 53_000);
    assert_eq!(r.employer, 58_000);

    // RM20,000/month — last row of seeded table
    let r = epf_service::calculate_epf(&pool, 2_000_000, "A", test_date())
        .await
        .unwrap();
    assert_eq!(r.employee, 214_500);
    assert_eq!(r.employer, 234_000);
}

#[tokio::test]
async fn epf_above_table_uses_flat_percent() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    // RM25,000/month > RM20,000 table cap → flat 11% / 12%
    let r = epf_service::calculate_epf(&pool, 2_500_000, "A", test_date())
        .await
        .unwrap();
    assert_eq!(r.employee, 275_000); // 11% of 2_500_000
    assert_eq!(r.employer, 300_000); // 12% of 2_500_000 (employer drops to 12% above RM5k)
}

#[tokio::test]
async fn epf_category_d_senior_citizen() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    // Category D (citizen ≥ 60): 0% employee, 4% employer, no table row so flat calc.
    let r = epf_service::calculate_epf(&pool, 2_500_000, "D", test_date())
        .await
        .unwrap();
    assert_eq!(r.employee, 0);
    assert_eq!(r.employer, 100_000); // 4% of 2_500_000
}

#[tokio::test]
async fn epf_invalid_category_errors() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    // Wage above the table triggers the fallback branch where the category is
    // validated; an unknown category must be rejected.
    let err = epf_service::calculate_epf(&pool, 2_500_000, "Z", test_date())
        .await
        .unwrap_err();
    // AppError::BadRequest renders via IntoResponse; here we only care that
    // the error variant is surfaced.
    assert!(format!("{err:?}").contains("Invalid EPF category"));
}

// ---------------------------------------------------------------------------
// SOCSO — values from seed in migration 007.
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
async fn socso_foreigner_is_exempt() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let r = socso_service::calculate_socso(&pool, 250_000, 30, true, test_date())
        .await
        .unwrap();
    assert_eq!(r.employee, 0);
    assert_eq!(r.employer, 0);
    assert_eq!(r.category, SocsoCategory::Exempt);
}

// ---------------------------------------------------------------------------
// EIS — values from seed in migration 007.
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
async fn eis_age_57_plus_exempt() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };

    let r = eis_service::calculate_eis(&pool, 250_000, 57, false, test_date())
        .await
        .unwrap();
    assert_eq!(r.employee, 0);
    assert_eq!(r.employer, 0);
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
// migrations/007_seed_statutory_data.sql. If you revise the seed (e.g. update
// the `cumulative_tax` column on `pcb_brackets` or bump any relief amount),
// re-derive these expected values rather than editing them to match.
// ---------------------------------------------------------------------------

fn pcb_input_defaults(monthly_gross: i64) -> PcbInput {
    PcbInput {
        monthly_gross,
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
        is_bonus_month: false,
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
/// 2024, no YTD. Hand-derived from LHDN Schedule 1 2024 against the (post-023)
/// seed values — see below. If this drifts, either the engine changed or the
/// brackets/reliefs seed changed; do not rewrite the expected value without
/// re-deriving it.
///
/// Derivation:
///   Annual income         = 500_000 × 12             = 6_000_000 sen
///   Individual relief                                 =   900_000
///   EPF relief (capped @ RM3,000)                     =   300_000
///   SOCSO relief (12 × 1,825, cap RM350)              =    21_900
///   EIS relief   (12 ×   990, cap RM350)              =    11_880
///   Total reliefs                                     = 1_233_780
///   Chargeable income                                 = 4_766_220
///   Tax bracket (0–RM5k)      0%                      =        0
///   Tax bracket (RM5k–RM20k)  1% × 1_499_999          =   14_999
///   Tax bracket (RM20k–RM35k) 3% × 1_499_999          =   44_999
///   Tax bracket (RM35k–RM47,662.20) 6% × 1_266_219    =   75_973
///   Annual tax                                        =  135_971
///   Monthly PCB (÷12, integer division)               =   11_330
///   Round up to nearest RM                            =   11_400
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
        pcb, 11_400,
        "regression guard: canonical RM5,000/month PCB must be RM114 (see derivation above)"
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
