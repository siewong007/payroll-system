-- Seed additional employees (EMP011-EMP020), user accounts, teams, and leave requests

INSERT INTO employees (
    id, company_id, employee_number, full_name, ic_number, date_of_birth, gender,
    nationality, race, residency_status, marital_status,
    email, phone, address_line1, city, state, postcode,
    department, designation, employment_type, date_joined,
    basic_salary, bank_name, bank_account_number,
    tax_identification_number, epf_number, socso_number, eis_number,
    working_spouse, num_children, epf_category, is_muslim,
    payroll_group_id, is_active
) VALUES
('a0000000-0000-0000-0000-000000000011', '00000000-0000-0000-0000-000000000001',
 'EMP011', 'Lim Chee Keong', '910725-08-3344', '1991-07-25', 'male',
 'Malaysian', 'chinese', 'citizen', 'married',
 'chee.keong@demo.com', '012-4445566', '18 Jalan PJS 11/28', 'Petaling Jaya', 'Selangor', '46150',
 'Engineering', 'DevOps Engineer', 'permanent', '2021-05-01',
 620000, 'Maybank', '1645567890',
 'SG10091072533344', 'EPF-100011', 'SOCSO-100011', 'EIS-100011',
 TRUE, 1, 'A', FALSE, '00000000-0000-0000-0000-000000000003', TRUE),
('a0000000-0000-0000-0000-000000000012', '00000000-0000-0000-0000-000000000001',
 'EMP012', 'Kavitha a/p Subramaniam', '930912-07-5567', '1993-09-12', 'female',
 'Malaysian', 'indian', 'citizen', 'single',
 'kavitha.s@demo.com', '017-6667788', '55 Jalan Tun Sambanthan', 'Kuala Lumpur', 'Wilayah Persekutuan', '50470',
 'Engineering', 'UX Designer', 'permanent', '2022-02-14',
 520000, 'CIMB', '7601234567',
 'SG10093091255567', 'EPF-100012', 'SOCSO-100012', 'EIS-100012',
 FALSE, 0, 'A', FALSE, '00000000-0000-0000-0000-000000000003', TRUE),
('a0000000-0000-0000-0000-000000000013', '00000000-0000-0000-0000-000000000001',
 'EMP013', 'Azman bin Yusof', '880305-01-7789', '1988-03-05', 'male',
 'Malaysian', 'malay', 'citizen', 'married',
 'azman.yusof@demo.com', '013-7778899', '8 Taman Melawati', 'Kuala Lumpur', 'Wilayah Persekutuan', '53100',
 'Sales', 'Sales Executive', 'permanent', '2020-09-01',
 480000, 'RHB Bank', '2141234567',
 'SG10088030577789', 'EPF-100013', 'SOCSO-100013', 'EIS-100013',
 TRUE, 3, 'A', TRUE, '00000000-0000-0000-0000-000000000003', TRUE),
('a0000000-0000-0000-0000-000000000014', '00000000-0000-0000-0000-000000000001',
 'EMP014', 'Ng Siew Mei', '950614-10-8899', '1995-06-14', 'female',
 'Malaysian', 'chinese', 'citizen', 'single',
 'siew.mei@demo.com', '016-8889900', '33 Jalan Bukit Kiara', 'Kuala Lumpur', 'Wilayah Persekutuan', '60000',
 'Sales', 'Sales Executive', 'permanent', '2023-01-09',
 440000, 'Public Bank', '3189876543',
 'SG10095061488899', 'EPF-100014', 'SOCSO-100014', 'EIS-100014',
 FALSE, 0, 'A', FALSE, '00000000-0000-0000-0000-000000000003', TRUE),
('a0000000-0000-0000-0000-000000000015', '00000000-0000-0000-0000-000000000001',
 'EMP015', 'Hafiz bin Rahman', '890120-14-2233', '1989-01-20', 'male',
 'Malaysian', 'malay', 'citizen', 'married',
 'hafiz.rahman@demo.com', '019-1112233', '12 Damansara Heights', 'Kuala Lumpur', 'Wilayah Persekutuan', '50490',
 'Engineering', 'Senior Developer', 'permanent', '2019-11-01',
 700000, 'Hong Leong Bank', '0449876543',
 'SG10089012022233', 'EPF-100015', 'SOCSO-100015', 'EIS-100015',
 TRUE, 2, 'A', TRUE, '00000000-0000-0000-0000-000000000003', TRUE),
