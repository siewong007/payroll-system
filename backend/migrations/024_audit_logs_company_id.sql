-- Scope audit_logs by company so the listing query can filter reliably,
-- including rows where user_id IS NULL (e.g. public kiosk endpoints).
-- Nullable to preserve existing rows and to accommodate platform-level actions
-- that are not tied to a single company.
ALTER TABLE audit_logs
    ADD COLUMN company_id UUID REFERENCES companies(id);

CREATE INDEX idx_audit_logs_company_created ON audit_logs(company_id, created_at DESC);

-- Backfill existing rows from their user's company where possible, so
-- historical data remains visible after the listing query switches to
-- filtering by company_id.
UPDATE audit_logs al
SET company_id = u.company_id
FROM users u
WHERE al.user_id = u.id
  AND al.company_id IS NULL;
