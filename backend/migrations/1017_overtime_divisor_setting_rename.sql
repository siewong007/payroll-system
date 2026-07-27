-- The `payroll/unpaid_leave_divisor` company setting has never had anything to
-- do with unpaid leave. It is read in exactly one place — the divisor that turns
-- a monthly basic salary into an OVERTIME hourly rate — so editing a field
-- labelled "Unpaid Leave Divisor" changed every overtime payment and nothing
-- about unpaid leave. Renamed here to `working_days_per_month`, carrying each
-- company's current value forward. The service keeps reading the old key as a
-- fallback for one release, for a settings row restored from an older backup.
--
-- The seed row at 1000_schema.sql:3191 and the function that writes it are
-- frozen; this migration supersedes both.

-- 1. Carry the existing value onto the new key, per company.
INSERT INTO company_settings (company_id, category, key, value, label, description)
SELECT company_id,
       'payroll',
       'working_days_per_month',
       value,
       'Working Days Per Month',
       'Divisor used to derive the hourly rate for overtime'
FROM company_settings
WHERE category = 'payroll' AND key = 'unpaid_leave_divisor'
ON CONFLICT (company_id, category, key) DO NOTHING;

-- 2. Retire the misleading key.
DELETE FROM company_settings
WHERE category = 'payroll' AND key = 'unpaid_leave_divisor';

-- 3. Same rename in the per-company provisioning template, so a company created
-- after this migration gets the new key. Reproduced in full because
-- 1000_schema.sql is deployed and immutable; only the one settings row differs.
CREATE OR REPLACE FUNCTION public.provision_company_defaults(
    p_company_id uuid,
    p_actor_id uuid DEFAULT NULL
) RETURNS void
LANGUAGE plpgsql
SET search_path = public, pg_temp
AS $company_defaults$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM payroll_groups WHERE company_id = p_company_id
    ) THEN
        INSERT INTO payroll_groups (
            company_id, name, description, cutoff_day, payment_day, created_by, updated_by
        ) VALUES (
            p_company_id, 'Default', 'Default monthly payroll group', 25, 28,
            p_actor_id, p_actor_id
        )
        ON CONFLICT DO NOTHING;
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM leave_types WHERE company_id = p_company_id
    ) THEN
        INSERT INTO leave_types (
            company_id, name, description, default_days, is_paid, is_system
        ) VALUES
            (p_company_id, 'Annual Leave', 'Paid annual leave entitlement', 14, true, true),
            (p_company_id, 'Sick Leave', 'Paid sick leave (outpatient)', 14, true, true),
            (p_company_id, 'Hospitalisation Leave', 'Paid hospitalisation leave', 60, true, true),
            (p_company_id, 'Compassionate Leave', 'Bereavement / compassionate leave', 3, true, true),
            (p_company_id, 'Maternity Leave', 'Paid maternity leave', 98, true, true),
            (p_company_id, 'Paternity Leave', 'Paid paternity leave', 7, true, true),
            (p_company_id, 'Marriage Leave', 'Leave for own marriage', 3, true, true),
            (p_company_id, 'Unpaid Leave', 'Unpaid leave', 365, false, true)
        ON CONFLICT (company_id, name) DO NOTHING;
    END IF;

    INSERT INTO working_day_config (company_id, day_of_week, is_working_day)
    VALUES
        (p_company_id, 0, false),
        (p_company_id, 1, true),
        (p_company_id, 2, true),
        (p_company_id, 3, true),
        (p_company_id, 4, true),
        (p_company_id, 5, true),
        (p_company_id, 6, false)
    ON CONFLICT (company_id, day_of_week) DO NOTHING;

    INSERT INTO company_work_schedules (
        company_id, name, start_time, end_time, grace_minutes,
        half_day_hours, timezone, is_default
    ) VALUES (
        p_company_id, 'Default', '09:00', '18:00', 15,
        4.0, 'Asia/Kuala_Lumpur', true
    )
    ON CONFLICT (company_id) WHERE is_default = true DO NOTHING;

    INSERT INTO company_settings (
        company_id, category, key, value, label, description
    )
    SELECT p_company_id, defaults.category, defaults.key,
           defaults.value::jsonb, defaults.label, defaults.description
    FROM (VALUES
        ('payroll', 'default_pay_day', '"28"', 'Default Pay Day', 'Day of month for salary payment'),
        ('payroll', 'default_cutoff_day', '"25"', 'Default Cutoff Day', 'Day of month for payroll cutoff'),
        ('payroll', 'overtime_multiplier_normal', '"1.5"', 'OT Multiplier (Normal)', 'Overtime rate multiplier for normal working days'),
        ('payroll', 'overtime_multiplier_rest', '"2.0"', 'OT Multiplier (Rest Day)', 'Overtime rate multiplier for rest days'),
        ('payroll', 'overtime_multiplier_public', '"3.0"', 'OT Multiplier (Public Holiday)', 'Overtime rate multiplier for public holidays'),
        ('payroll', 'working_days_per_month', '"26"', 'Working Days Per Month', 'Divisor used to derive the hourly rate for overtime'),
        ('payroll', 'rounding_method', '"nearest"', 'Rounding Method', 'Salary calculation rounding method'),
        ('payroll', 'working_hours_per_day', '"9"', 'Working Hours Per Day', 'Office hours per day including rest time'),
        ('payroll', 'rest_time_minutes', '"60"', 'Rest Time (minutes)', 'Daily rest time'),
        ('payroll', 'effective_hours_per_day', '"8"', 'Effective Hours Per Day', 'Working hours after rest time'),
        ('statutory', 'epf_employer_rate_below_60', '"13"', 'EPF Employer Rate (< 60)', 'Reference employer rate percentage'),
        ('statutory', 'epf_employer_rate_above_60', '"6.5"', 'EPF Employer Rate (>= 60)', 'Reference employer rate percentage'),
        ('statutory', 'socso_enabled', 'true', 'SOCSO Enabled', 'Whether SOCSO calculations are enabled'),
        ('statutory', 'eis_enabled', 'true', 'EIS Enabled', 'Whether EIS calculations are enabled'),
        ('statutory', 'hrdf_enabled', 'false', 'HRDF Enabled', 'Whether HRDF calculations are enabled'),
        ('statutory', 'hrdf_rate', '"1"', 'HRDF Rate (%)', 'HRDF levy percentage'),
        ('system', 'currency', '"MYR"', 'Currency', 'System currency code'),
        ('system', 'date_format', '"DD/MM/YYYY"', 'Date Format', 'Display date format'),
        ('system', 'financial_year_start_month', '"1"', 'Financial Year Start', 'Financial year start month'),
        ('system', 'payslip_template', '"default"', 'Payslip Template', 'Payslip template key'),
        ('notifications', 'email_payslip', 'true', 'Email Payslips', 'Email payslips after payroll approval'),
        ('notifications', 'expiry_alert_days', '"30"', 'Document Expiry Alert (days)', 'Document expiry warning window'),
        ('notifications', 'probation_alert_days', '"14"', 'Probation End Alert (days)', 'Probation warning window'),
        ('email', 'auto_welcome_email', 'true', 'Auto Welcome Email', 'Email an employee after account creation')
    ) AS defaults(category, key, value, label, description)
    ON CONFLICT (company_id, category, key) DO NOTHING;
