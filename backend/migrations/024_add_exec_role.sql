-- Add 'exec' role to the users table
ALTER TABLE users DROP CONSTRAINT IF EXISTS users_role_check;
ALTER TABLE users ADD CONSTRAINT users_role_check
    CHECK (role IN ('super_admin', 'payroll_admin', 'hr_manager', 'finance', 'exec', 'employee'));

-- Seed test exec user (password: admin123)
INSERT INTO users (id, email, password_hash, full_name, role, company_id)
VALUES (
    '00000000-0000-0000-0000-000000000020',
    'exec@demo.com',
    '$2b$12$glAZlYUHfNs/tV4IgNnMxOM3O/ZqB/BYJCDiomT3DMPCpE7oQQC8u',
    'Executive Director',
    'exec',
    '00000000-0000-0000-0000-000000000001'
) ON CONFLICT (email) DO NOTHING;
