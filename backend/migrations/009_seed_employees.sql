-- =====================================================
-- Seed Employees for Demo Company
-- Company ID: 00000000-0000-0000-0000-000000000001
-- Payroll Group ID: 00000000-0000-0000-0000-000000000003
-- =====================================================

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
-- 1. Ahmad Razak - Engineering Manager
(
    'a0000000-0000-0000-0000-000000000001',
    '00000000-0000-0000-0000-000000000001',
    'EMP001', 'Ahmad bin Razak', '850615-14-5231', '1985-06-15', 'male',
    'Malaysian', 'malay', 'citizen', 'married',
    'ahmad.razak@demo.com', '012-3456789', '12 Jalan Bukit Bintang', 'Kuala Lumpur', 'Wilayah Persekutuan', '50200',
    'Engineering', 'Engineering Manager', 'permanent', '2020-03-01',
    850000, 'Maybank', '1234567890',
    'SG10085061545231', 'EPF-100001', 'SOCSO-100001', 'EIS-100001',
    TRUE, 2, 'A', TRUE,
    '00000000-0000-0000-0000-000000000003', TRUE
),
-- 2. Siti Nurhaliza - Senior Accountant
(
    'a0000000-0000-0000-0000-000000000002',
    '00000000-0000-0000-0000-000000000001',
    'EMP002', 'Siti Nurhaliza binti Abdullah', '880922-10-6754', '1988-09-22', 'female',
    'Malaysian', 'malay', 'citizen', 'married',
    'siti.nurhaliza@demo.com', '013-9876543', '45 Persiaran Hartamas', 'Kuala Lumpur', 'Wilayah Persekutuan', '50480',
    'Finance', 'Senior Accountant', 'permanent', '2019-07-15',
    650000, 'CIMB', '7600123456',
    'SG10088092266754', 'EPF-100002', 'SOCSO-100002', 'EIS-100002',
    TRUE, 3, 'A', TRUE,
    '00000000-0000-0000-0000-000000000003', TRUE
),
-- 3. Lee Wei Ming - Software Developer
(
    'a0000000-0000-0000-0000-000000000003',
    '00000000-0000-0000-0000-000000000001',
    'EMP003', 'Lee Wei Ming', '920310-08-4523', '1992-03-10', 'male',
    'Malaysian', 'chinese', 'citizen', 'single',
    'wei.ming@demo.com', '016-5551234', '88 Jalan SS2/55', 'Petaling Jaya', 'Selangor', '47300',
    'Engineering', 'Software Developer', 'permanent', '2021-01-10',
    550000, 'Public Bank', '3188456789',
    'SG10092031044523', 'EPF-100003', 'SOCSO-100003', 'EIS-100003',
    FALSE, 0, 'A', FALSE,
    '00000000-0000-0000-0000-000000000003', TRUE
),
-- 4. Priya Nair - HR Executive
(
    'a0000000-0000-0000-0000-000000000004',
    '00000000-0000-0000-0000-000000000001',
    'EMP004', 'Priya a/p Nair', '900715-07-8812', '1990-07-15', 'female',
    'Malaysian', 'indian', 'citizen', 'married',
    'priya.nair@demo.com', '017-2223344', '23 Jalan Ampang Hilir', 'Kuala Lumpur', 'Wilayah Persekutuan', '50450',
    'Human Resources', 'HR Executive', 'permanent', '2022-04-01',
    480000, 'Hong Leong Bank', '0441234567',
    'SG10090071588812', 'EPF-100004', 'SOCSO-100004', 'EIS-100004',
    TRUE, 1, 'A', FALSE,
    '00000000-0000-0000-0000-000000000003', TRUE
),
-- 5. Muhammad Faiz - Junior Developer (contract)
(
    'a0000000-0000-0000-0000-000000000005',
    '00000000-0000-0000-0000-000000000001',
    'EMP005', 'Muhammad Faiz bin Ismail', '970420-01-3345', '1997-04-20', 'male',
    'Malaysian', 'malay', 'citizen', 'single',
    'faiz.ismail@demo.com', '011-98765432', '7 Jalan Desa Utama', 'Shah Alam', 'Selangor', '40150',
    'Engineering', 'Junior Developer', 'contract', '2024-06-01',
    380000, 'RHB Bank', '2140987654',
    'SG10097042013345', 'EPF-100005', 'SOCSO-100005', 'EIS-100005',
    FALSE, 0, 'A', TRUE,
    '00000000-0000-0000-0000-000000000003', TRUE
),
-- 6. Tan Mei Ling - Marketing Manager
(
    'a0000000-0000-0000-0000-000000000006',
    '00000000-0000-0000-0000-000000000001',
    'EMP006', 'Tan Mei Ling', '870812-10-2298', '1987-08-12', 'female',
    'Malaysian', 'chinese', 'citizen', 'divorced',
    'mei.ling@demo.com', '012-8881234', '15 Lorong Maarof', 'Kuala Lumpur', 'Wilayah Persekutuan', '59000',
    'Marketing', 'Marketing Manager', 'permanent', '2018-11-01',
    720000, 'Maybank', '1649876543',
    'SG10087081222298', 'EPF-100006', 'SOCSO-100006', 'EIS-100006',
    FALSE, 1, 'A', FALSE,
    '00000000-0000-0000-0000-000000000003', TRUE
),
-- 7. Rajesh Kumar - Operations Executive
(
    'a0000000-0000-0000-0000-000000000007',
    '00000000-0000-0000-0000-000000000001',
    'EMP007', 'Rajesh a/l Kumar', '910530-14-7761', '1991-05-30', 'male',
    'Malaysian', 'indian', 'citizen', 'married',
    'rajesh.kumar@demo.com', '019-3334455', '32 Taman Desa Jaya', 'Johor Bahru', 'Johor', '81100',
    'Operations', 'Operations Executive', 'permanent', '2023-02-15',
    450000, 'Ambank', '8801234567',
    'SG10091053077761', 'EPF-100007', 'SOCSO-100007', 'EIS-100007',
    TRUE, 2, 'A', FALSE,
    '00000000-0000-0000-0000-000000000003', TRUE
),
-- 8. Nurul Aisyah - Finance Assistant
(
    'a0000000-0000-0000-0000-000000000008',
    '00000000-0000-0000-0000-000000000001',
    'EMP008', 'Nurul Aisyah binti Hassan', '950218-02-4456', '1995-02-18', 'female',
    'Malaysian', 'malay', 'citizen', 'single',
    'nurul.aisyah@demo.com', '014-5556677', '10 Persiaran Raja Chulan', 'Kuala Lumpur', 'Wilayah Persekutuan', '50200',
    'Finance', 'Finance Assistant', 'permanent', '2023-09-01',
    350000, 'Bank Islam', '1200987654',
    'SG10095021844456', 'EPF-100008', 'SOCSO-100008', 'EIS-100008',
    FALSE, 0, 'A', TRUE,
    '00000000-0000-0000-0000-000000000003', TRUE
),
-- 9. Wong Jia Hao - QA Engineer (intern)
(
    'a0000000-0000-0000-0000-000000000009',
    '00000000-0000-0000-0000-000000000001',
    'EMP009', 'Wong Jia Hao', '000815-14-1122', '2000-08-15', 'male',
    'Malaysian', 'chinese', 'citizen', 'single',
    'jia.hao@demo.com', '011-12345678', '5 Jalan Universiti', 'Petaling Jaya', 'Selangor', '46200',
    'Engineering', 'QA Intern', 'intern', '2025-10-01',
    180000, 'Maybank', '1641122334',
    'SG10000081511122', 'EPF-100009', 'SOCSO-100009', 'EIS-100009',
    FALSE, 0, 'A', FALSE,
    '00000000-0000-0000-0000-000000000003', TRUE
),
-- 10. Amirah Zainal - Admin & Office Manager
(
    'a0000000-0000-0000-0000-000000000010',
    '00000000-0000-0000-0000-000000000001',
    'EMP010', 'Amirah binti Zainal', '930401-06-9988', '1993-04-01', 'female',
    'Malaysian', 'malay', 'citizen', 'married',
    'amirah.zainal@demo.com', '018-7778899', '22 Jalan Bangsar', 'Kuala Lumpur', 'Wilayah Persekutuan', '59200',
    'Operations', 'Office Manager', 'permanent', '2021-08-16',
    420000, 'CIMB', '7600654321',
    'SG10093040199988', 'EPF-100010', 'SOCSO-100010', 'EIS-100010',
    TRUE, 1, 'A', TRUE,
    '00000000-0000-0000-0000-000000000003', TRUE
)
ON CONFLICT (id) DO NOTHING;

