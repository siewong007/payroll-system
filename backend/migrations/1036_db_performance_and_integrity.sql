-- Performance indexes, duplicate index cleanup, integrity checks, and extended statistics.
-- Every statement is idempotent so re-running or applying against an existing cluster is safe.

-- 1. Drop duplicate, superseded, or unscoped indexes.
DROP INDEX IF EXISTS public.idx_payroll_item_details_item;
DROP INDEX IF EXISTS public.idx_payroll_item_details;
DROP INDEX IF EXISTS public.idx_oauth2_accounts_user;
DROP INDEX IF EXISTS public.idx_claims_approved_payroll;
DROP INDEX IF EXISTS public.idx_holidays_company_year;
DROP INDEX IF EXISTS public.idx_payroll_runs_period;

-- 2. Upgrade default work-schedule unique index to covering for index-only scans
--    while keeping the unique constraint required by `ON CONFLICT (company_id) WHERE is_default = TRUE`.
DROP INDEX IF EXISTS public.idx_work_schedules_default;
CREATE UNIQUE INDEX IF NOT EXISTS idx_work_schedules_default
    ON public.company_work_schedules (company_id)
    INCLUDE (timezone, start_time, end_time, grace_minutes, unpaid_break_minutes)
    WHERE (is_default = true);

-- 3. Payroll & statutory performance indexes.
CREATE INDEX IF NOT EXISTS idx_payroll_item_details_item_sort
    ON public.payroll_item_details (payroll_item_id, category DESC, is_statutory, id);

CREATE INDEX IF NOT EXISTS idx_payroll_entries_company_unprocessed
    ON public.payroll_entries (company_id, period_year, period_month)
    WHERE is_processed = FALSE;

