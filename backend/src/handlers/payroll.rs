use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use uuid::Uuid;

use crate::core::app_state::AppState;
use crate::core::auth::AuthUser;
use crate::core::error::{AppError, AppResult};
use crate::models::payroll::{
    CreatePayrollEntryRequest, PayrollEntry, PayrollEntryWithEmployee, PayrollGroup, PayrollItem,
    PayrollItemSummary, PayrollRun, PayrollSummary, ProcessPayrollRequest,
    UpdatePayrollEntryRequest, UpdatePayrollPcbRequest,
};
use crate::services::payroll_engine;

pub async fn process(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<ProcessPayrollRequest>,
) -> AppResult<Json<PayrollRun>> {
    auth.require_payroll_privileged()?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let pay_date = req.pay_date.unwrap_or_else(|| {
        chrono::NaiveDate::from_ymd_opt(req.period_year, req.period_month as u32, 28)
            .unwrap_or_else(|| {
                chrono::NaiveDate::from_ymd_opt(req.period_year, req.period_month as u32, 1)
                    .unwrap()
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

async fn load_payroll_summary(
    pool: &sqlx::PgPool,
    company_id: Uuid,
    id: Uuid,
) -> AppResult<PayrollSummary> {
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
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Payroll run not found".into()))?;

    let items =
        sqlx::query_as::<_, (Uuid, String, String, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64)>(
            r#"SELECT pi.employee_id, e.full_name, e.employee_number,
           pi.basic_salary, pi.total_allowances, pi.total_overtime, pi.total_claims,
           pi.gross_salary, pi.total_deductions, pi.net_salary,
           pi.epf_employee, pi.socso_employee, pi.eis_employee, pi.pcb_amount
        FROM payroll_items pi
        JOIN employees e ON pi.employee_id = e.id
        WHERE pi.payroll_run_id = $1
        ORDER BY e.employee_number"#,
        )
        .bind(id)
        .fetch_all(pool)
        .await?;

    let item_summaries: Vec<PayrollItemSummary> = items
        .into_iter()
        .map(
            |(eid, name, num, basic, allowances, overtime, claims, gross, ded, net, epf, socso, eis, pcb)| PayrollItemSummary {
                employee_id: eid,
                employee_name: name,
                employee_number: num,
                basic_salary: basic,
                total_allowances: allowances,
                total_overtime: overtime,
                total_claims: claims,
                gross_salary: gross,
                total_deductions: ded,
                net_salary: net,
                epf_employee: epf,
                socso_employee: socso,
                eis_employee: eis,
                pcb_amount: pcb,
            },
        )
        .collect();

    Ok(PayrollSummary {
        payroll_run: run,
        items: item_summaries,
    })
}

pub async fn list_runs(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<PayrollRun>>> {
    auth.require_payroll_privileged()?;
    let company_id = auth
        .0
        .company_id
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
    auth.require_payroll_privileged()?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    Ok(Json(
        load_payroll_summary(&state.pool, company_id, id).await?,
    ))
}

pub async fn delete_run(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    auth.require_payroll_privileged()?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let run = sqlx::query_as::<_, PayrollRun>(
        r#"SELECT id, company_id, payroll_group_id, period_year, period_month,
            period_start, period_end, pay_date, status::text as status,
            total_gross, total_net, total_employer_cost,
            total_epf_employee, total_epf_employer, total_socso_employee, total_socso_employer,
            total_eis_employee, total_eis_employer, total_pcb, total_zakat,
            employee_count, version, processed_by, processed_at, approved_by, approved_at,
            locked_at, locked_by, notes, created_at, updated_at, created_by, updated_by
        FROM payroll_runs
        WHERE id = $1 AND company_id = $2"#,
    )
    .bind(id)
    .bind(company_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Payroll run not found".into()))?;

    if ["approved", "paid"].contains(&run.status.as_str()) {
        return Err(AppError::BadRequest(
            "Approved or paid payroll runs cannot be deleted".into(),
        ));
    }

    if run.status == "processing" {
        return Err(AppError::BadRequest(
            "Payroll run is currently processing and cannot be deleted".into(),
        ));
    }

    let mut tx = state.pool.begin().await?;

    sqlx::query(
        r#"UPDATE payroll_entries
        SET is_processed = FALSE, payroll_run_id = NULL, updated_at = NOW(), updated_by = $3
        WHERE payroll_run_id = $1 AND company_id = $2"#,
    )
    .bind(id)
    .bind(company_id)
    .bind(auth.0.sub)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"DELETE FROM payroll_item_details pid
        USING payroll_items pi
        WHERE pid.payroll_item_id = pi.id
          AND pi.payroll_run_id = $1"#,
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;

    sqlx::query("DELETE FROM payroll_items WHERE payroll_run_id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;

    sqlx::query("DELETE FROM payroll_runs WHERE id = $1 AND company_id = $2")
        .bind(id)
        .bind(company_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    Ok(Json(serde_json::json!({ "success": true })))
}

pub async fn update_item_pcb(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((run_id, employee_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<UpdatePayrollPcbRequest>,
) -> AppResult<Json<PayrollSummary>> {
    auth.require_payroll_privileged()?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    if req.pcb_amount < 0 {
        return Err(AppError::BadRequest("PCB amount cannot be negative".into()));
    }

    let mut tx = state.pool.begin().await?;

    let (run_status, period_year, period_month) = sqlx::query_as::<_, (String, i32, i32)>(
        r#"SELECT status::text, period_year, period_month
        FROM payroll_runs
        WHERE id = $1 AND company_id = $2
        FOR UPDATE"#,
    )
    .bind(run_id)
    .bind(company_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Payroll run not found".into()))?;

    if run_status != "processed" {
        return Err(AppError::BadRequest(
            "PCB can only be edited while the payroll run is processed and not yet approved".into(),
        ));
    }

    let has_later_run = sqlx::query_scalar::<_, bool>(
        r#"SELECT EXISTS(
            SELECT 1
            FROM payroll_items pi
            JOIN payroll_runs pr ON pi.payroll_run_id = pr.id
            WHERE pi.employee_id = $1
              AND pr.company_id = $2
              AND pr.status::text IN ('processed', 'approved', 'paid')
              AND (pr.period_year > $3 OR (pr.period_year = $3 AND pr.period_month > $4))
        )"#,
    )
    .bind(employee_id)
    .bind(company_id)
    .bind(period_year)
    .bind(period_month)
    .fetch_one(&mut *tx)
    .await?;

    if has_later_run {
        return Err(AppError::BadRequest(
            "PCB cannot be edited because a later payroll run already exists for this employee"
                .into(),
        ));
    }

    let current = sqlx::query_as::<_, (i64, i64, i64, i64)>(
        r#"SELECT pcb_amount, total_deductions, net_salary, ytd_pcb
        FROM payroll_items
        WHERE payroll_run_id = $1 AND employee_id = $2
        FOR UPDATE"#,
    )
    .bind(run_id)
    .bind(employee_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Payroll item not found".into()))?;

    let (old_pcb, total_deductions, net_salary, ytd_pcb) = current;
    let delta = req.pcb_amount - old_pcb;

    sqlx::query(
        r#"UPDATE payroll_items
        SET pcb_amount = $3,
            total_deductions = $4,
            net_salary = $5,
            ytd_pcb = $6,
            updated_at = NOW()
        WHERE payroll_run_id = $1 AND employee_id = $2"#,
    )
    .bind(run_id)
    .bind(employee_id)
    .bind(req.pcb_amount)
    .bind(total_deductions + delta)
    .bind(net_salary - delta)
    .bind(ytd_pcb + delta)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"UPDATE payroll_runs
        SET total_pcb = total_pcb + $3,
            total_net = total_net - $3,
            updated_at = NOW(),
            updated_by = $4
        WHERE id = $1 AND company_id = $2"#,
    )
    .bind(run_id)
    .bind(company_id)
    .bind(delta)
    .bind(auth.0.sub)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(
        load_payroll_summary(&state.pool, company_id, run_id).await?,
    ))
}

pub async fn approve_run(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<PayrollRun>> {
    auth.require_payroll_privileged()?;
    let company_id = auth
        .0
        .company_id
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
    .ok_or_else(|| {
        AppError::BadRequest("Payroll run not found or not in processed status".into())
    })?;

    Ok(Json(run))
}

pub async fn lock_run(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<PayrollRun>> {
    auth.require_payroll_privileged()?;
    let company_id = auth
        .0
        .company_id
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
    auth.require_payroll_privileged()?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let groups = sqlx::query_as::<_, PayrollGroup>(
        "SELECT * FROM payroll_groups WHERE company_id = $1 AND is_active = TRUE ORDER BY name",
    )
    .bind(company_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(groups))
}

#[derive(Debug, Deserialize)]
pub struct PayrollEntryQuery {
    pub period_year: Option<i32>,
    pub period_month: Option<i32>,
    pub employee_id: Option<Uuid>,
    pub item_type: Option<String>,
    pub include_processed: Option<bool>,
}

fn validate_payroll_entry(
    period_year: i32,
    period_month: i32,
    category: &str,
    item_type: &str,
    description: &str,
    amount: i64,
) -> AppResult<()> {
    if !(1..=12).contains(&period_month) {
        return Err(AppError::BadRequest("Payroll month must be 1-12".into()));
    }

    if !(1900..=3000).contains(&period_year) {
        return Err(AppError::BadRequest("Payroll year is invalid".into()));
    }

    if !["earning", "deduction"].contains(&category) {
        return Err(AppError::BadRequest(
            "Category must be earning or deduction".into(),
        ));
    }

    if item_type.trim().is_empty() {
        return Err(AppError::BadRequest("Item type is required".into()));
    }

    if description.trim().is_empty() {
        return Err(AppError::BadRequest("Description is required".into()));
    }

    if amount <= 0 {
        return Err(AppError::BadRequest(
            "Amount must be greater than zero".into(),
        ));
    }

    Ok(())
}

async fn ensure_employee_in_company(
    pool: &sqlx::PgPool,
    company_id: Uuid,
    employee_id: Uuid,
) -> AppResult<()> {
    let exists = sqlx::query_scalar::<_, bool>(
        r#"SELECT EXISTS(
            SELECT 1 FROM employees
            WHERE id = $1 AND company_id = $2 AND deleted_at IS NULL
        )"#,
    )
    .bind(employee_id)
    .bind(company_id)
    .fetch_one(pool)
    .await?;

    if !exists {
        return Err(AppError::NotFound(
            "Employee not found in the active company".into(),
        ));
    }

    Ok(())
}

pub async fn list_entries(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<PayrollEntryQuery>,
) -> AppResult<Json<Vec<PayrollEntryWithEmployee>>> {
    auth.require_payroll_privileged()?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;
    let include_processed = q.include_processed.unwrap_or(false);

    let entries = sqlx::query_as::<_, PayrollEntryWithEmployee>(
        r#"SELECT pe.id, pe.employee_id, pe.company_id, pe.period_year, pe.period_month,
            pe.category, pe.item_type, pe.description, pe.amount, pe.quantity, pe.rate,
            pe.is_taxable, pe.is_processed, pe.payroll_run_id, pe.created_at, pe.updated_at,
            pe.created_by, pe.updated_by,
            e.full_name as employee_name, e.employee_number
        FROM payroll_entries pe
        JOIN employees e ON pe.employee_id = e.id
        WHERE pe.company_id = $1
          AND ($2::int IS NULL OR pe.period_year = $2)
          AND ($3::int IS NULL OR pe.period_month = $3)
          AND ($4::uuid IS NULL OR pe.employee_id = $4)
          AND ($5::text IS NULL OR pe.item_type = $5)
          AND ($6::bool = TRUE OR pe.is_processed = FALSE)
        ORDER BY pe.period_year DESC, pe.period_month DESC, e.employee_number, pe.created_at DESC"#,
    )
    .bind(company_id)
    .bind(q.period_year)
    .bind(q.period_month)
    .bind(q.employee_id)
    .bind(q.item_type.as_deref())
    .bind(include_processed)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(entries))
}

pub async fn create_entry(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CreatePayrollEntryRequest>,
) -> AppResult<Json<PayrollEntry>> {
    auth.require_payroll_privileged()?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    validate_payroll_entry(
        req.period_year,
        req.period_month,
        &req.category,
        &req.item_type,
        &req.description,
        req.amount,
    )?;
    ensure_employee_in_company(&state.pool, company_id, req.employee_id).await?;

    let entry = sqlx::query_as::<_, PayrollEntry>(
        r#"INSERT INTO payroll_entries
            (employee_id, company_id, period_year, period_month, category, item_type,
             description, amount, quantity, rate, is_taxable, created_by)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, COALESCE($11, TRUE), $12)
        RETURNING *"#,
    )
    .bind(req.employee_id)
    .bind(company_id)
    .bind(req.period_year)
    .bind(req.period_month)
    .bind(req.category.trim())
    .bind(req.item_type.trim())
    .bind(req.description.trim())
    .bind(req.amount)
    .bind(req.quantity)
    .bind(req.rate)
    .bind(req.is_taxable)
    .bind(auth.0.sub)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(entry))
}

pub async fn update_entry(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdatePayrollEntryRequest>,
) -> AppResult<Json<PayrollEntry>> {
    auth.require_payroll_privileged()?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let current = sqlx::query_as::<_, PayrollEntry>(
        "SELECT * FROM payroll_entries WHERE id = $1 AND company_id = $2 AND is_processed = FALSE",
    )
    .bind(id)
    .bind(company_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("Payroll entry not found or already processed".into()))?;

    let employee_id = req.employee_id.unwrap_or(current.employee_id);
    let period_year = req.period_year.unwrap_or(current.period_year);
    let period_month = req.period_month.unwrap_or(current.period_month);
    let category = req.category.unwrap_or(current.category);
    let item_type = req.item_type.unwrap_or(current.item_type);
    let description = req.description.unwrap_or(current.description);
    let amount = req.amount.unwrap_or(current.amount);

    validate_payroll_entry(
        period_year,
        period_month,
        &category,
        &item_type,
        &description,
        amount,
    )?;
    ensure_employee_in_company(&state.pool, company_id, employee_id).await?;

    let updated = sqlx::query_as::<_, PayrollEntry>(
        r#"UPDATE payroll_entries
        SET employee_id = $3,
            period_year = $4,
            period_month = $5,
            category = $6,
            item_type = $7,
            description = $8,
            amount = $9,
            quantity = $10,
            rate = $11,
            is_taxable = COALESCE($12, is_taxable),
            updated_by = $13,
            updated_at = NOW()
        WHERE id = $1 AND company_id = $2 AND is_processed = FALSE
        RETURNING *"#,
    )
    .bind(id)
    .bind(company_id)
    .bind(employee_id)
    .bind(period_year)
    .bind(period_month)
    .bind(category.trim())
    .bind(item_type.trim())
    .bind(description.trim())
    .bind(amount)
    .bind(req.quantity.or(current.quantity))
    .bind(req.rate.or(current.rate))
    .bind(req.is_taxable)
    .bind(auth.0.sub)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(updated))
}

pub async fn delete_entry(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    auth.require_payroll_privileged()?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let rows = sqlx::query(
        "DELETE FROM payroll_entries WHERE id = $1 AND company_id = $2 AND is_processed = FALSE",
    )
    .bind(id)
    .bind(company_id)
    .execute(&state.pool)
    .await?
    .rows_affected();

    if rows == 0 {
        return Err(AppError::BadRequest(
            "Payroll entry not found or already processed".into(),
        ));
    }

    Ok(Json(serde_json::json!({ "success": true })))
}

pub async fn download_run_payslips_pdf(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(run_id): Path<Uuid>,
) -> Result<axum::response::Response, crate::core::error::AppError> {
    use axum::body::Body;
    use axum::http::{Response, StatusCode, header};

    auth.require_payroll_privileged()?;
    let company_id = auth
        .0
        .company_id
        .ok_or_else(|| AppError::Forbidden("No company assigned".into()))?;

    let bytes = crate::services::payslip_pdf_service::generate_bulk_payslips(
        &state.pool,
        run_id,
        company_id,
    )
    .await?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/pdf")
        .header(
            header::CONTENT_DISPOSITION,
            "attachment; filename=\"payslips.pdf\"",
        )
        .body(Body::from(bytes))
        .unwrap())
}

pub async fn get_items(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Vec<PayrollItem>>> {
    auth.require_payroll_privileged()?;
    let items = sqlx::query_as::<_, PayrollItem>(
        "SELECT * FROM payroll_items WHERE payroll_run_id = $1 ORDER BY created_at",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(items))
}
