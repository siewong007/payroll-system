-- Add 'admin' role (same as exec but supports multiple companies)
ALTER TABLE users DROP CONSTRAINT IF EXISTS users_role_check;
ALTER TABLE users ADD CONSTRAINT users_role_check
  CHECK (role IN ('super_admin', 'admin', 'payroll_admin', 'hr_manager', 'finance', 'exec', 'employee'));
