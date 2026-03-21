-- Calendar Management: Holidays and Working Day Configuration

-- Working day configuration per company (which days of week are working days)
CREATE TABLE IF NOT EXISTS working_day_config (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    company_id UUID NOT NULL REFERENCES companies(id),
    day_of_week SMALLINT NOT NULL CHECK (day_of_week BETWEEN 0 AND 6), -- 0=Sunday, 6=Saturday
    is_working_day BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(company_id, day_of_week)
);

-- Holidays table
CREATE TABLE IF NOT EXISTS holidays (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    company_id UUID NOT NULL REFERENCES companies(id),
    name VARCHAR(255) NOT NULL,
    date DATE NOT NULL,
    holiday_type VARCHAR(50) NOT NULL DEFAULT 'public_holiday'
        CHECK (holiday_type IN ('public_holiday', 'company_holiday', 'replacement_leave', 'state_holiday')),
    description TEXT,
    is_recurring BOOLEAN NOT NULL DEFAULT FALSE, -- repeats every year (month+day)
    state VARCHAR(100), -- for state-specific holidays (e.g. Selangor, KL)
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_by UUID REFERENCES users(id),
    updated_by UUID REFERENCES users(id)
);

CREATE INDEX idx_holidays_company_date ON holidays(company_id, date);
CREATE INDEX idx_holidays_company_year ON holidays(company_id, (EXTRACT(YEAR FROM date)));

-- Seed default working day config (Mon-Fri working, Sat-Sun off) for existing companies
INSERT INTO working_day_config (company_id, day_of_week, is_working_day)
SELECT c.id, d.day, d.is_working
FROM companies c
CROSS JOIN (VALUES (0, FALSE), (1, TRUE), (2, TRUE), (3, TRUE), (4, TRUE), (5, TRUE), (6, FALSE)) AS d(day, is_working)
ON CONFLICT DO NOTHING;

-- Seed Malaysian 2026 public holidays for existing companies
INSERT INTO holidays (company_id, name, date, holiday_type, is_recurring, description)
SELECT c.id, h.name, h.date::DATE, 'public_holiday', h.recurring, h.description
FROM companies c
CROSS JOIN (VALUES
    ('New Year''s Day', '2026-01-01', TRUE, 'New Year'),
    ('Thaipusam', '2026-01-25', FALSE, 'Hindu festival'),
    ('Nuzul Al-Quran', '2026-02-17', FALSE, 'Revelation of the Quran'),
    ('Labour Day', '2026-05-01', TRUE, 'International Workers Day'),
    ('Vesak Day', '2026-05-12', FALSE, 'Buddhist festival'),
    ('Hari Raya Aidilfitri', '2026-03-20', FALSE, 'End of Ramadan'),
    ('Hari Raya Aidilfitri (2nd Day)', '2026-03-21', FALSE, 'End of Ramadan'),
    ('Yang di-Pertuan Agong Birthday', '2026-06-01', FALSE, 'King''s birthday'),
    ('Hari Raya Haji', '2026-05-27', FALSE, 'Feast of Sacrifice'),
    ('Hari Raya Haji (2nd Day)', '2026-05-28', FALSE, 'Feast of Sacrifice'),
    ('Awal Muharram', '2026-06-17', FALSE, 'Islamic New Year'),
    ('Malaysia Day', '2026-09-16', TRUE, 'Formation of Malaysia'),
    ('Mawlid Nabi', '2026-08-26', FALSE, 'Prophet Muhammad''s Birthday'),
    ('Deepavali', '2026-10-20', FALSE, 'Festival of Lights'),
    ('Christmas Day', '2026-12-25', TRUE, 'Christmas'),
    ('Merdeka Day', '2026-08-31', TRUE, 'Independence Day')
) AS h(name, date, recurring, description)
ON CONFLICT DO NOTHING;
