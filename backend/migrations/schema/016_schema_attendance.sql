-- Attendance System: platform settings, QR tokens, attendance records

-- Platform-level settings (super_admin controlled, not per-company)
CREATE TABLE platform_settings (
    key VARCHAR(100) PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_by UUID REFERENCES users(id)
);

-- Seed default attendance method
INSERT INTO platform_settings (key, value) VALUES
    ('attendance_method', 'qr_code'),
    ('allow_company_override', 'false');

-- Per-company attendance method override (NULL = use platform default)
ALTER TABLE companies
    ADD COLUMN IF NOT EXISTS attendance_method VARCHAR(20) DEFAULT NULL
        CHECK (attendance_method IN ('qr_code', 'face_id'));

-- Short-lived QR tokens (one per company, refreshed every 60s)
CREATE TABLE attendance_qr_tokens (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    company_id  UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    token       VARCHAR(128) NOT NULL UNIQUE,
    expires_at  TIMESTAMPTZ NOT NULL,
    used        BOOLEAN NOT NULL DEFAULT FALSE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_qr_tokens_company_expires ON attendance_qr_tokens(company_id, expires_at);
CREATE INDEX idx_qr_tokens_token ON attendance_qr_tokens(token);

-- Attendance records
CREATE TABLE attendance_records (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    company_id      UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    employee_id     UUID NOT NULL REFERENCES employees(id) ON DELETE CASCADE,
    check_in_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    check_out_at    TIMESTAMPTZ,
    method          VARCHAR(20) NOT NULL CHECK (method IN ('qr_code', 'face_id', 'manual')),
    status          VARCHAR(20) NOT NULL DEFAULT 'present'
                        CHECK (status IN ('present', 'late', 'absent', 'half_day')),
    -- Geolocation at check-in
    latitude        DOUBLE PRECISION,
    longitude       DOUBLE PRECISION,
    -- Geolocation at check-out
    checkout_latitude   DOUBLE PRECISION,
    checkout_longitude  DOUBLE PRECISION,
    notes           TEXT,
    qr_token_id     UUID REFERENCES attendance_qr_tokens(id),
    created_by      UUID REFERENCES users(id),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_attendance_employee ON attendance_records(employee_id);
CREATE INDEX idx_attendance_company_date ON attendance_records(company_id, check_in_at DESC);
CREATE INDEX idx_attendance_status ON attendance_records(company_id, status);