('a0000000-0000-0000-0000-000000000016', '00000000-0000-0000-0000-000000000001',
 'EMP016', 'Chen Yee Ling', '960830-08-4455', '1996-08-30', 'female',
 'Malaysian', 'chinese', 'citizen', 'single',
 'yee.ling@demo.com', '011-22334455', '7 Jalan Damai', 'Kuala Lumpur', 'Wilayah Persekutuan', '55000',
 'Marketing', 'Marketing Executive', 'permanent', '2024-03-01',
 400000, 'Bank Islam', '1201234567',
 'SG10096083044455', 'EPF-100016', 'SOCSO-100016', 'EIS-100016',
 FALSE, 0, 'A', FALSE, '00000000-0000-0000-0000-000000000003', TRUE),
('a0000000-0000-0000-0000-000000000017', '00000000-0000-0000-0000-000000000001',
 'EMP017', 'Ravi a/l Krishnan', '870215-07-6677', '1987-02-15', 'male',
 'Malaysian', 'indian', 'citizen', 'married',
 'ravi.krishnan@demo.com', '012-3334455', '20 Taman OUG', 'Kuala Lumpur', 'Wilayah Persekutuan', '58200',
 'Operations', 'Support Lead', 'permanent', '2020-06-15',
 530000, 'Ambank', '8801239876',
 'SG10087021566677', 'EPF-100017', 'SOCSO-100017', 'EIS-100017',
 TRUE, 2, 'A', FALSE, '00000000-0000-0000-0000-000000000003', TRUE),
('a0000000-0000-0000-0000-000000000018', '00000000-0000-0000-0000-000000000001',
 'EMP018', 'Zainab binti Osman', '980410-02-8811', '1998-04-10', 'female',
 'Malaysian', 'malay', 'citizen', 'single',
 'zainab.osman@demo.com', '014-4445566', '15 Jalan Ipoh', 'Kuala Lumpur', 'Wilayah Persekutuan', '51200',
 'Operations', 'Support Agent', 'permanent', '2024-08-01',
 340000, 'Maybank', '1646543210',
 'SG10098041088811', 'EPF-100018', 'SOCSO-100018', 'EIS-100018',
 FALSE, 0, 'A', TRUE, '00000000-0000-0000-0000-000000000003', TRUE),
('a0000000-0000-0000-0000-000000000019', '00000000-0000-0000-0000-000000000001',
 'EMP019', 'Daniel Tan Wei Jie', '940718-10-9900', '1994-07-18', 'male',
 'Malaysian', 'chinese', 'citizen', 'single',
 'daniel.tan@demo.com', '016-5556677', '28 Jalan Kelang Lama', 'Kuala Lumpur', 'Wilayah Persekutuan', '58000',
 'Finance', 'Data Analyst', 'permanent', '2023-06-01',
 480000, 'CIMB', '7602345678',
 'SG10094071899900', 'EPF-100019', 'SOCSO-100019', 'EIS-100019',
 FALSE, 0, 'A', FALSE, '00000000-0000-0000-0000-000000000003', TRUE),
('a0000000-0000-0000-0000-000000000020', '00000000-0000-0000-0000-000000000001',
 'EMP020', 'Farah binti Aziz', '990905-14-1122', '1999-09-05', 'female',
 'Malaysian', 'malay', 'citizen', 'single',
 'farah.aziz@demo.com', '018-6667788', '3 Jalan Tun Razak', 'Kuala Lumpur', 'Wilayah Persekutuan', '50400',
 'Human Resources', 'HR Assistant', 'permanent', '2025-01-15',
 320000, 'Public Bank', '3181234567',
 'SG10099090511122', 'EPF-100020', 'SOCSO-100020', 'EIS-100020',
 FALSE, 0, 'A', TRUE, '00000000-0000-0000-0000-000000000003', TRUE)
ON CONFLICT (id) DO NOTHING;

-- Leave balances for new employees
INSERT INTO leave_balances (employee_id, leave_type_id, year, entitled_days)
SELECT e.id, lt.id, 2026, lt.default_days
FROM employees e
CROSS JOIN leave_types lt
WHERE e.company_id = '00000000-0000-0000-0000-000000000001'
AND lt.company_id = '00000000-0000-0000-0000-000000000001'
AND e.is_active = TRUE
AND e.id IN (
    'a0000000-0000-0000-0000-000000000011', 'a0000000-0000-0000-0000-000000000012',
    'a0000000-0000-0000-0000-000000000013', 'a0000000-0000-0000-0000-000000000014',
    'a0000000-0000-0000-0000-000000000015', 'a0000000-0000-0000-0000-000000000016',
    'a0000000-0000-0000-0000-000000000017', 'a0000000-0000-0000-0000-000000000018',
    'a0000000-0000-0000-0000-000000000019', 'a0000000-0000-0000-0000-000000000020'
)
ON CONFLICT (employee_id, leave_type_id, year) DO NOTHING;

