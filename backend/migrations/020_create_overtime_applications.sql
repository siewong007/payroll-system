-- Overtime applications submitted by employees
CREATE TABLE overtime_applications (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    employee_id UUID NOT NULL REFERENCES employees(id),
    company_id UUID NOT NULL REFERENCES companies(id),
    ot_date DATE NOT NULL,
    start_time TIME NOT NULL,
    end_time TIME NOT NULL,
    hours NUMERIC(5,2) NOT NULL,
    ot_type VARCHAR(20) NOT NULL DEFAULT 'normal'
        CHECK (ot_type IN ('normal', 'rest_day', 'public_holiday')),
    reason VARCHAR(500),
    status VARCHAR(20) NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'approved', 'rejected', 'cancelled')),
    reviewed_by UUID REFERENCES users(id),
    reviewed_at TIMESTAMPTZ,
    review_notes VARCHAR(500),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_overtime_applications_employee ON overtime_applications(employee_id, status);
CREATE INDEX idx_overtime_applications_company ON overtime_applications(company_id, status);
