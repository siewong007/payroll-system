-- Give audit_logs' foreign keys an ON DELETE policy.
--
-- Both were created with the default NO ACTION while every sibling table
-- (refresh_tokens, user_companies, the passkey and oauth tables) cascades. That
-- was not a retention decision, it was an omission, and it surfaced as two live
-- failures:
--
--   * DELETE /api/admin/companies/{id} -> repositories::companies::delete_cascade
--     removes ~20 child tables but never audit_logs, so the final
--     `DELETE FROM companies` violated audit_logs_company_id_fkey for any tenant
--     that had ever been used.
--
--   * Hard-deleting a user (users::delete_by_employee) violated
--     audit_logs_user_id_fkey for any user who had ever done anything.
--
-- The two get *different* policies on purpose:
--
--   company_id -> CASCADE. Deleting a company is a full tenant teardown; its
--   employees, payroll runs and settings are already removed by delete_cascade.
--   Audit rows for a company that no longer exists are unreadable anyway — every
--   read path filters `WHERE al.company_id = $1`.
--
--   user_id -> SET NULL, *not* CASCADE. Cascading would erase a departed actor's
--   entire history, which is precisely the evidence an audit trail exists to
--   keep. The read model already LEFT JOINs users and projects user_email as
--   nullable, so a null actor renders correctly today.

ALTER TABLE audit_logs DROP CONSTRAINT audit_logs_company_id_fkey;
ALTER TABLE audit_logs
    ADD CONSTRAINT audit_logs_company_id_fkey
    FOREIGN KEY (company_id) REFERENCES companies (id) ON DELETE CASCADE;

ALTER TABLE audit_logs DROP CONSTRAINT audit_logs_user_id_fkey;
ALTER TABLE audit_logs
    ADD CONSTRAINT audit_logs_user_id_fkey
    FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE SET NULL;

-- Notifications are not evidence: they are transient UI state, and a user's
-- notifications are meaningless once the user is gone. This FK blocked user
-- deletion for the same reason and wants the opposite policy.
ALTER TABLE notifications DROP CONSTRAINT notifications_user_id_fkey;
ALTER TABLE notifications
    ADD CONSTRAINT notifications_user_id_fkey
    FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE;