-- User accounts for new employees (password: admin123)
INSERT INTO users (id, email, password_hash, full_name, role, company_id, employee_id) VALUES
('00000000-0000-0000-0000-000000000111', 'chee.keong@demo.com', '$2b$12$fRjUOo/hbE.JoyBkrDmmg.0nkGxVzL6dQXJuUHZ2dgDBZcdvX3yA2', 'Lim Chee Keong', 'employee', '00000000-0000-0000-0000-000000000001', 'a0000000-0000-0000-0000-000000000011'),
('00000000-0000-0000-0000-000000000112', 'kavitha.s@demo.com', '$2b$12$fRjUOo/hbE.JoyBkrDmmg.0nkGxVzL6dQXJuUHZ2dgDBZcdvX3yA2', 'Kavitha Subramaniam', 'employee', '00000000-0000-0000-0000-000000000001', 'a0000000-0000-0000-0000-000000000012'),
('00000000-0000-0000-0000-000000000113', 'azman.yusof@demo.com', '$2b$12$fRjUOo/hbE.JoyBkrDmmg.0nkGxVzL6dQXJuUHZ2dgDBZcdvX3yA2', 'Azman Yusof', 'employee', '00000000-0000-0000-0000-000000000001', 'a0000000-0000-0000-0000-000000000013'),
('00000000-0000-0000-0000-000000000114', 'siew.mei@demo.com', '$2b$12$fRjUOo/hbE.JoyBkrDmmg.0nkGxVzL6dQXJuUHZ2dgDBZcdvX3yA2', 'Ng Siew Mei', 'employee', '00000000-0000-0000-0000-000000000001', 'a0000000-0000-0000-0000-000000000014'),
('00000000-0000-0000-0000-000000000115', 'hafiz.rahman@demo.com', '$2b$12$fRjUOo/hbE.JoyBkrDmmg.0nkGxVzL6dQXJuUHZ2dgDBZcdvX3yA2', 'Hafiz Rahman', 'employee', '00000000-0000-0000-0000-000000000001', 'a0000000-0000-0000-0000-000000000015'),
('00000000-0000-0000-0000-000000000116', 'yee.ling@demo.com', '$2b$12$fRjUOo/hbE.JoyBkrDmmg.0nkGxVzL6dQXJuUHZ2dgDBZcdvX3yA2', 'Chen Yee Ling', 'employee', '00000000-0000-0000-0000-000000000001', 'a0000000-0000-0000-0000-000000000016'),
('00000000-0000-0000-0000-000000000117', 'ravi.krishnan@demo.com', '$2b$12$fRjUOo/hbE.JoyBkrDmmg.0nkGxVzL6dQXJuUHZ2dgDBZcdvX3yA2', 'Ravi Krishnan', 'employee', '00000000-0000-0000-0000-000000000001', 'a0000000-0000-0000-0000-000000000017'),
('00000000-0000-0000-0000-000000000118', 'zainab.osman@demo.com', '$2b$12$fRjUOo/hbE.JoyBkrDmmg.0nkGxVzL6dQXJuUHZ2dgDBZcdvX3yA2', 'Zainab Osman', 'employee', '00000000-0000-0000-0000-000000000001', 'a0000000-0000-0000-0000-000000000018'),
('00000000-0000-0000-0000-000000000119', 'daniel.tan@demo.com', '$2b$12$fRjUOo/hbE.JoyBkrDmmg.0nkGxVzL6dQXJuUHZ2dgDBZcdvX3yA2', 'Daniel Tan', 'employee', '00000000-0000-0000-0000-000000000001', 'a0000000-0000-0000-0000-000000000019'),
('00000000-0000-0000-0000-000000000120', 'farah.aziz@demo.com', '$2b$12$fRjUOo/hbE.JoyBkrDmmg.0nkGxVzL6dQXJuUHZ2dgDBZcdvX3yA2', 'Farah Aziz', 'employee', '00000000-0000-0000-0000-000000000001', 'a0000000-0000-0000-0000-000000000020')
ON CONFLICT (email) DO NOTHING;

