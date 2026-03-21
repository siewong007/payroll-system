-- Add shift type classification to teams
ALTER TABLE teams
    ADD COLUMN shift_type VARCHAR(20) NOT NULL DEFAULT 'day'
    CHECK (shift_type IN ('day', 'night'));
