-- Phase 2: Geofencing, overtime tracking, attendance corrections

-- ─── Company Office Locations (for geofencing) ───
CREATE TABLE company_locations (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    company_id      UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    name            VARCHAR(150) NOT NULL,
    latitude        DOUBLE PRECISION NOT NULL,
    longitude       DOUBLE PRECISION NOT NULL,
    radius_meters   INTEGER NOT NULL DEFAULT 200,
    is_active       BOOLEAN NOT NULL DEFAULT TRUE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_company_locations_company ON company_locations(company_id);

-- ─── Geofence enforcement mode per company ───
-- 'none' = no check, 'warn' = allow but flag, 'enforce' = reject if outside
ALTER TABLE companies
    ADD COLUMN IF NOT EXISTS geofence_mode VARCHAR(10) NOT NULL DEFAULT 'none'
        CHECK (geofence_mode IN ('none', 'warn', 'enforce'));

-- ─── Overtime & hours tracking on attendance records ───
ALTER TABLE attendance_records
    ADD COLUMN IF NOT EXISTS hours_worked NUMERIC(5,2),
    ADD COLUMN IF NOT EXISTS overtime_hours NUMERIC(5,2) DEFAULT 0,
    ADD COLUMN IF NOT EXISTS is_outside_geofence BOOLEAN DEFAULT FALSE;