-- Update user_companies for new users
INSERT INTO user_companies (user_id, company_id)
SELECT id, company_id FROM users WHERE company_id IS NOT NULL
ON CONFLICT DO NOTHING;

-- Teams
INSERT INTO teams (id, company_id, name, description, tag, created_at) VALUES
('b0000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000001', 'Backend Squad', 'Backend API and infrastructure', 'engineering', NOW()),
('b0000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000001', 'Frontend Squad', 'UI/UX and frontend development', 'engineering', NOW()),
('b0000000-0000-0000-0000-000000000003', '00000000-0000-0000-0000-000000000001', 'QA Team', 'Quality assurance and testing', 'engineering', NOW()),
('b0000000-0000-0000-0000-000000000004', '00000000-0000-0000-0000-000000000001', 'Sales Team', 'Sales and business development', 'sales', NOW()),
('b0000000-0000-0000-0000-000000000005', '00000000-0000-0000-0000-000000000001', 'Marketing Team', 'Marketing and branding', 'marketing', NOW()),
('b0000000-0000-0000-0000-000000000006', '00000000-0000-0000-0000-000000000001', 'Customer Support', 'Customer-facing support', 'support', NOW()),
('b0000000-0000-0000-0000-000000000007', '00000000-0000-0000-0000-000000000001', 'Finance & Accounting', 'Finance operations', 'corporate', NOW()),
('b0000000-0000-0000-0000-000000000008', '00000000-0000-0000-0000-000000000001', 'People & Culture', 'HR and employee experience', 'corporate', NOW()),
('b0000000-0000-0000-0000-000000000009', '00000000-0000-0000-0000-000000000001', 'Product Launch Q2', 'Cross-team product launch initiative', 'project', NOW())
ON CONFLICT (company_id, name) DO NOTHING;

-- Team Members
INSERT INTO team_members (team_id, employee_id, role) VALUES
('b0000000-0000-0000-0000-000000000001', 'a0000000-0000-0000-0000-000000000001', 'lead'),
('b0000000-0000-0000-0000-000000000001', 'a0000000-0000-0000-0000-000000000003', 'member'),
('b0000000-0000-0000-0000-000000000001', 'a0000000-0000-0000-0000-000000000015', 'member'),
('b0000000-0000-0000-0000-000000000001', 'a0000000-0000-0000-0000-000000000005', 'member'),
('b0000000-0000-0000-0000-000000000002', 'a0000000-0000-0000-0000-000000000015', 'lead'),
('b0000000-0000-0000-0000-000000000002', 'a0000000-0000-0000-0000-000000000012', 'member'),
('b0000000-0000-0000-0000-000000000002', 'a0000000-0000-0000-0000-000000000011', 'member'),
('b0000000-0000-0000-0000-000000000003', 'a0000000-0000-0000-0000-000000000009', 'member'),
('b0000000-0000-0000-0000-000000000003', 'a0000000-0000-0000-0000-000000000011', 'member'),
('b0000000-0000-0000-0000-000000000004', 'a0000000-0000-0000-0000-000000000013', 'lead'),
('b0000000-0000-0000-0000-000000000004', 'a0000000-0000-0000-0000-000000000014', 'member'),
('b0000000-0000-0000-0000-000000000005', 'a0000000-0000-0000-0000-000000000006', 'lead'),
('b0000000-0000-0000-0000-000000000005', 'a0000000-0000-0000-0000-000000000016', 'member'),
('b0000000-0000-0000-0000-000000000006', 'a0000000-0000-0000-0000-000000000017', 'lead'),
('b0000000-0000-0000-0000-000000000006', 'a0000000-0000-0000-0000-000000000018', 'member'),
('b0000000-0000-0000-0000-000000000006', 'a0000000-0000-0000-0000-000000000007', 'member'),
('b0000000-0000-0000-0000-000000000007', 'a0000000-0000-0000-0000-000000000002', 'lead'),
('b0000000-0000-0000-0000-000000000007', 'a0000000-0000-0000-0000-000000000008', 'member'),
('b0000000-0000-0000-0000-000000000007', 'a0000000-0000-0000-0000-000000000019', 'member'),
('b0000000-0000-0000-0000-000000000008', 'a0000000-0000-0000-0000-000000000004', 'lead'),
('b0000000-0000-0000-0000-000000000008', 'a0000000-0000-0000-0000-000000000020', 'member'),
('b0000000-0000-0000-0000-000000000008', 'a0000000-0000-0000-0000-000000000010', 'member'),
('b0000000-0000-0000-0000-000000000009', 'a0000000-0000-0000-0000-000000000001', 'lead'),
('b0000000-0000-0000-0000-000000000009', 'a0000000-0000-0000-0000-000000000006', 'member'),
('b0000000-0000-0000-0000-000000000009', 'a0000000-0000-0000-0000-000000000013', 'member'),
('b0000000-0000-0000-0000-000000000009', 'a0000000-0000-0000-0000-000000000004', 'member')
ON CONFLICT (team_id, employee_id) DO NOTHING;

