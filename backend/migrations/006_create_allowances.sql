-- Recurring allowances/deductions assigned to employees
CREATE TABLE employee_allowances (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    employee_id UUID NOT NULL REFERENCES employees(id),
    category VARCHAR(20) NOT NULL CHECK (category IN ('earning', 'deduction')),
    name VARCHAR(100) NOT NULL,
    description VARCHAR(255),
    amount BIGINT NOT NULL, -- in sen
    is_taxable BOOLEAN DEFAULT TRUE,
    is_recurring BOOLEAN DEFAULT TRUE,
    effective_from DATE NOT NULL,
    effective_to DATE, -- NULL = ongoing
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_by UUID,
    updated_by UUID
);

-- Staged variable entries for upcoming payroll (OT, bonus, ad-hoc)
CREATE TABLE payroll_entries (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    employee_id UUID NOT NULL REFERENCES employees(id),
    company_id UUID NOT NULL REFERENCES companies(id),
    period_year INTEGER NOT NULL,
    period_month INTEGER NOT NULL,
    category VARCHAR(20) NOT NULL CHECK (category IN ('earning', 'deduction')),
    item_type VARCHAR(50) NOT NULL, -- overtime, bonus, commission, back_pay, etc.
    description VARCHAR(255) NOT NULL,
    amount BIGINT NOT NULL, -- in sen
    quantity NUMERIC(10,2), -- hours for OT, days, etc.
    rate BIGINT, -- rate in sen (e.g. OT rate per hour)
    is_taxable BOOLEAN DEFAULT TRUE,
    is_processed BOOLEAN DEFAULT FALSE,
    payroll_run_id UUID REFERENCES payroll_runs(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_by UUID,
    updated_by UUID
);

CREATE INDEX idx_employee_allowances ON employee_allowances(employee_id, is_active);
CREATE INDEX idx_payroll_entries ON payroll_entries(employee_id, period_year, period_month);
