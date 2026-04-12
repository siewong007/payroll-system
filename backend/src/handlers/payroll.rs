use axum::{
    extract::{Path, State},
    Json,
};
use uuid::Uuid;

use crate::core::app_state::AppState;
use crate::core::auth::AuthUser;
use crate::core::error::{AppError, AppResult};
use crate::models::payroll::{
    PayrollGroup, PayrollItem, PayrollRun, PayrollSummary, PayrollItemSummary,
    ProcessPayrollRequest,
};
use crate::services::payroll_engine;

pub async fn process(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<ProcessPayrollRequest>,
) -> AppResult<Json<PayrollRun>> {
    auth.deny_exec()?;
    let company_id = auth.0.company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let pay_date = req.pay_date.unwrap_or_else(|| {
        chrono::NaiveDate::from_ymd_opt(req.period_year, req.period_month as u32, 28)
            .unwrap_or_else(|| {
                chrono::NaiveDate::from_ymd_opt(req.period_year, req.period_month as u32, 1).unwrap()
            })
    });

    let run = payroll_engine::process_payroll(
        &state.pool,
        company_id,
        req.payroll_group_id,
        req.period_year,
        req.period_month,
        pay_date,
        auth.0.sub,
        req.notes,
    )
    .await?;

    Ok(Json(run))
}

pub async fn list_runs(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<PayrollRun>>> {
    auth.deny_exec()?;
    let company_id = auth.0.company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let runs = sqlx::query_as::<_, PayrollRun>(
        r#"SELECT id, company_id, payroll_group_id, period_year, period_month,
            period_start, period_end, pay_date, status::text as status,
            total_gross, total_net, total_employer_cost,
            total_epf_employee, total_epf_employer, total_socso_employee, total_socso_employer,
            total_eis_employee, total_eis_employer, total_pcb, total_zakat,
            employee_count, version, processed_by, processed_at, approved_by, approved_at,
            locked_at, locked_by, notes, created_at, updated_at, created_by, updated_by
        FROM payroll_runs
        WHERE company_id = $1
        ORDER BY period_year DESC, period_month DESC, created_at DESC
        LIMIT 50"#,
    )
    .bind(company_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(runs))
}

pub async fn get_run(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<PayrollSummary>> {
    auth.deny_exec()?;
    let company_id = auth.0.company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let run = sqlx::query_as::<_, PayrollRun>(
        r#"SELECT id, company_id, payroll_group_id, period_year, period_month,
            period_start, period_end, pay_date, status::text as status,
            total_gross, total_net, total_employer_cost,
            total_epf_employee, total_epf_employer, total_socso_employee, total_socso_employer,
            total_eis_employee, total_eis_employer, total_pcb, total_zakat,
            employee_count, version, processed_by, processed_at, approved_by, approved_at,
            locked_at, locked_by, notes, created_at, updated_at, created_by, updated_by
        FROM payroll_runs WHERE id = $1 AND company_id = $2"#,
    )
    .bind(id)
    .bind(company_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Payroll run not found".into()))?;

    let items = sqlx::query_as::<_, (Uuid, String, String, i64, i64, i64, i64, i64, i64, i64, i64)>(
        r#"SELECT pi.employee_id, e.full_name, e.employee_number,
           pi.basic_salary, pi.gross_salary, pi.total_deductions, pi.net_salary,
           pi.epf_employee, pi.socso_employee, pi.eis_employee, pi.pcb_amount
        FROM payroll_items pi
        JOIN employees e ON pi.employee_id = e.id
        WHERE pi.payroll_run_id = $1
        ORDER BY e.employee_number"#,
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;

    let item_summaries: Vec<PayrollItemSummary> = items
        .into_iter()
        .map(|(eid, name, num, basic, gross, ded, net, epf, socso, eis, pcb)| {
            PayrollItemSummary {
                employee_id: eid,
                employee_name: name,
                employee_number: num,
                basic_salary: basic,
                gross_salary: gross,
                total_deductions: ded,
                net_salary: net,
                epf_employee: epf,
                socso_employee: socso,
                eis_employee: eis,
                pcb_amount: pcb,
            }
        })
        .collect();

    Ok(Json(PayrollSummary {
        payroll_run: run,
        items: item_summaries,
    }))
}

pub async fn approve_run(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<PayrollRun>> {
    auth.deny_exec()?;
    let company_id = auth.0.company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let run = sqlx::query_as::<_, PayrollRun>(
        r#"UPDATE payroll_runs SET
            status = 'approved', approved_by = $3, approved_at = NOW(), updated_at = NOW()
        WHERE id = $1 AND company_id = $2 AND status = 'processed'
        RETURNING id, company_id, payroll_group_id, period_year, period_month,
            period_start, period_end, pay_date, status::text as status,
            total_gross, total_net, total_employer_cost,
            total_epf_employee, total_epf_employer, total_socso_employee, total_socso_employer,
            total_eis_employee, total_eis_employer, total_pcb, total_zakat,
            employee_count, version, processed_by, processed_at, approved_by, approved_at,
            locked_at, locked_by, notes, created_at, updated_at, created_by, updated_by"#,
    )
    .bind(id)
    .bind(company_id)
    .bind(auth.0.sub)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("Payroll run not found or not in processed status".into()))?;

    Ok(Json(run))
}

pub async fn lock_run(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<PayrollRun>> {
    auth.deny_exec()?;
    let company_id = auth.0.company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let run = sqlx::query_as::<_, PayrollRun>(
        r#"UPDATE payroll_runs SET
            status = 'paid', locked_by = $3, locked_at = NOW(), updated_at = NOW()
        WHERE id = $1 AND company_id = $2 AND status = 'approved'
        RETURNING id, company_id, payroll_group_id, period_year, period_month,
            period_start, period_end, pay_date, status::text as status,
            total_gross, total_net, total_employer_cost,
            total_epf_employee, total_epf_employer, total_socso_employee, total_socso_employer,
            total_eis_employee, total_eis_employer, total_pcb, total_zakat,
            employee_count, version, processed_by, processed_at, approved_by, approved_at,
            locked_at, locked_by, notes, created_at, updated_at, created_by, updated_by"#,
    )
    .bind(id)
    .bind(company_id)
    .bind(auth.0.sub)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("Payroll run not found or not approved".into()))?;

    Ok(Json(run))
}

pub async fn list_groups(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<PayrollGroup>>> {
    auth.deny_exec()?;
    let company_id = auth.0.company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let groups = sqlx::query_as::<_, PayrollGroup>(
        "SELECT * FROM payroll_groups WHERE company_id = $1 AND is_active = TRUE ORDER BY name",
    )
    .bind(company_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(groups))
}

pub async fn download_run_payslips_pdf(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(run_id): Path<Uuid>,
) -> Result<axum::response::Response, crate::core::error::AppError> {
    use axum::body::Body;
    use axum::http::{header, Response, StatusCode};

    auth.deny_exec()?;
    let company_id = auth.0.company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let bytes = crate::services::payslip_pdf_service::generate_bulk_payslips(
        &state.pool, run_id, company_id,
    ).await?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/pdf")
        .header(header::CONTENT_DISPOSITION, "attachment; filename=\"payslips.pdf\"")
        .body(Body::from(bytes))
        .unwrap())
}

pub async fn get_items(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Vec<PayrollItem>>> {
    auth.deny_exec()?;
    let items = sqlx::query_as::<_, PayrollItem>(
        "SELECT * FROM payroll_items WHERE payroll_run_id = $1 ORDER BY created_at",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(items))
}
