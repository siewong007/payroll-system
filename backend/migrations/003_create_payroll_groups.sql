CREATE TABLE payroll_groups (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    company_id UUID NOT NULL REFERENCES companies(id),
    name VARCHAR(100) NOT NULL,
    description VARCHAR(255),
    cutoff_day INTEGER NOT NULL DEFAULT 25, -- day of month
    payment_day INTEGER NOT NULL DEFAULT 28,
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_by UUID,
    updated_by UUID
);

ALTER TABLE employees
    ADD CONSTRAINT fk_employees_payroll_group
    FOREIGN KEY (payroll_group_id) REFERENCES payroll_groups(id);
