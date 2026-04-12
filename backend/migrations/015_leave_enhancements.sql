-- Leave carry-forward policy columns
ALTER TABLE leave_types
  ADD COLUMN max_carry_forward NUMERIC(5,1) NOT NULL DEFAULT 0,
  ADD COLUMN carry_forward_expiry_months INTEGER NOT NULL DEFAULT 3,
  ADD COLUMN is_system BOOLEAN NOT NULL DEFAULT FALSE;

-- Extra audit_logs indexes for dashboard queries
CREATE INDEX IF NOT EXISTS idx_audit_logs_action ON audit_logs(action);
CREATE INDEX IF NOT EXISTS idx_audit_logs_date_range ON audit_logs(created_at DESC, entity_type);
