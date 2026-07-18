-- Preserve a deletion tombstone so a later employee import or backup restore
-- cannot silently recreate access for an account removed by a super admin.
ALTER TABLE users
    ADD COLUMN deleted_at TIMESTAMPTZ,
    ADD COLUMN deleted_by UUID;

CREATE INDEX idx_users_active_not_deleted
    ON users (id)
    WHERE is_active = TRUE AND deleted_at IS NULL;

-- Older accounts used `company_id` as their active company without necessarily
-- having a matching membership row. Backfill the link used by company-scoped
-- user visibility and access checks.
INSERT INTO user_companies (user_id, company_id)
SELECT id, company_id
FROM users
WHERE company_id IS NOT NULL
  AND deleted_at IS NULL
ON CONFLICT DO NOTHING;
