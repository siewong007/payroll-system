//! Data access for the `tp3_records` table.

use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::models::employee::{CreateTp3Request, Tp3Record};

pub async fn upsert(
    executor: impl Executor<'_, Database = Postgres>,
    id: Uuid,
    employee_id: Uuid,
    req: &CreateTp3Request,
    created_by: Uuid,
) -> AppResult<Tp3Record> {
    let record = sqlx::query_as!(
        Tp3Record,
        r#"INSERT INTO tp3_records (
            id, employee_id, tax_year, previous_employer_name,
            previous_income_ytd, previous_epf_ytd, previous_pcb_ytd,
            previous_socso_ytd, previous_zakat_ytd, created_by
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        ON CONFLICT (employee_id, tax_year)
        DO UPDATE SET
            previous_employer_name = EXCLUDED.previous_employer_name,
            previous_income_ytd = EXCLUDED.previous_income_ytd,
            previous_epf_ytd = EXCLUDED.previous_epf_ytd,
            previous_pcb_ytd = EXCLUDED.previous_pcb_ytd,
            previous_socso_ytd = EXCLUDED.previous_socso_ytd,
            previous_zakat_ytd = EXCLUDED.previous_zakat_ytd
        RETURNING *"#,
        id,
        employee_id,
        req.tax_year,
        req.previous_employer_name,
        req.previous_income_ytd,
        req.previous_epf_ytd,
        req.previous_pcb_ytd,
        req.previous_socso_ytd,
        req.previous_zakat_ytd.unwrap_or(0),
        created_by,
    )
    .fetch_one(executor)
    .await?;
    Ok(record)
}
