-- Per-employee work schedules
-- Allows overriding company-wide default schedules for specific employees.

CREATE TABLE employee_work_schedules (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    employee_id     UUID NOT NULL REFERENCES employees(id) ON DELETE CASCADE,
    company_id      UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    day_of_week     SMALLINT NOT NULL CHECK (day_of_week BETWEEN 0 AND 6),
    start_time      TIME NOT NULL DEFAULT '09:00',
    end_time        TIME NOT NULL DEFAULT '18:00',
    grace_minutes   INTEGER NOT NULL DEFAULT 15,
    is_active       BOOLEAN NOT NULL DEFAULT TRUE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(employee_id, day_of_week)
);

CREATE INDEX idx_employee_work_schedules_employee ON employee_work_schedules(employee_id);