END
$company_defaults$;

-- 4. `companies.unpaid_leave_divisor` starts being read by the unpaid-leave
-- deduction with this release. Until now nothing read it, so every row carries
-- the column default of 26 whether or not anyone chose it — leaving those values
-- in place would silently change what every tenant deducts per day (a 26 divisor
-- on a 21-working-day month deducts noticeably less). Blank it and drop the
-- default so the deduction keeps falling back to the calendar's working days
-- until HR deliberately sets a divisor. The CHECK (> 0) already admits NULL.
ALTER TABLE public.companies ALTER COLUMN unpaid_leave_divisor DROP DEFAULT;
UPDATE public.companies SET unpaid_leave_divisor = NULL WHERE unpaid_leave_divisor IS NOT NULL;

-- 5. `employees.epf_category` is free text with no constraint at all, and the
-- engine now derives the Third-Schedule part from age and residency and treats
-- this column as an explicit override. An override outside A-E is a typo, not a
-- policy. NOT VALID plus a conditional VALIDATE, per 1016: migrations run at
-- container start and a bare VALIDATE against one legacy row would crash-loop
-- the backend.
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint
                   WHERE conname = 'employees_epf_category_check'
                     AND conrelid = 'public.employees'::regclass) THEN
        ALTER TABLE public.employees
            ADD CONSTRAINT employees_epf_category_check
            CHECK (epf_category IS NULL OR epf_category IN ('A', 'B', 'C', 'D', 'E')) NOT VALID;
    END IF;
END $$;

DO $$
DECLARE bad bigint;
BEGIN
    SELECT count(*) INTO bad FROM public.employees
    WHERE epf_category IS NOT NULL AND epf_category NOT IN ('A', 'B', 'C', 'D', 'E');
    IF bad = 0 THEN
        ALTER TABLE public.employees VALIDATE CONSTRAINT employees_epf_category_check;
    ELSE
        RAISE WARNING 'employees_epf_category_check left NOT VALID: % row(s) outside A-E', bad;
    END IF;
END $$;