-- Recurring allowances for some employees
-- Delete any existing seed allowances to avoid duplicates on re-run
DELETE FROM employee_allowances WHERE employee_id IN (
    'a0000000-0000-0000-0000-000000000001',
    'a0000000-0000-0000-0000-000000000002',
    'a0000000-0000-0000-0000-000000000004',
    'a0000000-0000-0000-0000-000000000006',
    'a0000000-0000-0000-0000-000000000007',
    'a0000000-0000-0000-0000-000000000010'
);
INSERT INTO employee_allowances (employee_id, category, name, amount, is_taxable, effective_from) VALUES
('a0000000-0000-0000-0000-000000000001', 'earning', 'Housing Allowance', 50000, TRUE, '2020-03-01'),
('a0000000-0000-0000-0000-000000000001', 'earning', 'Transport Allowance', 30000, TRUE, '2020-03-01'),
('a0000000-0000-0000-0000-000000000002', 'earning', 'Housing Allowance', 40000, TRUE, '2019-07-15'),
('a0000000-0000-0000-0000-000000000004', 'earning', 'Transport Allowance', 20000, TRUE, '2022-04-01'),
('a0000000-0000-0000-0000-000000000006', 'earning', 'Housing Allowance', 45000, TRUE, '2018-11-01'),
('a0000000-0000-0000-0000-000000000006', 'earning', 'Phone Allowance', 15000, FALSE, '2018-11-01'),
('a0000000-0000-0000-0000-000000000007', 'earning', 'Transport Allowance', 20000, TRUE, '2023-02-15'),
('a0000000-0000-0000-0000-000000000010', 'earning', 'Parking Allowance', 10000, FALSE, '2021-08-16');
