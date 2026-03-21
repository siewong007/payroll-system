-- Notifications for employees and admins
CREATE TABLE notifications (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id),
    company_id UUID NOT NULL REFERENCES companies(id),
    notification_type VARCHAR(50) NOT NULL,
    -- leave_submitted, leave_approved, leave_rejected,
    -- claim_submitted, claim_approved, claim_rejected,
    -- payroll_processed, payroll_approved, general
    title VARCHAR(255) NOT NULL,
    message TEXT NOT NULL,
    entity_type VARCHAR(50), -- leave_request, claim, payroll_run
    entity_id UUID,
    is_read BOOLEAN NOT NULL DEFAULT FALSE,
    read_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_notifications_user ON notifications(user_id, is_read);
CREATE INDEX idx_notifications_company ON notifications(company_id);
CREATE INDEX idx_notifications_created ON notifications(created_at DESC);

-- Seed leave balances for Sarah (EMP002) so leave feature works
INSERT INTO leave_balances (employee_id, leave_type_id, year, entitled_days, taken_days, pending_days, carried_forward)
SELECT
    'a0000000-0000-0000-0000-000000000002',
    lt.id,
    2026,
    lt.default_days,
    0,
    0,
    0
FROM leave_types lt
WHERE lt.company_id = '00000000-0000-0000-0000-000000000001' AND lt.is_active = TRUE
ON CONFLICT (employee_id, leave_type_id, year) DO NOTHING;
