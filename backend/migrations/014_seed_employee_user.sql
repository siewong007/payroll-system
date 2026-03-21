-- Create an employee user account for portal testing
-- Password: employee123 (bcrypt hash)
INSERT INTO users (id, email, password_hash, full_name, role, company_id, employee_id)
VALUES (
    '00000000-0000-0000-0000-000000000010',
    'sarah@demo.com',
    '$2b$12$eGHs6MK8r5JgA7KZPQWkR.Op6RKb3Zomh0CIqFDreBNDzaFZTOqk.',
    'Sarah Lee',
    'employee',
    '00000000-0000-0000-0000-000000000001',
    (SELECT id FROM employees WHERE employee_number = 'EMP002' AND company_id = '00000000-0000-0000-0000-000000000001' LIMIT 1)
)
ON CONFLICT (email) DO NOTHING;
