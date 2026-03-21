-- Leave Types
CREATE TABLE leave_types (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    company_id UUID NOT NULL REFERENCES companies(id),
    name VARCHAR(100) NOT NULL,
    description TEXT,
    default_days NUMERIC(5,1) NOT NULL DEFAULT 0,
    is_paid BOOLEAN NOT NULL DEFAULT TRUE,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(company_id, name)
);

-- Leave Balances (per employee per year)
CREATE TABLE leave_balances (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    employee_id UUID NOT NULL REFERENCES employees(id),
    leave_type_id UUID NOT NULL REFERENCES leave_types(id),
    year INTEGER NOT NULL,
    entitled_days NUMERIC(5,1) NOT NULL DEFAULT 0,
    taken_days NUMERIC(5,1) NOT NULL DEFAULT 0,
    pending_days NUMERIC(5,1) NOT NULL DEFAULT 0,
    carried_forward NUMERIC(5,1) NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(employee_id, leave_type_id, year)
);

-- Leave Requests
CREATE TABLE leave_requests (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    employee_id UUID NOT NULL REFERENCES employees(id),
    company_id UUID NOT NULL REFERENCES companies(id),
    leave_type_id UUID NOT NULL REFERENCES leave_types(id),
    start_date DATE NOT NULL,
    end_date DATE NOT NULL,
    days NUMERIC(5,1) NOT NULL,
    reason TEXT,
    status VARCHAR(20) NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'approved', 'rejected', 'cancelled')),
    reviewed_by UUID REFERENCES users(id),
    reviewed_at TIMESTAMPTZ,
    review_notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_leave_requests_employee ON leave_requests(employee_id);
CREATE INDEX idx_leave_requests_status ON leave_requests(company_id, status);

-- Expense Claims
CREATE TABLE claims (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    employee_id UUID NOT NULL REFERENCES employees(id),
    company_id UUID NOT NULL REFERENCES companies(id),
    title VARCHAR(255) NOT NULL,
    description TEXT,
    amount BIGINT NOT NULL, -- in sen
    category VARCHAR(100),
    receipt_url VARCHAR(500),
    receipt_file_name VARCHAR(255),
    expense_date DATE NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'draft'
        CHECK (status IN ('draft', 'pending', 'approved', 'rejected', 'processed')),
    submitted_at TIMESTAMPTZ,
    reviewed_by UUID REFERENCES users(id),
    reviewed_at TIMESTAMPTZ,
    review_notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_claims_employee ON claims(employee_id);
CREATE INDEX idx_claims_status ON claims(company_id, status);

-- Seed default leave types for demo company
INSERT INTO leave_types (company_id, name, description, default_days) VALUES
('00000000-0000-0000-0000-000000000001', 'Annual Leave', 'Paid annual leave entitlement', 14),
('00000000-0000-0000-0000-000000000001', 'Sick Leave', 'Paid sick leave (outpatient)', 14),
('00000000-0000-0000-0000-000000000001', 'Hospitalisation Leave', 'Paid hospitalisation leave', 60),
('00000000-0000-0000-0000-000000000001', 'Compassionate Leave', 'Bereavement / compassionate leave', 3),
('00000000-0000-0000-0000-000000000001', 'Maternity Leave', 'Paid maternity leave', 98),
('00000000-0000-0000-0000-000000000001', 'Paternity Leave', 'Paid paternity leave', 7),
('00000000-0000-0000-0000-000000000001', 'Marriage Leave', 'Leave for own marriage', 3),
('00000000-0000-0000-0000-000000000001', 'Unpaid Leave', 'Unpaid leave', 365);

-- Seed leave balances for existing employees (current year)
INSERT INTO leave_balances (employee_id, leave_type_id, year, entitled_days)
SELECT e.id, lt.id, 2026, lt.default_days
FROM employees e
CROSS JOIN leave_types lt
WHERE e.company_id = '00000000-0000-0000-0000-000000000001'
AND lt.company_id = '00000000-0000-0000-0000-000000000001'
AND e.is_active = TRUE;
