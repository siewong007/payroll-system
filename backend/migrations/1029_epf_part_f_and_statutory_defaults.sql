-- Align statutory constraints and tenant defaults with 2025-2026 Malaysian law:
--
--   1. Employees Provident Fund (Amendment) Act 2025 (effective 1 October 2025):
--      introduces Third Schedule Part F (2% employee + 2% employer) for
--      non-Malaysian citizen employees under age 75 holding valid work passes.
--      Migration 1022 restricted `employees.epf_category` to A-E; expand the
--      CHECK constraint to A-F so Part F overrides and imports are accepted.
--
--   2. Minimum Wages Order 2024 [P.U. (A) 376/2024] under the National Wages
--      Consultative Council Act 2011 (Act 732): sets the national monthly
--      minimum wage floor at RM1,700 (effective 1 Feb 2025 for >=5 employees
--      and MASCO professional employers; universal across all employers from
--      1 Aug 2025). Seed `statutory/minimum_wage_monthly` and
--      `statutory/epf_foreign_worker_rate` on existing and newly provisioned
--      companies without overwriting any tenant-customized value.

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'employees_epf_category_check'
          AND conrelid = 'public.employees'::regclass
    ) THEN
        ALTER TABLE public.employees
            DROP CONSTRAINT employees_epf_category_check;
    END IF;

    ALTER TABLE public.employees
        ADD CONSTRAINT employees_epf_category_check
        CHECK (epf_category IS NULL OR epf_category IN ('A', 'B', 'C', 'D', 'E', 'F')) NOT VALID;
END $$;

DO $$
DECLARE bad bigint;
BEGIN
    SELECT count(*) INTO bad FROM public.employees
    WHERE epf_category IS NOT NULL AND epf_category NOT IN ('A', 'B', 'C', 'D', 'E', 'F');
    IF bad = 0 THEN
        ALTER TABLE public.employees VALIDATE CONSTRAINT employees_epf_category_check;
    ELSE
        RAISE WARNING 'employees_epf_category_check left NOT VALID: % row(s) outside A-F', bad;
    END IF;
END $$;

INSERT INTO public.company_settings (company_id, category, key, value, label, description)
SELECT c.id,
       defaults.category,
       defaults.key,
       defaults.value::jsonb,
       defaults.label,
       defaults.description
FROM public.companies c
CROSS JOIN (VALUES
    ('statutory', 'minimum_wage_monthly', '"1700"', 'Statutory Minimum Wage (RM)', 'Monthly minimum wage floor under Minimum Wages Order 2024 (P.U. (A) 376/2024)'),
    ('statutory', 'epf_foreign_worker_rate', '"2"', 'EPF Foreign Worker Rate (%)', 'Reference Part F rate (2% employee + 2% employer) under EPF Amendment Act 2025')
) AS defaults(category, key, value, label, description)
ON CONFLICT (company_id, category, key) DO NOTHING;

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
            (p_company_id, 'Annual Leave', 'Paid annual leave entitlement (EA 1955 s.60E)', 14, true, true),
            (p_company_id, 'Sick Leave', 'Paid outpatient sick leave (EA 1955 s.60F)', 14, true, true),
            (p_company_id, 'Hospitalisation Leave', 'Paid hospitalisation leave (EA 1955 s.60F)', 60, true, true),
            (p_company_id, 'Compassionate Leave', 'Bereavement / compassionate leave', 3, true, true),
            (p_company_id, 'Maternity Leave', 'Paid maternity leave (EA 1955 s.37 — 98 consecutive days)', 98, true, true),
            (p_company_id, 'Paternity Leave', 'Paid paternity leave (EA 1955 s.60FA — 7 consecutive days)', 7, true, true),
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
        ('payroll', 'default_pay_day', '"28"', 'Default Pay Day', 'Day of month for salary payment (within 7 days of period end per EA 1955 s.19)'),
        ('payroll', 'default_cutoff_day', '"25"', 'Default Cutoff Day', 'Day of month for payroll cutoff'),
        ('payroll', 'overtime_multiplier_normal', '"1.5"', 'OT Multiplier (Normal)', 'Overtime rate multiplier for normal working days (EA 1955 s.60A(3))'),
        ('payroll', 'overtime_multiplier_rest', '"2.0"', 'OT Multiplier (Rest Day)', 'Overtime rate multiplier for rest days (EA 1955 s.60(3))'),
        ('payroll', 'overtime_multiplier_public', '"3.0"', 'OT Multiplier (Public Holiday)', 'Overtime rate multiplier for public holidays (EA 1955 s.60D(3))'),
        ('payroll', 'working_days_per_month', '"26"', 'Working Days Per Month', 'Divisor used to derive the ORP / hourly rate for overtime (EA 1955 s.60I)'),
        ('payroll', 'rounding_method', '"nearest"', 'Rounding Method', 'Salary calculation rounding method'),
        ('payroll', 'working_hours_per_day', '"9"', 'Working Hours Per Day', 'Office hours per day including rest time'),
        ('payroll', 'rest_time_minutes', '"60"', 'Rest Time (minutes)', 'Daily rest time'),
        ('payroll', 'effective_hours_per_day', '"8"', 'Effective Hours Per Day', 'Working hours after rest time (max 45h/week per EA 1955 s.60A(1))'),
        ('statutory', 'minimum_wage_monthly', '"1700"', 'Statutory Minimum Wage (RM)', 'Monthly minimum wage floor under Minimum Wages Order 2024 (P.U. (A) 376/2024)'),
        ('statutory', 'epf_employer_rate_below_60', '"13"', 'EPF Employer Rate (< 60)', 'Reference Part A employer rate percentage (<= RM5,000; 12% above RM5,000)'),
        ('statutory', 'epf_employer_rate_above_60', '"4"', 'EPF Employer Rate (>= 60 Citizen)', 'Reference Part E employer rate percentage for Malaysian citizens aged 60-74'),
        ('statutory', 'epf_foreign_worker_rate', '"2"', 'EPF Foreign Worker Rate (%)', 'Reference Part F rate (2% employee + 2% employer) under EPF Amendment Act 2025'),
        ('statutory', 'socso_enabled', 'true', 'SOCSO Enabled', 'Whether SOCSO (Act 4, RM6,000 ceiling) calculations are enabled'),
        ('statutory', 'eis_enabled', 'true', 'EIS Enabled', 'Whether EIS (Act 800, RM6,000 ceiling) calculations are enabled'),
        ('statutory', 'hrdf_enabled', 'false', 'HRDF Enabled', 'Whether HRD Corp levy (Act 612) tracking is enabled'),
        ('statutory', 'hrdf_rate', '"1"', 'HRDF Rate (%)', 'HRD Corp levy percentage (1% mandatory >=10 Malaysian employees; 0.5% optional 5-9)'),
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
