-- Work schedules per company + attendance timezone/pagination fixes

-- ─── Company Work Schedules ───
-- Defines expected start/end times per company so the system can auto-detect late status.
CREATE TABLE company_work_schedules (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    company_id      UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    name            VARCHAR(100) NOT NULL DEFAULT 'Default',
    start_time      TIME NOT NULL DEFAULT '09:00',
    end_time        TIME NOT NULL DEFAULT '18:00',
    grace_minutes   INTEGER NOT NULL DEFAULT 15,
    half_day_hours  NUMERIC(4,2) NOT NULL DEFAULT 4.0,
    timezone        VARCHAR(50) NOT NULL DEFAULT 'Asia/Kuala_Lumpur',
    is_default      BOOLEAN NOT NULL DEFAULT TRUE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_work_schedules_company ON company_work_schedules(company_id);

-- Ensure only one default schedule per company
CREATE UNIQUE INDEX idx_work_schedules_default
    ON company_work_schedules(company_id) WHERE is_default = TRUE;

-- Seed a default schedule for every existing company
INSERT INTO company_work_schedules (company_id, name, start_time, end_time, grace_minutes, timezone)
SELECT id, 'Default', '09:00', '18:00', 15, 'Asia/Kuala_Lumpur'
FROM companies
ON CONFLICT DO NOTHING;

-- ─── Add timezone column to companies ───
ALTER TABLE companies
    ADD COLUMN IF NOT EXISTS timezone VARCHAR(50) NOT NULL DEFAULT 'Asia/Kuala_Lumpur';
