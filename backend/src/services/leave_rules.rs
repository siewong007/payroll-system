use chrono::NaiveDate;
use rust_decimal::Decimal;

use crate::core::error::{AppError, AppResult};

pub fn validate_period(start_date: NaiveDate, end_date: NaiveDate, days: Decimal) -> AppResult<()> {
    if start_date > end_date {
        return Err(AppError::BadRequest(
            "Leave start date must not be after the end date".into(),
        ));
    }
    if days <= Decimal::ZERO {
        return Err(AppError::BadRequest(
            "Leave days must be greater than zero".into(),
        ));
    }
    Ok(())
}