-- Approved leave requests for team calendar testing
INSERT INTO leave_requests (id, employee_id, company_id, leave_type_id, start_date, end_date, days, reason, status, reviewed_at) VALUES
('c0000000-0000-0000-0000-000000000001', 'a0000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000001',
 (SELECT id FROM leave_types WHERE company_id = '00000000-0000-0000-0000-000000000001' AND name = 'Annual Leave'),
 '2026-03-24', '2026-03-26', 3, 'Family trip', 'approved', NOW()),
('c0000000-0000-0000-0000-000000000002', 'a0000000-0000-0000-0000-000000000003', '00000000-0000-0000-0000-000000000001',
 (SELECT id FROM leave_types WHERE company_id = '00000000-0000-0000-0000-000000000001' AND name = 'Sick Leave'),
 '2026-03-21', '2026-03-21', 1, 'Not feeling well', 'approved', NOW()),
('c0000000-0000-0000-0000-000000000003', 'a0000000-0000-0000-0000-000000000015', '00000000-0000-0000-0000-000000000001',
 (SELECT id FROM leave_types WHERE company_id = '00000000-0000-0000-0000-000000000001' AND name = 'Annual Leave'),
 '2026-03-27', '2026-03-28', 2, 'Personal matters', 'approved', NOW()),
('c0000000-0000-0000-0000-000000000004', 'a0000000-0000-0000-0000-000000000012', '00000000-0000-0000-0000-000000000001',
 (SELECT id FROM leave_types WHERE company_id = '00000000-0000-0000-0000-000000000001' AND name = 'Annual Leave'),
 '2026-03-31', '2026-04-02', 3, 'Vacation', 'approved', NOW()),
('c0000000-0000-0000-0000-000000000005', 'a0000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000001',
 (SELECT id FROM leave_types WHERE company_id = '00000000-0000-0000-0000-000000000001' AND name = 'Compassionate Leave'),
 '2026-03-25', '2026-03-25', 1, 'Family emergency', 'approved', NOW()),
('c0000000-0000-0000-0000-000000000006', 'a0000000-0000-0000-0000-000000000013', '00000000-0000-0000-0000-000000000001',
 (SELECT id FROM leave_types WHERE company_id = '00000000-0000-0000-0000-000000000001' AND name = 'Annual Leave'),
 '2026-04-01', '2026-04-03', 3, 'Raya holiday extension', 'approved', NOW()),
('c0000000-0000-0000-0000-000000000007', 'a0000000-0000-0000-0000-000000000017', '00000000-0000-0000-0000-000000000001',
 (SELECT id FROM leave_types WHERE company_id = '00000000-0000-0000-0000-000000000001' AND name = 'Sick Leave'),
 '2026-03-20', '2026-03-21', 2, 'Flu', 'approved', NOW()),
('c0000000-0000-0000-0000-000000000008', 'a0000000-0000-0000-0000-000000000006', '00000000-0000-0000-0000-000000000001',
 (SELECT id FROM leave_types WHERE company_id = '00000000-0000-0000-0000-000000000001' AND name = 'Annual Leave'),
 '2026-04-07', '2026-04-09', 3, 'Overseas trip', 'approved', NOW()),
('c0000000-0000-0000-0000-000000000009', 'a0000000-0000-0000-0000-000000000005', '00000000-0000-0000-0000-000000000001',
 (SELECT id FROM leave_types WHERE company_id = '00000000-0000-0000-0000-000000000001' AND name = 'Annual Leave'),
 '2026-03-28', '2026-03-28', 1, 'Personal', 'pending', NULL)
ON CONFLICT (id) DO NOTHING;
