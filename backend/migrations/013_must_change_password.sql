ALTER TABLE users ADD COLUMN IF NOT EXISTS must_change_password BOOLEAN NOT NULL DEFAULT FALSE;

-- Flag existing employee-role users to change their default password on next login
UPDATE users SET must_change_password = TRUE WHERE role = 'employee' AND must_change_password = FALSE;
