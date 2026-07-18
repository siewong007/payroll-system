-- Whole-company backups intentionally exclude authentication data, so older
-- imports can have employees without matching login accounts. Backfill active,
-- non-deleted employees that have an email address and link every matching
-- account to its company.
--
-- New accounts receive a bcrypt hash of a discarded random secret. Nobody can
-- derive or use that temporary credential; the existing Forgot Password flow is
-- the only way to activate access. `must_change_password` remains an additional
-- guard if an administrator later assigns a temporary password.

WITH candidates AS (
    SELECT DISTINCT ON (BTRIM(e.email))
        BTRIM(e.email) AS email,
        e.full_name,
        e.company_id,
        e.id AS employee_id
    FROM employees e
    WHERE e.deleted_at IS NULL
      AND COALESCE(e.is_active, TRUE)
      AND e.email IS NOT NULL
      AND BTRIM(e.email) <> ''
    ORDER BY BTRIM(e.email), e.updated_at DESC, e.id DESC
)
INSERT INTO users (
    email,
    password_hash,
    full_name,
    roles,
    company_id,
    employee_id,
    must_change_password
)
SELECT
    c.email,
    '$2b$12$4KlNiC0qvbl15bM6tKnolOTcd0lWLSMNSDI2IUG9qZxA0MJQWNhze',
    c.full_name,
    ARRAY['employee']::VARCHAR(50)[],
    c.company_id,
    c.employee_id,
    TRUE
FROM candidates c
ON CONFLICT (email) DO UPDATE
SET employee_id = EXCLUDED.employee_id,
    company_id = EXCLUDED.company_id,
    updated_at = NOW();

-- Employee-only accounts may have a stale membership after a company overwrite.
WITH candidates AS (
    SELECT DISTINCT ON (BTRIM(e.email))
        BTRIM(e.email) AS email,
        e.company_id
    FROM employees e
    WHERE e.deleted_at IS NULL
      AND COALESCE(e.is_active, TRUE)
      AND e.email IS NOT NULL
      AND BTRIM(e.email) <> ''
    ORDER BY BTRIM(e.email), e.updated_at DESC, e.id DESC
)
DELETE FROM user_companies uc
USING users u, candidates c
WHERE uc.user_id = u.id
  AND u.email = c.email
  AND u.roles = ARRAY['employee']::VARCHAR(50)[]
  AND uc.company_id <> c.company_id;

WITH candidates AS (
    SELECT DISTINCT ON (BTRIM(e.email))
        BTRIM(e.email) AS email,
        e.company_id
    FROM employees e
    WHERE e.deleted_at IS NULL
      AND COALESCE(e.is_active, TRUE)
      AND e.email IS NOT NULL
      AND BTRIM(e.email) <> ''
    ORDER BY BTRIM(e.email), e.updated_at DESC, e.id DESC
)
INSERT INTO user_companies (user_id, company_id)
SELECT u.id, c.company_id
FROM candidates c
JOIN users u ON u.email = c.email
ON CONFLICT DO NOTHING;
