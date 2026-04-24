//! Approval workflows for leave / claim / overtime.
//!
//! Originally a single 1,875-line `approval_service.rs`. Split by domain to
//! keep each file navigable. The public API is unchanged — callers still
//! reference `approval_service::approve_leave` etc. via these re-exports.

mod claim;
mod common;
mod leave;
mod overtime;

pub use claim::{
    ClaimWithEmployee, approve_claim, cancel_claim_admin, create_claim_admin, delete_claim_admin,
    get_claim_with_employee_by_id, get_pending_claims, reject_claim, update_claim_admin,
};
pub use leave::{
    LeaveRequestWithEmployee, approve_leave, cancel_leave_request_admin,
    create_leave_request_admin, delete_leave_request_admin, get_pending_leave_requests,
    reject_leave, update_leave_request_admin,
};
pub use overtime::{
    OvertimeWithEmployee, approve_overtime, cancel_overtime_admin, create_overtime_admin,
    delete_overtime_admin, get_overtime_with_employee_by_id, get_pending_overtime, reject_overtime,
    update_overtime_admin,
};
