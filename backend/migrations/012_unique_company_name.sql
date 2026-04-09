-- Deduplicate company names before adding unique constraint
-- Keep the oldest company (smallest created_at), remove duplicates and their data
DO $$
DECLARE
    dup_id UUID;
BEGIN
    FOR dup_id IN
        SELECT id FROM (
            SELECT id, ROW_NUMBER() OVER (PARTITION BY LOWER(name) ORDER BY created_at ASC) AS rn
            FROM companies
        ) ranked WHERE rn > 1
    LOOP
        DELETE FROM company_settings WHERE company_id = dup_id;
        DELETE FROM email_templates WHERE company_id = dup_id;
        DELETE FROM working_day_config WHERE company_id = dup_id;
        DELETE FROM holidays WHERE company_id = dup_id;
        DELETE FROM team_members WHERE team_id IN (SELECT id FROM teams WHERE company_id = dup_id);
        DELETE FROM teams WHERE company_id = dup_id;
        DELETE FROM documents WHERE company_id = dup_id;
        DELETE FROM document_categories WHERE company_id = dup_id;
        DELETE FROM payroll_entries WHERE company_id = dup_id;
        DELETE FROM payroll_item_details WHERE payroll_item_id IN (SELECT pi.id FROM payroll_items pi JOIN payroll_runs pr ON pi.payroll_run_id = pr.id WHERE pr.company_id = dup_id);
        DELETE FROM payroll_items WHERE payroll_run_id IN (SELECT id FROM payroll_runs WHERE company_id = dup_id);
        DELETE FROM payroll_runs WHERE company_id = dup_id;
        DELETE FROM overtime_applications WHERE company_id = dup_id;
        DELETE FROM claims WHERE company_id = dup_id;
        DELETE FROM leave_requests WHERE company_id = dup_id;
        DELETE FROM leave_balances WHERE employee_id IN (SELECT id FROM employees WHERE company_id = dup_id);
        DELETE FROM leave_types WHERE company_id = dup_id;
        DELETE FROM tp3_records WHERE employee_id IN (SELECT id FROM employees WHERE company_id = dup_id);
        DELETE FROM salary_history WHERE employee_id IN (SELECT id FROM employees WHERE company_id = dup_id);
        DELETE FROM employee_allowances WHERE employee_id IN (SELECT id FROM employees WHERE company_id = dup_id);
        DELETE FROM notifications WHERE company_id = dup_id;
        UPDATE users SET employee_id = NULL WHERE employee_id IN (SELECT id FROM employees WHERE company_id = dup_id);
        DELETE FROM employees WHERE company_id = dup_id;
        DELETE FROM payroll_groups WHERE company_id = dup_id;
        DELETE FROM user_companies WHERE company_id = dup_id;
        UPDATE users SET company_id = NULL WHERE company_id = dup_id;
        DELETE FROM companies WHERE id = dup_id;
    END LOOP;
END $$;

-- Make company name unique (case-insensitive)
CREATE UNIQUE INDEX idx_companies_name_unique ON companies (LOWER(name));
