-- Replace shift_type with a flexible tag column on teams
ALTER TABLE teams DROP COLUMN IF EXISTS shift_type;
ALTER TABLE teams ADD COLUMN tag VARCHAR(50) NOT NULL DEFAULT 'general';
