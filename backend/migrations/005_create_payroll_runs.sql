CREATE TYPE payroll_status AS ENUM (
    'draft', 'processing', 'processed', 'approved', 'paid', 'cancelled'
);

CREATE TABLE payroll_runs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    company_id UUID NOT NULL REFERENCES companies(id),
    payroll_group_id UUID NOT NULL REFERENCES payroll_groups(id),
    period_year INTEGER NOT NULL,
    period_month INTEGER NOT NULL,
    period_start DATE NOT NULL,
    period_end DATE NOT NULL,
    pay_date DATE NOT NULL,
    status payroll_status NOT NULL DEFAULT 'draft',

    -- Totals (in sen)
    total_gross BIGINT NOT NULL DEFAULT 0,
    total_net BIGINT NOT NULL DEFAULT 0,
    total_employer_cost BIGINT NOT NULL DEFAULT 0,
    total_epf_employee BIGINT NOT NULL DEFAULT 0,
    total_epf_employer BIGINT NOT NULL DEFAULT 0,
    total_socso_employee BIGINT NOT NULL DEFAULT 0,
    total_socso_employer BIGINT NOT NULL DEFAULT 0,
    total_eis_employee BIGINT NOT NULL DEFAULT 0,
    total_eis_employer BIGINT NOT NULL DEFAULT 0,
    total_pcb BIGINT NOT NULL DEFAULT 0,
    total_zakat BIGINT NOT NULL DEFAULT 0,
    employee_count INTEGER NOT NULL DEFAULT 0,

    -- Version tracking
    version INTEGER NOT NULL DEFAULT 1,

    processed_by UUID REFERENCES users(id),
    processed_at TIMESTAMPTZ,
    approved_by UUID REFERENCES users(id),
    approved_at TIMESTAMPTZ,
    locked_at TIMESTAMPTZ,
    locked_by UUID REFERENCES users(id),

    notes TEXT,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_by UUID,
    updated_by UUID,

    UNIQUE(company_id, payroll_group_id, period_year, period_month, version)
);

-- Individual employee payroll items
CREATE TABLE payroll_items (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    payroll_run_id UUID NOT NULL REFERENCES payroll_runs(id),
    employee_id UUID NOT NULL REFERENCES employees(id),

    -- Earnings (in sen)
    basic_salary BIGINT NOT NULL DEFAULT 0,
    gross_salary BIGINT NOT NULL DEFAULT 0,
    total_allowances BIGINT NOT NULL DEFAULT 0,
    total_overtime BIGINT NOT NULL DEFAULT 0,
    total_bonus BIGINT NOT NULL DEFAULT 0,
    total_commission BIGINT NOT NULL DEFAULT 0,
    total_claims BIGINT NOT NULL DEFAULT 0,

    -- Statutory deductions (in sen)
    epf_employee BIGINT NOT NULL DEFAULT 0,
    epf_employer BIGINT NOT NULL DEFAULT 0,
    socso_employee BIGINT NOT NULL DEFAULT 0,
    socso_employer BIGINT NOT NULL DEFAULT 0,
    eis_employee BIGINT NOT NULL DEFAULT 0,
    eis_employer BIGINT NOT NULL DEFAULT 0,
    pcb_amount BIGINT NOT NULL DEFAULT 0,
    zakat_amount BIGINT NOT NULL DEFAULT 0,

    -- Other deductions (in sen)
    ptptn_amount BIGINT NOT NULL DEFAULT 0,
    tabung_haji_amount BIGINT NOT NULL DEFAULT 0,
    total_loan_deductions BIGINT NOT NULL DEFAULT 0,
    total_other_deductions BIGINT NOT NULL DEFAULT 0,
    unpaid_leave_deduction BIGINT NOT NULL DEFAULT 0,
    unpaid_leave_days NUMERIC(5,2) NOT NULL DEFAULT 0,

    -- Totals (in sen)
    total_deductions BIGINT NOT NULL DEFAULT 0,
    net_salary BIGINT NOT NULL DEFAULT 0,
    employer_cost BIGINT NOT NULL DEFAULT 0, -- gross + employer statutory

    -- YTD figures (in sen)
    ytd_gross BIGINT NOT NULL DEFAULT 0,
    ytd_epf_employee BIGINT NOT NULL DEFAULT 0,
    ytd_pcb BIGINT NOT NULL DEFAULT 0,
    ytd_socso_employee BIGINT NOT NULL DEFAULT 0,
    ytd_eis_employee BIGINT NOT NULL DEFAULT 0,
    ytd_zakat BIGINT NOT NULL DEFAULT 0,
    ytd_net BIGINT NOT NULL DEFAULT 0,

    -- Pro-ration
    working_days INTEGER,
    days_worked NUMERIC(5,2),
    is_prorated BOOLEAN DEFAULT FALSE,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(payroll_run_id, employee_id)
);

-- Payroll item details (individual earnings/deductions line items)
CREATE TABLE payroll_item_details (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    payroll_item_id UUID NOT NULL REFERENCES payroll_items(id),
    category VARCHAR(20) NOT NULL CHECK (category IN ('earning', 'deduction')),
    item_type VARCHAR(50) NOT NULL, -- e.g. 'basic_salary', 'housing_allowance', 'epf_employee'
    description VARCHAR(255) NOT NULL,
    amount BIGINT NOT NULL, -- in sen
    is_taxable BOOLEAN DEFAULT TRUE,
    is_statutory BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_payroll_runs_company ON payroll_runs(company_id);
CREATE INDEX idx_payroll_runs_period ON payroll_runs(period_year, period_month);
CREATE INDEX idx_payroll_items_run ON payroll_items(payroll_run_id);
CREATE INDEX idx_payroll_items_employee ON payroll_items(employee_id);
CREATE INDEX idx_payroll_item_details ON payroll_item_details(payroll_item_id);
