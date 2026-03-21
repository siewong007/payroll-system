CREATE TABLE company_settings (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    company_id UUID NOT NULL REFERENCES companies(id),
    category VARCHAR(50) NOT NULL,
    key VARCHAR(100) NOT NULL,
    value JSONB NOT NULL,
    label VARCHAR(255),
    description VARCHAR(500),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_by UUID,
    UNIQUE(company_id, category, key)
);

CREATE INDEX idx_company_settings_company ON company_settings(company_id);
CREATE INDEX idx_company_settings_category ON company_settings(company_id, category);

-- Seed default settings for existing companies
INSERT INTO company_settings (company_id, category, key, value, label, description)
SELECT c.id, d.category, d.key, d.value::jsonb, d.label, d.description
FROM companies c
CROSS JOIN (VALUES
    ('payroll', 'default_pay_day', '"28"', 'Default Pay Day', 'Day of month for salary payment'),
    ('payroll', 'default_cutoff_day', '"25"', 'Default Cutoff Day', 'Day of month for payroll cutoff'),
    ('payroll', 'overtime_multiplier_normal', '"1.5"', 'OT Multiplier (Normal)', 'Overtime rate multiplier for normal working days'),
    ('payroll', 'overtime_multiplier_rest', '"2.0"', 'OT Multiplier (Rest Day)', 'Overtime rate multiplier for rest days'),
    ('payroll', 'overtime_multiplier_public', '"3.0"', 'OT Multiplier (Public Holiday)', 'Overtime rate multiplier for public holidays'),
    ('payroll', 'unpaid_leave_divisor', '"26"', 'Unpaid Leave Divisor', 'Number of working days to divide salary for unpaid leave deduction'),
    ('payroll', 'rounding_method', '"nearest"', 'Rounding Method', 'How to round salary calculations: nearest, up, down'),
    ('statutory', 'epf_employer_rate_below_60', '"13"', 'EPF Employer Rate (< 60)', 'EPF employer contribution percentage for employees under 60'),
    ('statutory', 'epf_employer_rate_above_60', '"6.5"', 'EPF Employer Rate (>= 60)', 'EPF employer contribution percentage for employees 60 and above'),
    ('statutory', 'socso_enabled', 'true', 'SOCSO Enabled', 'Whether SOCSO contributions are active'),
    ('statutory', 'eis_enabled', 'true', 'EIS Enabled', 'Whether EIS contributions are active'),
    ('statutory', 'hrdf_enabled', 'false', 'HRDF Enabled', 'Whether HRDF levy is active'),
    ('statutory', 'hrdf_rate', '"1"', 'HRDF Rate (%)', 'HRDF levy rate percentage'),
    ('system', 'currency', '"MYR"', 'Currency', 'System currency code'),
    ('system', 'date_format', '"DD/MM/YYYY"', 'Date Format', 'Display date format across the system'),
    ('system', 'financial_year_start_month', '"1"', 'Financial Year Start', 'Month number when financial year begins (1 = January)'),
    ('system', 'payslip_template', '"default"', 'Payslip Template', 'Template used for generating payslips'),
    ('notifications', 'email_payslip', 'true', 'Email Payslips', 'Automatically email payslips to employees after payroll approval'),
    ('notifications', 'expiry_alert_days', '"30"', 'Document Expiry Alert (days)', 'Days before document expiry to send alerts'),
    ('notifications', 'probation_alert_days', '"14"', 'Probation End Alert (days)', 'Days before probation end to send alerts')
) AS d(category, key, value, label, description);