CREATE INDEX IF NOT EXISTS idx_payroll_entries_company_period_created
    ON public.payroll_entries (company_id, period_year DESC, period_month DESC, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_payroll_entries_emp_period_unprocessed
    ON public.payroll_entries (employee_id, period_year, period_month)
    WHERE is_processed = FALSE;

CREATE INDEX IF NOT EXISTS idx_payroll_items_employee_run
    ON public.payroll_items (employee_id, payroll_run_id);

CREATE INDEX IF NOT EXISTS idx_payroll_items_run_created
    ON public.payroll_items (payroll_run_id, created_at, id);

CREATE INDEX IF NOT EXISTS idx_payroll_runs_company_active_dates
    ON public.payroll_runs (company_id, period_start, period_end)
    WHERE status IN ('pending_approval'::payroll_status, 'approved'::payroll_status, 'paid'::payroll_status);

CREATE INDEX IF NOT EXISTS idx_payroll_runs_company_awaiting
    ON public.payroll_runs (company_id, period_year DESC, period_month DESC)
    WHERE status IN ('processed'::payroll_status, 'pending_approval'::payroll_status, 'approved'::payroll_status);

CREATE INDEX IF NOT EXISTS idx_employee_allowances_emp_recurring
    ON public.employee_allowances (employee_id, effective_from, effective_to)
    WHERE is_active = TRUE AND is_recurring = TRUE;

CREATE INDEX IF NOT EXISTS idx_salary_history_emp_effective
    ON public.salary_history (employee_id, effective_date DESC, created_at DESC);

-- 4. Claims, leave, overtime, attendance, calendar & location indexes.
CREATE INDEX IF NOT EXISTS idx_claims_company_expense_date
    ON public.claims (company_id, expense_date);

CREATE INDEX IF NOT EXISTS idx_claims_company_status_created
    ON public.claims (company_id, status, created_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_claims_company_created
    ON public.claims (company_id, created_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_claims_employee_created
    ON public.claims (employee_id, created_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_leave_balances_employee_year
    ON public.leave_balances (employee_id, year);

CREATE INDEX IF NOT EXISTS idx_leave_balances_year_employee
    ON public.leave_balances (year, employee_id);

CREATE INDEX IF NOT EXISTS idx_leave_requests_emp_active_dates
    ON public.leave_requests (employee_id, start_date, end_date)
    WHERE status IN ('pending', 'approved');

CREATE INDEX IF NOT EXISTS idx_leave_requests_company_status_created
    ON public.leave_requests (company_id, status, created_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_leave_requests_company_created
    ON public.leave_requests (company_id, created_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_leave_requests_employee_created
    ON public.leave_requests (employee_id, created_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_overtime_emp_active_date
    ON public.overtime_applications (employee_id, ot_date)
    WHERE status NOT IN ('rejected', 'cancelled');

CREATE INDEX IF NOT EXISTS idx_overtime_company_status_created
    ON public.overtime_applications (company_id, status, created_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_overtime_company_created
    ON public.overtime_applications (company_id, created_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_overtime_employee_created
    ON public.overtime_applications (employee_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_attendance_company_outside_geofence
    ON public.attendance_records (company_id, check_in_at DESC)
    WHERE is_outside_geofence = TRUE;

CREATE INDEX IF NOT EXISTS idx_attendance_company_open_active
    ON public.attendance_records (company_id, check_in_at DESC)
    WHERE check_out_at IS NULL AND status <> 'absent';

CREATE INDEX IF NOT EXISTS idx_attendance_qr_tokens_expires_at
    ON public.attendance_qr_tokens (expires_at);

CREATE INDEX IF NOT EXISTS idx_holidays_company_recurring_md
    ON public.holidays (company_id, (EXTRACT(MONTH FROM date)::int), (EXTRACT(DAY FROM date)::int), date)
    WHERE is_recurring = TRUE;

CREATE INDEX IF NOT EXISTS idx_company_locations_company_active
    ON public.company_locations (company_id)
    INCLUDE (latitude, longitude, radius_meters, name)
    WHERE is_active = TRUE;

-- 5. Employees, users, notifications, documents, uploads, jobs, audit & outbox indexes.
CREATE INDEX IF NOT EXISTS idx_employees_company_active_dept
    ON public.employees (company_id, is_active, department)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_employees_company_email_lower
    ON public.employees (company_id, lower(btrim(email)))
    WHERE deleted_at IS NULL AND email IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_notifications_user_company_created
    ON public.notifications (user_id, company_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_notifications_user_company_unread
    ON public.notifications (user_id, company_id)
    WHERE is_read = FALSE;

CREATE INDEX IF NOT EXISTS idx_documents_company_active_created
    ON public.documents (company_id, created_at DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_documents_company_active_expiry
    ON public.documents (company_id, expiry_date)
    WHERE deleted_at IS NULL AND expiry_date IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_documents_search_trgm
    ON public.documents USING gin (lower((title::text || ' ' || file_name::text)) gin_trgm_ops)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_users_active_created
    ON public.users (created_at DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_users_search_trgm
    ON public.users USING gin (lower((full_name::text || ' ' || email::text)) gin_trgm_ops)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_users_employee_all
    ON public.users (employee_id)
    WHERE employee_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_uploaded_files_created_at
    ON public.uploaded_files (created_at);

CREATE INDEX IF NOT EXISTS idx_background_jobs_company_active
    ON public.background_jobs (company_id)
    WHERE status IN ('pending', 'running');

CREATE INDEX IF NOT EXISTS idx_audit_logs_payroll_item_run_new
    ON public.audit_logs (company_id, (new_values->>'payroll_run_id'))
    WHERE entity_type = 'payroll_item' AND new_values ? 'payroll_run_id';

CREATE INDEX IF NOT EXISTS idx_audit_logs_payroll_item_run_old
    ON public.audit_logs (company_id, (old_values->>'payroll_run_id'))
    WHERE entity_type = 'payroll_item' AND old_values ? 'payroll_run_id';

CREATE INDEX IF NOT EXISTS idx_push_outbox_pending_user_retry
    ON public.push_outbox (user_id, next_retry_at, created_at)
    WHERE status = 'pending';

CREATE INDEX IF NOT EXISTS idx_email_logs_outbox_unscheduled
    ON public.email_logs (created_at)
    WHERE status = 'pending' AND next_retry_at IS NULL;

-- 6. Integrity constraints.
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conrelid = 'public.leave_balances'::regclass
          AND conname = 'leave_balances_non_negative_check'
    ) THEN
        ALTER TABLE public.leave_balances
            ADD CONSTRAINT leave_balances_non_negative_check
            CHECK (
                entitled_days >= 0
                AND taken_days >= 0
                AND pending_days >= 0
                AND carried_forward >= 0
            );
    END IF;
END
$$;

-- 7. Extended multivariate planner statistics for correlated tenant/status filters.
CREATE STATISTICS IF NOT EXISTS public.leave_requests_tenant_status_stats
    (dependencies, mcv)
    ON company_id, status, employee_id
    FROM public.leave_requests;

CREATE STATISTICS IF NOT EXISTS public.claims_tenant_status_stats
    (dependencies, mcv)
    ON company_id, status, employee_id, payroll_run_id
    FROM public.claims;

CREATE STATISTICS IF NOT EXISTS public.overtime_tenant_status_stats
    (dependencies, mcv)
    ON company_id, status, employee_id
    FROM public.overtime_applications;

CREATE STATISTICS IF NOT EXISTS public.payroll_entries_tenant_period_stats
    (dependencies, mcv)
    ON company_id, employee_id, period_year, period_month, is_processed
    FROM public.payroll_entries;

CREATE STATISTICS IF NOT EXISTS public.notifications_user_company_read_stats
    (dependencies, mcv)
    ON user_id, company_id, is_read
    FROM public.notifications;
