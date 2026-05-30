-- source: 001_seed_statutory_data.sql

-- Seed statutory rate tables and system settings

-- EPF Third Schedule 2024 - Category A
INSERT INTO epf_rates (wage_from, wage_to, employee_contribution, employer_contribution, category, effective_from) VALUES
(1, 3000, 0, 500, 'A', '2024-01-01'),
(3001, 5000, 500, 500, 'A', '2024-01-01'),
(5001, 7000, 500, 1000, 'A', '2024-01-01'),
(7001, 10000, 1000, 1000, 'A', '2024-01-01'),
(10001, 12000, 1000, 1500, 'A', '2024-01-01'),
(12001, 15000, 1500, 1500, 'A', '2024-01-01'),
(15001, 17000, 1500, 2000, 'A', '2024-01-01'),
(17001, 20000, 2000, 2000, 'A', '2024-01-01'),
(20001, 25000, 2500, 2500, 'A', '2024-01-01'),
(25001, 30000, 3000, 3500, 'A', '2024-01-01'),
(30001, 40000, 3500, 4500, 'A', '2024-01-01'),
(40001, 50000, 5000, 5500, 'A', '2024-01-01'),
(50001, 60000, 5500, 6500, 'A', '2024-01-01'),
(60001, 70000, 7000, 7500, 'A', '2024-01-01'),
(70001, 80000, 8000, 9000, 'A', '2024-01-01'),
(80001, 90000, 9000, 10000, 'A', '2024-01-01'),
(90001, 100000, 10500, 11500, 'A', '2024-01-01'),
(100001, 120000, 12000, 14000, 'A', '2024-01-01'),
(120001, 140000, 14500, 16500, 'A', '2024-01-01'),
(140001, 160000, 16500, 19000, 'A', '2024-01-01'),
(160001, 180000, 19000, 21000, 'A', '2024-01-01'),
(180001, 200000, 21000, 23500, 'A', '2024-01-01'),
(200001, 220000, 23500, 26000, 'A', '2024-01-01'),
(220001, 240000, 25500, 28000, 'A', '2024-01-01'),
(240001, 260000, 27500, 30500, 'A', '2024-01-01'),
(260001, 280000, 29500, 33000, 'A', '2024-01-01'),
(280001, 300000, 32000, 35000, 'A', '2024-01-01'),
(300001, 320000, 34000, 37500, 'A', '2024-01-01'),
(320001, 340000, 36000, 39500, 'A', '2024-01-01'),
(340001, 360000, 38000, 42000, 'A', '2024-01-01'),
(360001, 380000, 40000, 44500, 'A', '2024-01-01'),
(380001, 400000, 42500, 46500, 'A', '2024-01-01'),
(400001, 420000, 44500, 49000, 'A', '2024-01-01'),
(420001, 440000, 46500, 51000, 'A', '2024-01-01'),
(440001, 460000, 48500, 53500, 'A', '2024-01-01'),
(460001, 480000, 51000, 55500, 'A', '2024-01-01'),
(480001, 500000, 53000, 58000, 'A', '2024-01-01'),
(500001, 520000, 55000, 60000, 'A', '2024-01-01'),
(520001, 540000, 57000, 62400, 'A', '2024-01-01'),
(540001, 560000, 59500, 64800, 'A', '2024-01-01'),
(560001, 580000, 61500, 67200, 'A', '2024-01-01'),
(580001, 600000, 63500, 69600, 'A', '2024-01-01'),
(600001, 650000, 68500, 75000, 'A', '2024-01-01'),
(650001, 700000, 74000, 81000, 'A', '2024-01-01'),
(700001, 750000, 79500, 87000, 'A', '2024-01-01'),
(750001, 800000, 85500, 93000, 'A', '2024-01-01'),
(800001, 850000, 90500, 99000, 'A', '2024-01-01'),
(850001, 900000, 96000, 105000, 'A', '2024-01-01'),
(900001, 950000, 101500, 111000, 'A', '2024-01-01'),
(950001, 1000000, 107500, 117000, 'A', '2024-01-01'),
(1000001, 1100000, 115500, 126000, 'A', '2024-01-01'),
(1100001, 1200000, 126500, 138000, 'A', '2024-01-01'),
(1200001, 1300000, 137500, 150000, 'A', '2024-01-01'),
(1300001, 1400000, 148500, 162000, 'A', '2024-01-01'),
(1400001, 1500000, 159500, 174000, 'A', '2024-01-01'),
(1500001, 1600000, 170500, 186000, 'A', '2024-01-01'),
(1600001, 1700000, 181500, 198000, 'A', '2024-01-01'),
(1700001, 1800000, 192500, 210000, 'A', '2024-01-01'),
(1800001, 1900000, 203500, 222000, 'A', '2024-01-01'),
(1900001, 2000000, 214500, 234000, 'A', '2024-01-01');

-- SOCSO First Schedule 2024
INSERT INTO socso_rates (wage_from, wage_to, first_cat_employee, first_cat_employer, second_cat_employer, effective_from) VALUES
(1, 3000, 5, 15, 15, '2024-01-01'),
(3001, 5000, 15, 35, 25, '2024-01-01'),
(5001, 10000, 35, 65, 45, '2024-01-01'),
(10001, 20000, 65, 135, 85, '2024-01-01'),
(20001, 30000, 105, 195, 125, '2024-01-01'),
(30001, 40000, 145, 265, 175, '2024-01-01'),
(40001, 50000, 175, 335, 215, '2024-01-01'),
(50001, 60000, 215, 395, 255, '2024-01-01'),
(60001, 70000, 245, 465, 295, '2024-01-01'),
(70001, 80000, 285, 535, 335, '2024-01-01'),
(80001, 90000, 325, 595, 375, '2024-01-01'),
(90001, 100000, 355, 665, 415, '2024-01-01'),
(100001, 110000, 395, 735, 455, '2024-01-01'),
(110001, 120000, 435, 795, 495, '2024-01-01'),
(120001, 130000, 465, 865, 535, '2024-01-01'),
(130001, 140000, 505, 935, 575, '2024-01-01'),
(140001, 150000, 545, 995, 615, '2024-01-01'),
(150001, 160000, 575, 1065, 655, '2024-01-01'),
(160001, 170000, 615, 1135, 695, '2024-01-01'),
(170001, 180000, 655, 1195, 735, '2024-01-01'),
(180001, 190000, 685, 1265, 775, '2024-01-01'),
(190001, 200000, 725, 1335, 815, '2024-01-01'),
(200001, 210000, 765, 1395, 855, '2024-01-01'),
(210001, 220000, 795, 1465, 895, '2024-01-01'),
(220001, 230000, 835, 1535, 935, '2024-01-01'),
(230001, 240000, 875, 1595, 975, '2024-01-01'),
(240001, 250000, 905, 1665, 1015, '2024-01-01'),
(250001, 260000, 945, 1735, 1055, '2024-01-01'),
(260001, 270000, 985, 1795, 1095, '2024-01-01'),
(270001, 280000, 1015, 1865, 1135, '2024-01-01'),
(280001, 290000, 1055, 1935, 1175, '2024-01-01'),
(290001, 300000, 1095, 1995, 1215, '2024-01-01'),
(300001, 310000, 1125, 2065, 1255, '2024-01-01'),
(310001, 320000, 1165, 2135, 1295, '2024-01-01'),
(320001, 330000, 1205, 2195, 1335, '2024-01-01'),
(330001, 340000, 1235, 2265, 1375, '2024-01-01'),
(340001, 350000, 1275, 2335, 1415, '2024-01-01'),
(350001, 360000, 1315, 2395, 1455, '2024-01-01'),
(360001, 370000, 1345, 2465, 1495, '2024-01-01'),
(370001, 380000, 1385, 2535, 1535, '2024-01-01'),
(380001, 390000, 1425, 2595, 1575, '2024-01-01'),
(390001, 400000, 1455, 2665, 1615, '2024-01-01'),
(400001, 410000, 1495, 2735, 1655, '2024-01-01'),
(410001, 420000, 1535, 2795, 1695, '2024-01-01'),
(420001, 430000, 1565, 2865, 1735, '2024-01-01'),
(430001, 440000, 1605, 2935, 1775, '2024-01-01'),
(440001, 450000, 1645, 2995, 1815, '2024-01-01'),
(450001, 460000, 1675, 3065, 1855, '2024-01-01'),
(460001, 470000, 1715, 3135, 1895, '2024-01-01'),
(470001, 480000, 1755, 3195, 1935, '2024-01-01'),
(480001, 490000, 1785, 3265, 1975, '2024-01-01'),
(490001, 500000, 1825, 3335, 2015, '2024-01-01'),
(500001, 600000, 2175, 3985, 2415, '2024-01-01');

-- EIS Contribution Table 2024
INSERT INTO eis_rates (wage_from, wage_to, employee_contribution, employer_contribution, effective_from) VALUES
(1, 3000, 5, 5, '2024-01-01'),
(3001, 5000, 10, 10, '2024-01-01'),
(5001, 10000, 15, 15, '2024-01-01'),
(10001, 20000, 35, 35, '2024-01-01'),
(20001, 30000, 50, 50, '2024-01-01'),
(30001, 40000, 70, 70, '2024-01-01'),
(40001, 50000, 90, 90, '2024-01-01'),
(50001, 60000, 110, 110, '2024-01-01'),
(60001, 70000, 130, 130, '2024-01-01'),
(70001, 80000, 150, 150, '2024-01-01'),
(80001, 90000, 170, 170, '2024-01-01'),
(90001, 100000, 190, 190, '2024-01-01'),
(100001, 110000, 210, 210, '2024-01-01'),
(110001, 120000, 230, 230, '2024-01-01'),
(120001, 130000, 250, 250, '2024-01-01'),
(130001, 140000, 270, 270, '2024-01-01'),
(140001, 150000, 290, 290, '2024-01-01'),
(150001, 160000, 310, 310, '2024-01-01'),
(160001, 170000, 330, 330, '2024-01-01'),
(170001, 180000, 350, 350, '2024-01-01'),
(180001, 190000, 370, 370, '2024-01-01'),
(190001, 200000, 390, 390, '2024-01-01'),
(200001, 210000, 410, 410, '2024-01-01'),
(210001, 220000, 430, 430, '2024-01-01'),
(220001, 230000, 450, 450, '2024-01-01'),
(230001, 240000, 470, 470, '2024-01-01'),
(240001, 250000, 490, 490, '2024-01-01'),
(250001, 260000, 510, 510, '2024-01-01'),
(260001, 270000, 530, 530, '2024-01-01'),
(270001, 280000, 550, 550, '2024-01-01'),
(280001, 290000, 570, 570, '2024-01-01'),
(290001, 300000, 590, 590, '2024-01-01'),
(300001, 310000, 610, 610, '2024-01-01'),
(310001, 320000, 630, 630, '2024-01-01'),
(320001, 330000, 650, 650, '2024-01-01'),
(330001, 340000, 670, 670, '2024-01-01'),
(340001, 350000, 690, 690, '2024-01-01'),
(350001, 360000, 710, 710, '2024-01-01'),
(360001, 370000, 730, 730, '2024-01-01'),
(370001, 380000, 750, 750, '2024-01-01'),
(380001, 390000, 770, 770, '2024-01-01'),
(390001, 400000, 790, 790, '2024-01-01'),
(400001, 410000, 810, 810, '2024-01-01'),
(410001, 420000, 830, 830, '2024-01-01'),
(420001, 430000, 850, 850, '2024-01-01'),
(430001, 440000, 870, 870, '2024-01-01'),
(440001, 450000, 890, 890, '2024-01-01'),
(450001, 460000, 910, 910, '2024-01-01'),
(460001, 470000, 930, 930, '2024-01-01'),
(470001, 480000, 950, 950, '2024-01-01'),
(480001, 490000, 970, 970, '2024-01-01'),
(490001, 500000, 990, 990, '2024-01-01');

-- PCB Tax Brackets 2024
INSERT INTO pcb_brackets (chargeable_income_from, chargeable_income_to, tax_rate_percent, cumulative_tax, effective_year) VALUES
(0, 500000, 0, 0, 2024),
(500001, 2000000, 1, 0, 2024),
(2000001, 3500000, 3, 1500000, 2024),
(3500001, 5000000, 6, 6000000, 2024),
(5000001, 7000000, 11, 15000000, 2024),
(7000001, 10000000, 19, 37000000, 2024),
(10000001, 40000000, 25, 94000000, 2024),
(40000001, 60000000, 26, 844000000, 2024),
(60000001, 200000000, 28, 1364000000, 2024),
(200000001, 9999999999, 30, 5284000000, 2024);

-- PCB Reliefs 2024
INSERT INTO pcb_reliefs (relief_type, amount, effective_year, description) VALUES
('individual', 900000, 2024, 'Individual relief RM9,000'),
('spouse', 400000, 2024, 'Spouse relief RM4,000 (non-working spouse)'),
('child_under_18', 200000, 2024, 'Child relief RM2,000 per child under 18'),
('child_18_plus_education', 800000, 2024, 'Child relief RM8,000 per child 18+ in higher education'),
('disabled_individual', 600000, 2024, 'Additional disabled individual RM6,000'),
('disabled_spouse', 500000, 2024, 'Additional disabled spouse RM5,000'),
('disabled_child', 600000, 2024, 'Additional disabled child RM6,000'),
('life_insurance', 300000, 2024, 'Life insurance / EPF relief cap RM3,000'),
('epf_additional', 400000, 2024, 'Additional voluntary EPF RM4,000'),
('medical_insurance', 300000, 2024, 'Medical/education insurance RM3,000'),
('socso_relief', 35000, 2024, 'SOCSO employee contribution relief RM350'),
('eis_relief', 35000, 2024, 'EIS employee contribution relief RM350'),
('lifestyle_relief', 250000, 2024, 'Lifestyle relief RM2,500'),
('tax_rebate_individual', 40000, 2024, 'Tax rebate RM400 if chargeable income <= RM35,000');

-- System Settings
INSERT INTO system_settings (setting_key, setting_value, description, effective_from) VALUES
('minimum_wage', '150000', 'Minimum wage RM1,500/month in sen', '2023-01-01'),
('standard_working_hours_day', '8', 'Standard working hours per day', '2023-01-01'),
('standard_working_hours_week', '45', 'Standard working hours per week (Peninsular + Labuan)', '2023-01-01'),
('ot_divisor_days', '26', 'Monthly OT hourly rate divisor (days)', '2023-01-01'),
('ot_divisor_hours', '8', 'Daily OT hourly rate divisor (hours)', '2023-01-01'),
('ot_rate_normal', '1.5', 'OT multiplier for normal working day', '2023-01-01'),
('ot_rate_rest_day', '2.0', 'OT multiplier for rest day', '2023-01-01'),
('ot_rate_public_holiday', '3.0', 'OT multiplier for public holiday', '2023-01-01'),
('epf_wage_ceiling', '0', 'EPF has no wage ceiling (0 = unlimited)', '2024-01-01'),
('socso_wage_ceiling', '600000', 'SOCSO wage ceiling RM6,000 in sen', '2024-01-01'),
('eis_wage_ceiling', '500000', 'EIS wage ceiling RM5,000 in sen', '2024-01-01'),
('pcb_rounding', 'up', 'PCB rounds up to nearest RM', '2024-01-01');

-- source: 002_seed_company_users_employees.sql

-- Seed demo company, users, employees, and allowances

-- Demo Company
INSERT INTO companies (id, name, registration_number, tax_number)
VALUES ('00000000-0000-0000-0000-000000000001', 'Demo Company Sdn Bhd', '202301012345', 'C2023-001234');

-- Password: admin123 (bcrypt hash)
-- Super Admin user (superadmin@demo.com)
INSERT INTO users (id, email, password_hash, full_name, roles, company_id)
VALUES (
    '00000000-0000-0000-0000-000000000002',
    'superadmin@demo.com',
    '$2b$12$fRjUOo/hbE.JoyBkrDmmg.0nkGxVzL6dQXJuUHZ2dgDBZcdvX3yA2',
    'Super Administrator',
    ARRAY['super_admin']::varchar(50)[],
    '00000000-0000-0000-0000-000000000001'
);

-- Admin user (admin@demo.com)
INSERT INTO users (id, email, password_hash, full_name, roles, company_id)
VALUES (
    '00000000-0000-0000-0000-000000000021',
    'admin@demo.com',
    '$2b$12$fRjUOo/hbE.JoyBkrDmmg.0nkGxVzL6dQXJuUHZ2dgDBZcdvX3yA2',
    'Company Administrator',
    ARRAY['admin']::varchar(50)[],
    '00000000-0000-0000-0000-000000000001'
);

-- Executive Director user (exec@demo.com)
INSERT INTO users (id, email, password_hash, full_name, roles, company_id)
VALUES (
    '00000000-0000-0000-0000-000000000020',
    'exec@demo.com',
    '$2b$12$fRjUOo/hbE.JoyBkrDmmg.0nkGxVzL6dQXJuUHZ2dgDBZcdvX3yA2',
    'Executive Director',
    ARRAY['exec']::varchar(50)[],
    '00000000-0000-0000-0000-000000000001'
);

-- Employee portal user (sarah@demo.com)
INSERT INTO users (id, email, password_hash, full_name, roles, company_id)
VALUES (
    '00000000-0000-0000-0000-000000000010',
    'sarah@demo.com',
    '$2b$12$fRjUOo/hbE.JoyBkrDmmg.0nkGxVzL6dQXJuUHZ2dgDBZcdvX3yA2',
    'Sarah Lee',
    ARRAY['employee']::varchar(50)[],
    '00000000-0000-0000-0000-000000000001'
);

-- Default payroll group
INSERT INTO payroll_groups (id, company_id, name, description, cutoff_day, payment_day)
VALUES (
    '00000000-0000-0000-0000-000000000003',
    '00000000-0000-0000-0000-000000000001',
    'Default',
    'Default monthly payroll group',
    25,
    28
);

-- Link sarah to employee after employees are inserted (done via UPDATE below)

-- Employees (EMP001–EMP010)
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

-- Link sarah user to EMP002
UPDATE users SET employee_id = (
    SELECT id FROM employees WHERE employee_number = 'EMP002' AND company_id = '00000000-0000-0000-0000-000000000001' LIMIT 1
) WHERE id = '00000000-0000-0000-0000-000000000010';

-- Seed user_companies junction table
INSERT INTO user_companies (user_id, company_id)
SELECT id, company_id FROM users WHERE company_id IS NOT NULL
ON CONFLICT DO NOTHING;

-- Employee allowances
INSERT INTO employee_allowances (employee_id, category, name, amount, is_taxable, effective_from) VALUES
('a0000000-0000-0000-0000-000000000001', 'earning', 'Housing Allowance', 50000, TRUE, '2020-03-01'),
('a0000000-0000-0000-0000-000000000001', 'earning', 'Transport Allowance', 30000, TRUE, '2020-03-01'),
('a0000000-0000-0000-0000-000000000002', 'earning', 'Housing Allowance', 40000, TRUE, '2019-07-15'),
('a0000000-0000-0000-0000-000000000004', 'earning', 'Transport Allowance', 20000, TRUE, '2022-04-01'),
('a0000000-0000-0000-0000-000000000006', 'earning', 'Housing Allowance', 45000, TRUE, '2018-11-01'),
('a0000000-0000-0000-0000-000000000006', 'earning', 'Phone Allowance', 15000, FALSE, '2018-11-01'),
('a0000000-0000-0000-0000-000000000007', 'earning', 'Transport Allowance', 20000, TRUE, '2023-02-15'),
('a0000000-0000-0000-0000-000000000010', 'earning', 'Parking Allowance', 10000, FALSE, '2021-08-16');

-- source: 003_seed_documents_settings_calendar.sql

-- Seed document categories, documents, company settings, leave types, balances, calendar

-- Document categories
INSERT INTO document_categories (company_id, name, description)
SELECT c.id, cat.name, cat.description
FROM companies c
CROSS JOIN (VALUES
    ('IC / Passport', 'Identity card or passport copy'),
    ('Offer Letter', 'Employment offer letter'),
    ('Contract', 'Employment contract'),
    ('Tax Form', 'EA form, TP3, or other tax documents'),
    ('Certification', 'Professional certifications and licenses'),
    ('Medical', 'Medical reports or insurance documents'),
    ('Leave', 'Leave application forms'),
    ('Other', 'Miscellaneous documents')
) AS cat(name, description);

-- Seed documents
DO $$
DECLARE
    v_company_id UUID := '00000000-0000-0000-0000-000000000001';
    v_admin_id UUID := '00000000-0000-0000-0000-000000000002';
    v_cat_ic UUID;
    v_cat_offer UUID;
    v_cat_contract UUID;
    v_cat_tax UUID;
    v_cat_cert UUID;
    v_cat_medical UUID;
    v_cat_other UUID;
BEGIN
    SELECT id INTO v_cat_ic FROM document_categories WHERE company_id = v_company_id AND name = 'IC / Passport';
    SELECT id INTO v_cat_offer FROM document_categories WHERE company_id = v_company_id AND name = 'Offer Letter';
    SELECT id INTO v_cat_contract FROM document_categories WHERE company_id = v_company_id AND name = 'Contract';
    SELECT id INTO v_cat_tax FROM document_categories WHERE company_id = v_company_id AND name = 'Tax Form';
    SELECT id INTO v_cat_cert FROM document_categories WHERE company_id = v_company_id AND name = 'Certification';
    SELECT id INTO v_cat_medical FROM document_categories WHERE company_id = v_company_id AND name = 'Medical';
    SELECT id INTO v_cat_other FROM document_categories WHERE company_id = v_company_id AND name = 'Other';

    INSERT INTO documents (company_id, employee_id, category_id, title, description, file_name, file_url, file_size, mime_type, status, issue_date, is_confidential, created_by, updated_by)
    VALUES
    (v_company_id, 'a0000000-0000-0000-0000-000000000001', v_cat_ic, 'MyKad Copy', 'IC copy - 850615-14-5231', 'ahmad_razak_ic.pdf', '/documents/emp001/ahmad_razak_ic.pdf', 245000, 'application/pdf', 'active', '2020-02-25', TRUE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000001', v_cat_offer, 'Offer Letter', 'Engineering Manager offer letter', 'ahmad_razak_offer.pdf', '/documents/emp001/ahmad_razak_offer.pdf', 180000, 'application/pdf', 'active', '2020-02-15', FALSE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000001', v_cat_contract, 'Employment Contract', 'Permanent employment contract', 'ahmad_razak_contract.pdf', '/documents/emp001/ahmad_razak_contract.pdf', 320000, 'application/pdf', 'active', '2020-03-01', TRUE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000001', v_cat_tax, 'EA Form 2024', 'Annual remuneration statement for YA2024', 'ahmad_razak_ea_2024.pdf', '/documents/emp001/ahmad_razak_ea_2024.pdf', 150000, 'application/pdf', 'active', '2025-02-28', FALSE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000001', v_cat_cert, 'PMP Certification', 'Project Management Professional certificate', 'ahmad_razak_pmp.pdf', '/documents/emp001/ahmad_razak_pmp.pdf', 520000, 'application/pdf', 'active', '2022-05-10', FALSE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000001', v_cat_tax, 'EA Form 2023', 'Annual remuneration statement for YA2023', 'ahmad_razak_ea_2023.pdf', '/documents/emp001/ahmad_razak_ea_2023.pdf', 148000, 'application/pdf', 'active', '2024-02-28', FALSE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000002', v_cat_ic, 'MyKad Copy', 'IC copy - 880922-10-6754', 'siti_nurhaliza_ic.pdf', '/documents/emp002/siti_nurhaliza_ic.pdf', 230000, 'application/pdf', 'active', '2019-07-01', TRUE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000002', v_cat_offer, 'Offer Letter', 'Senior Accountant offer letter', 'siti_nurhaliza_offer.pdf', '/documents/emp002/siti_nurhaliza_offer.pdf', 175000, 'application/pdf', 'active', '2019-06-20', FALSE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000002', v_cat_contract, 'Employment Contract', 'Permanent employment contract', 'siti_nurhaliza_contract.pdf', '/documents/emp002/siti_nurhaliza_contract.pdf', 310000, 'application/pdf', 'active', '2019-07-15', TRUE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000002', v_cat_tax, 'EA Form 2024', 'Annual remuneration statement for YA2024', 'siti_nurhaliza_ea_2024.pdf', '/documents/emp002/siti_nurhaliza_ea_2024.pdf', 145000, 'application/pdf', 'active', '2025-02-28', FALSE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000002', v_cat_cert, 'ACCA Membership', 'Association of Chartered Certified Accountants', 'siti_nurhaliza_acca.pdf', '/documents/emp002/siti_nurhaliza_acca.pdf', 480000, 'application/pdf', 'active', '2018-12-01', FALSE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000002', v_cat_cert, 'MIA Registration', 'Malaysian Institute of Accountants membership', 'siti_nurhaliza_mia.pdf', '/documents/emp002/siti_nurhaliza_mia.pdf', 290000, 'application/pdf', 'active', '2019-03-15', FALSE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000003', v_cat_ic, 'MyKad Copy', 'IC copy - 920310-08-4523', 'lee_wei_ming_ic.pdf', '/documents/emp003/lee_wei_ming_ic.pdf', 240000, 'application/pdf', 'active', '2021-01-05', TRUE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000003', v_cat_offer, 'Offer Letter', 'Software Developer offer letter', 'lee_wei_ming_offer.pdf', '/documents/emp003/lee_wei_ming_offer.pdf', 170000, 'application/pdf', 'active', '2020-12-15', FALSE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000003', v_cat_contract, 'Employment Contract', 'Permanent employment contract', 'lee_wei_ming_contract.pdf', '/documents/emp003/lee_wei_ming_contract.pdf', 305000, 'application/pdf', 'active', '2021-01-10', TRUE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000003', v_cat_tax, 'EA Form 2024', 'Annual remuneration statement for YA2024', 'lee_wei_ming_ea_2024.pdf', '/documents/emp003/lee_wei_ming_ea_2024.pdf', 142000, 'application/pdf', 'active', '2025-02-28', FALSE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000003', v_cat_cert, 'AWS Solutions Architect', 'AWS Certified Solutions Architect - Associate', 'lee_wei_ming_aws_sa.pdf', '/documents/emp003/lee_wei_ming_aws_sa.pdf', 350000, 'application/pdf', 'active', '2023-08-20', FALSE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000004', v_cat_ic, 'MyKad Copy', 'IC copy - 900715-07-8812', 'priya_nair_ic.pdf', '/documents/emp004/priya_nair_ic.pdf', 235000, 'application/pdf', 'active', '2022-03-20', TRUE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000004', v_cat_offer, 'Offer Letter', 'HR Executive offer letter', 'priya_nair_offer.pdf', '/documents/emp004/priya_nair_offer.pdf', 168000, 'application/pdf', 'active', '2022-03-15', FALSE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000004', v_cat_contract, 'Employment Contract', 'Permanent employment contract', 'priya_nair_contract.pdf', '/documents/emp004/priya_nair_contract.pdf', 298000, 'application/pdf', 'active', '2022-04-01', TRUE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000004', v_cat_tax, 'EA Form 2024', 'Annual remuneration statement for YA2024', 'priya_nair_ea_2024.pdf', '/documents/emp004/priya_nair_ea_2024.pdf', 140000, 'application/pdf', 'active', '2025-02-28', FALSE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000004', v_cat_cert, 'SHRM-CP Certification', 'Society for Human Resource Management - Certified Professional', 'priya_nair_shrm.pdf', '/documents/emp004/priya_nair_shrm.pdf', 410000, 'application/pdf', 'active', '2023-11-05', FALSE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000004', v_cat_medical, 'Pre-Employment Medical Report', 'Medical check-up report', 'priya_nair_medical.pdf', '/documents/emp004/priya_nair_medical.pdf', 890000, 'application/pdf', 'active', '2022-03-25', TRUE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000005', v_cat_ic, 'MyKad Copy', 'IC copy - 970420-01-3345', 'faiz_ismail_ic.pdf', '/documents/emp005/faiz_ismail_ic.pdf', 228000, 'application/pdf', 'active', '2024-05-20', TRUE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000005', v_cat_offer, 'Offer Letter', 'Junior Developer contract offer', 'faiz_ismail_offer.pdf', '/documents/emp005/faiz_ismail_offer.pdf', 165000, 'application/pdf', 'active', '2024-05-15', FALSE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000005', v_cat_contract, 'Employment Contract', 'Fixed-term contract (12 months)', 'faiz_ismail_contract.pdf', '/documents/emp005/faiz_ismail_contract.pdf', 340000, 'application/pdf', 'active', '2024-06-01', TRUE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000005', v_cat_other, 'Degree Certificate', 'Bachelor of Computer Science - UTM', 'faiz_ismail_degree.pdf', '/documents/emp005/faiz_ismail_degree.pdf', 560000, 'application/pdf', 'active', '2023-10-20', FALSE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000006', v_cat_ic, 'MyKad Copy', 'IC copy - 870812-10-2298', 'tan_mei_ling_ic.pdf', '/documents/emp006/tan_mei_ling_ic.pdf', 242000, 'application/pdf', 'active', '2018-10-15', TRUE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000006', v_cat_offer, 'Offer Letter', 'Marketing Manager offer letter', 'tan_mei_ling_offer.pdf', '/documents/emp006/tan_mei_ling_offer.pdf', 182000, 'application/pdf', 'active', '2018-10-10', FALSE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000006', v_cat_contract, 'Employment Contract', 'Permanent employment contract', 'tan_mei_ling_contract.pdf', '/documents/emp006/tan_mei_ling_contract.pdf', 315000, 'application/pdf', 'active', '2018-11-01', TRUE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000006', v_cat_tax, 'EA Form 2024', 'Annual remuneration statement for YA2024', 'tan_mei_ling_ea_2024.pdf', '/documents/emp006/tan_mei_ling_ea_2024.pdf', 147000, 'application/pdf', 'active', '2025-02-28', FALSE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000006', v_cat_cert, 'Google Ads Certification', 'Google Ads Search certification', 'tan_mei_ling_google_ads.pdf', '/documents/emp006/tan_mei_ling_google_ads.pdf', 280000, 'application/pdf', 'active', '2024-07-15', FALSE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000006', v_cat_tax, 'EA Form 2023', 'Annual remuneration statement for YA2023', 'tan_mei_ling_ea_2023.pdf', '/documents/emp006/tan_mei_ling_ea_2023.pdf', 144000, 'application/pdf', 'active', '2024-02-28', FALSE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000007', v_cat_ic, 'MyKad Copy', 'IC copy - 910530-14-7761', 'rajesh_kumar_ic.pdf', '/documents/emp007/rajesh_kumar_ic.pdf', 238000, 'application/pdf', 'active', '2023-02-01', TRUE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000007', v_cat_offer, 'Offer Letter', 'Operations Executive offer letter', 'rajesh_kumar_offer.pdf', '/documents/emp007/rajesh_kumar_offer.pdf', 172000, 'application/pdf', 'active', '2023-01-20', FALSE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000007', v_cat_contract, 'Employment Contract', 'Permanent employment contract', 'rajesh_kumar_contract.pdf', '/documents/emp007/rajesh_kumar_contract.pdf', 308000, 'application/pdf', 'active', '2023-02-15', TRUE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000007', v_cat_tax, 'EA Form 2024', 'Annual remuneration statement for YA2024', 'rajesh_kumar_ea_2024.pdf', '/documents/emp007/rajesh_kumar_ea_2024.pdf', 141000, 'application/pdf', 'active', '2025-02-28', FALSE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000007', v_cat_medical, 'Pre-Employment Medical Report', 'Medical check-up report', 'rajesh_kumar_medical.pdf', '/documents/emp007/rajesh_kumar_medical.pdf', 920000, 'application/pdf', 'active', '2023-02-10', TRUE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000008', v_cat_ic, 'MyKad Copy', 'IC copy - 950218-02-4456', 'nurul_aisyah_ic.pdf', '/documents/emp008/nurul_aisyah_ic.pdf', 232000, 'application/pdf', 'active', '2023-08-20', TRUE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000008', v_cat_offer, 'Offer Letter', 'Finance Assistant offer letter', 'nurul_aisyah_offer.pdf', '/documents/emp008/nurul_aisyah_offer.pdf', 162000, 'application/pdf', 'active', '2023-08-15', FALSE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000008', v_cat_contract, 'Employment Contract', 'Permanent employment contract', 'nurul_aisyah_contract.pdf', '/documents/emp008/nurul_aisyah_contract.pdf', 295000, 'application/pdf', 'active', '2023-09-01', TRUE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000008', v_cat_tax, 'EA Form 2024', 'Annual remuneration statement for YA2024', 'nurul_aisyah_ea_2024.pdf', '/documents/emp008/nurul_aisyah_ea_2024.pdf', 138000, 'application/pdf', 'active', '2025-02-28', FALSE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000008', v_cat_other, 'Diploma Certificate', 'Diploma in Accounting - Politeknik', 'nurul_aisyah_diploma.pdf', '/documents/emp008/nurul_aisyah_diploma.pdf', 480000, 'application/pdf', 'active', '2017-09-15', FALSE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000009', v_cat_ic, 'MyKad Copy', 'IC copy - 000815-14-1122', 'wong_jia_hao_ic.pdf', '/documents/emp009/wong_jia_hao_ic.pdf', 225000, 'application/pdf', 'active', '2025-09-20', TRUE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000009', v_cat_offer, 'Internship Offer Letter', 'QA Intern offer letter (6-month internship)', 'wong_jia_hao_offer.pdf', '/documents/emp009/wong_jia_hao_offer.pdf', 155000, 'application/pdf', 'active', '2025-09-10', FALSE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000009', v_cat_contract, 'Internship Agreement', 'Industrial training agreement with university', 'wong_jia_hao_intern_agreement.pdf', '/documents/emp009/wong_jia_hao_intern_agreement.pdf', 280000, 'application/pdf', 'active', '2025-10-01', FALSE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000009', v_cat_other, 'University Endorsement Letter', 'Letter from university confirming internship placement', 'wong_jia_hao_uni_letter.pdf', '/documents/emp009/wong_jia_hao_uni_letter.pdf', 190000, 'application/pdf', 'active', '2025-09-05', FALSE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000010', v_cat_ic, 'MyKad Copy', 'IC copy - 930401-06-9988', 'amirah_zainal_ic.pdf', '/documents/emp010/amirah_zainal_ic.pdf', 237000, 'application/pdf', 'active', '2021-08-01', TRUE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000010', v_cat_offer, 'Offer Letter', 'Office Manager offer letter', 'amirah_zainal_offer.pdf', '/documents/emp010/amirah_zainal_offer.pdf', 176000, 'application/pdf', 'active', '2021-07-25', FALSE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000010', v_cat_contract, 'Employment Contract', 'Permanent employment contract', 'amirah_zainal_contract.pdf', '/documents/emp010/amirah_zainal_contract.pdf', 302000, 'application/pdf', 'active', '2021-08-16', TRUE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000010', v_cat_tax, 'EA Form 2024', 'Annual remuneration statement for YA2024', 'amirah_zainal_ea_2024.pdf', '/documents/emp010/amirah_zainal_ea_2024.pdf', 139000, 'application/pdf', 'active', '2025-02-28', FALSE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000010', v_cat_medical, 'Pre-Employment Medical Report', 'Medical check-up report', 'amirah_zainal_medical.pdf', '/documents/emp010/amirah_zainal_medical.pdf', 875000, 'application/pdf', 'active', '2021-08-10', TRUE, v_admin_id, v_admin_id),
    (v_company_id, 'a0000000-0000-0000-0000-000000000010', v_cat_cert, 'OSH Certification', 'Occupational Safety and Health awareness certificate', 'amirah_zainal_osh.pdf', '/documents/emp010/amirah_zainal_osh.pdf', 310000, 'application/pdf', 'active', '2024-03-20', FALSE, v_admin_id, v_admin_id);
END $$;

-- Company settings
INSERT INTO company_settings (company_id, category, key, value, label, description)
SELECT c.id, d.category, d.key, d.value::jsonb, d.label, d.description
FROM companies c
CROSS JOIN (VALUES
    ('payroll', 'default_pay_day', '"28"', 'Default Pay Day', 'Day of month for salary payment'),
    ('payroll', 'default_cutoff_day', '"25"', 'Default Cutoff Day', 'Day of month for payroll cutoff'),
    ('payroll', 'overtime_multiplier_normal', '"1.5"', 'OT Multiplier (Normal)', 'Overtime rate multiplier for normal working days'),
    ('payroll', 'overtime_multiplier_rest', '"2.0"', 'OT Multiplier (Rest Day)', 'Overtime rate multiplier for rest days'),
    ('payroll', 'overtime_multiplier_public', '"3.0"', 'OT Multiplier (Public Holiday)', 'Overtime rate multiplier for public holidays'),
    ('payroll', 'unpaid_leave_divisor', '"26"', 'Unpaid Leave Divisor', 'Number of working days to divide salary for unpaid leave deduction'),
    ('payroll', 'rounding_method', '"nearest"', 'Rounding Method', 'How to round salary calculations: nearest, up, down'),
    ('payroll', 'working_hours_per_day', '"9"', 'Working Hours Per Day', 'Total office hours per day including rest time'),
    ('payroll', 'rest_time_minutes', '"60"', 'Rest Time (minutes)', 'Afternoon rest/lunch break duration in minutes'),
    ('payroll', 'effective_hours_per_day', '"8"', 'Effective Hours Per Day', 'Actual working hours per day after deducting rest time'),
    ('statutory', 'epf_employer_rate_below_60', '"13"', 'EPF Employer Rate (< 60)', 'EPF employer contribution percentage for employees under 60'),
    ('statutory', 'epf_employer_rate_above_60', '"6.5"', 'EPF Employer Rate (>= 60)', 'EPF employer contribution percentage for employees 60 and above'),
    ('statutory', 'socso_enabled', 'true', 'SOCSO Enabled', 'Whether SOCSO contributions are active'),
    ('statutory', 'eis_enabled', 'true', 'EIS Enabled', 'Whether EIS contributions are active'),
    ('statutory', 'hrdf_enabled', 'false', 'HRDF Enabled', 'Whether HRDF levy is active'),
    ('statutory', 'hrdf_rate', '"1"', 'HRDF Rate (%)', 'HRDF levy rate percentage'),
    ('system', 'currency', '"MYR"', 'Currency', 'System currency code'),
    ('system', 'date_format', '"DD/MM/YYYY"', 'Date Format', 'Display date format across the system'),
    ('system', 'financial_year_start_month', '"1"', 'Financial Year Start', 'Month number when financial year begins'),
    ('system', 'payslip_template', '"default"', 'Payslip Template', 'Template used for generating payslips'),
    ('notifications', 'email_payslip', 'true', 'Email Payslips', 'Automatically email payslips to employees after payroll approval'),
    ('notifications', 'expiry_alert_days', '"30"', 'Document Expiry Alert (days)', 'Days before document expiry to send alerts'),
    ('notifications', 'probation_alert_days', '"14"', 'Probation End Alert (days)', 'Days before probation end to send alerts'),
    ('email', 'auto_welcome_email', 'true', 'Auto Welcome Email', 'Automatically send welcome email when a new employee is created')
) AS d(category, key, value, label, description)
ON CONFLICT (company_id, category, key) DO NOTHING;

-- Leave types
INSERT INTO leave_types (company_id, name, description, default_days) VALUES
('00000000-0000-0000-0000-000000000001', 'Annual Leave', 'Paid annual leave entitlement', 14),
('00000000-0000-0000-0000-000000000001', 'Sick Leave', 'Paid sick leave (outpatient)', 14),
('00000000-0000-0000-0000-000000000001', 'Hospitalisation Leave', 'Paid hospitalisation leave', 60),
('00000000-0000-0000-0000-000000000001', 'Compassionate Leave', 'Bereavement / compassionate leave', 3),
('00000000-0000-0000-0000-000000000001', 'Maternity Leave', 'Paid maternity leave', 98),
('00000000-0000-0000-0000-000000000001', 'Paternity Leave', 'Paid paternity leave', 7),
('00000000-0000-0000-0000-000000000001', 'Marriage Leave', 'Leave for own marriage', 3),
('00000000-0000-0000-0000-000000000001', 'Unpaid Leave', 'Unpaid leave', 365);

-- Leave balances for all active employees
INSERT INTO leave_balances (employee_id, leave_type_id, year, entitled_days)
SELECT e.id, lt.id, 2026, lt.default_days
FROM employees e
CROSS JOIN leave_types lt
WHERE e.company_id = '00000000-0000-0000-0000-000000000001'
AND lt.company_id = '00000000-0000-0000-0000-000000000001'
AND e.is_active = TRUE
ON CONFLICT (employee_id, leave_type_id, year) DO NOTHING;

-- Working day config (Mon-Fri working, Sat-Sun off)
INSERT INTO working_day_config (company_id, day_of_week, is_working_day)
SELECT c.id, d.day, d.is_working
FROM companies c
CROSS JOIN (VALUES (0, FALSE), (1, TRUE), (2, TRUE), (3, TRUE), (4, TRUE), (5, TRUE), (6, FALSE)) AS d(day, is_working)
ON CONFLICT DO NOTHING;

-- Malaysian 2026 public holidays
INSERT INTO holidays (company_id, name, date, holiday_type, is_recurring, description)
SELECT c.id, h.name, h.date::DATE, 'public_holiday', h.recurring, h.description
FROM companies c
CROSS JOIN (VALUES
    ('New Year''s Day', '2026-01-01', TRUE, 'New Year'),
    ('Thaipusam', '2026-01-25', FALSE, 'Hindu festival'),
    ('Nuzul Al-Quran', '2026-02-17', FALSE, 'Revelation of the Quran'),
    ('Labour Day', '2026-05-01', TRUE, 'International Workers Day'),
    ('Vesak Day', '2026-05-12', FALSE, 'Buddhist festival'),
    ('Hari Raya Aidilfitri', '2026-03-20', FALSE, 'End of Ramadan'),
    ('Hari Raya Aidilfitri (2nd Day)', '2026-03-21', FALSE, 'End of Ramadan'),
    ('Yang di-Pertuan Agong Birthday', '2026-06-01', FALSE, 'King''s birthday'),
    ('Hari Raya Haji', '2026-05-27', FALSE, 'Feast of Sacrifice'),
    ('Hari Raya Haji (2nd Day)', '2026-05-28', FALSE, 'Feast of Sacrifice'),
    ('Awal Muharram', '2026-06-17', FALSE, 'Islamic New Year'),
    ('Malaysia Day', '2026-09-16', TRUE, 'Formation of Malaysia'),
    ('Mawlid Nabi', '2026-08-26', FALSE, 'Prophet Muhammad''s Birthday'),
    ('Deepavali', '2026-10-20', FALSE, 'Festival of Lights'),
    ('Christmas Day', '2026-12-25', TRUE, 'Christmas'),
    ('Merdeka Day', '2026-08-31', TRUE, 'Independence Day')
) AS h(name, date, recurring, description)
ON CONFLICT DO NOTHING;

-- source: 004_seed_teams_extra_employees.sql

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
INSERT INTO users (id, email, password_hash, full_name, roles, company_id, employee_id) VALUES
('00000000-0000-0000-0000-000000000111', 'chee.keong@demo.com', '$2b$12$fRjUOo/hbE.JoyBkrDmmg.0nkGxVzL6dQXJuUHZ2dgDBZcdvX3yA2', 'Lim Chee Keong', ARRAY['employee']::varchar(50)[], '00000000-0000-0000-0000-000000000001', 'a0000000-0000-0000-0000-000000000011'),
('00000000-0000-0000-0000-000000000112', 'kavitha.s@demo.com', '$2b$12$fRjUOo/hbE.JoyBkrDmmg.0nkGxVzL6dQXJuUHZ2dgDBZcdvX3yA2', 'Kavitha Subramaniam', ARRAY['employee']::varchar(50)[], '00000000-0000-0000-0000-000000000001', 'a0000000-0000-0000-0000-000000000012'),
('00000000-0000-0000-0000-000000000113', 'azman.yusof@demo.com', '$2b$12$fRjUOo/hbE.JoyBkrDmmg.0nkGxVzL6dQXJuUHZ2dgDBZcdvX3yA2', 'Azman Yusof', ARRAY['employee']::varchar(50)[], '00000000-0000-0000-0000-000000000001', 'a0000000-0000-0000-0000-000000000013'),
('00000000-0000-0000-0000-000000000114', 'siew.mei@demo.com', '$2b$12$fRjUOo/hbE.JoyBkrDmmg.0nkGxVzL6dQXJuUHZ2dgDBZcdvX3yA2', 'Ng Siew Mei', ARRAY['employee']::varchar(50)[], '00000000-0000-0000-0000-000000000001', 'a0000000-0000-0000-0000-000000000014'),
('00000000-0000-0000-0000-000000000115', 'hafiz.rahman@demo.com', '$2b$12$fRjUOo/hbE.JoyBkrDmmg.0nkGxVzL6dQXJuUHZ2dgDBZcdvX3yA2', 'Hafiz Rahman', ARRAY['employee']::varchar(50)[], '00000000-0000-0000-0000-000000000001', 'a0000000-0000-0000-0000-000000000015'),
('00000000-0000-0000-0000-000000000116', 'yee.ling@demo.com', '$2b$12$fRjUOo/hbE.JoyBkrDmmg.0nkGxVzL6dQXJuUHZ2dgDBZcdvX3yA2', 'Chen Yee Ling', ARRAY['employee']::varchar(50)[], '00000000-0000-0000-0000-000000000001', 'a0000000-0000-0000-0000-000000000016'),
('00000000-0000-0000-0000-000000000117', 'ravi.krishnan@demo.com', '$2b$12$fRjUOo/hbE.JoyBkrDmmg.0nkGxVzL6dQXJuUHZ2dgDBZcdvX3yA2', 'Ravi Krishnan', ARRAY['employee']::varchar(50)[], '00000000-0000-0000-0000-000000000001', 'a0000000-0000-0000-0000-000000000017'),
('00000000-0000-0000-0000-000000000118', 'zainab.osman@demo.com', '$2b$12$fRjUOo/hbE.JoyBkrDmmg.0nkGxVzL6dQXJuUHZ2dgDBZcdvX3yA2', 'Zainab Osman', ARRAY['employee']::varchar(50)[], '00000000-0000-0000-0000-000000000001', 'a0000000-0000-0000-0000-000000000018'),
('00000000-0000-0000-0000-000000000119', 'daniel.tan@demo.com', '$2b$12$fRjUOo/hbE.JoyBkrDmmg.0nkGxVzL6dQXJuUHZ2dgDBZcdvX3yA2', 'Daniel Tan', ARRAY['employee']::varchar(50)[], '00000000-0000-0000-0000-000000000001', 'a0000000-0000-0000-0000-000000000019'),
('00000000-0000-0000-0000-000000000120', 'farah.aziz@demo.com', '$2b$12$fRjUOo/hbE.JoyBkrDmmg.0nkGxVzL6dQXJuUHZ2dgDBZcdvX3yA2', 'Farah Aziz', ARRAY['employee']::varchar(50)[], '00000000-0000-0000-0000-000000000001', 'a0000000-0000-0000-0000-000000000020')
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

-- source: 005_seed_male_employee_user.sql

-- Male employee portal user (ahmad@demo.com) linked to EMP001
INSERT INTO users (id, email, password_hash, full_name, roles, company_id, employee_id)
VALUES (
    '00000000-0000-0000-0000-000000000011',
    'ahmad@demo.com',
    '$2b$12$fRjUOo/hbE.JoyBkrDmmg.0nkGxVzL6dQXJuUHZ2dgDBZcdvX3yA2',
    'Ahmad bin Razak',
    ARRAY['employee']::varchar(50)[],
    '00000000-0000-0000-0000-000000000001',
    'a0000000-0000-0000-0000-000000000001'
) ON CONFLICT (id) DO NOTHING;

INSERT INTO user_companies (user_id, company_id)
VALUES ('00000000-0000-0000-0000-000000000011', '00000000-0000-0000-0000-000000000001')
ON CONFLICT DO NOTHING;

-- source: 006_seed_default_attendance_method.sql

-- Seed default attendance method
INSERT INTO platform_settings (key, value) VALUES
    ('attendance_method', 'qr_code'),
    ('allow_company_override', 'false')
ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value;

-- source: 007_seed_company_work_schedules.sql

-- Seed a default work schedule for every existing company
INSERT INTO company_work_schedules (company_id, name, start_time, end_time, grace_minutes, timezone)
SELECT id, 'Default', '09:00', '18:00', 15, 'Asia/Kuala_Lumpur'
FROM companies
ON CONFLICT DO NOTHING;

-- source: 023_fix_pcb_cumulative_tax.sql
--
-- The pcb_brackets cumulative_tax values inserted above carry the original
-- (mis-scaled, 100x-too-large) figures; calculate_tax_from_brackets adds
-- cumulative_tax directly to a sen-denominated bracket_tax, so these must be
-- in sen. Correct the 2024 brackets to their proper cumulative tax in sen.
UPDATE pcb_brackets SET cumulative_tax = 15000       WHERE effective_year = 2024 AND chargeable_income_from = 2000001;
UPDATE pcb_brackets SET cumulative_tax = 60000       WHERE effective_year = 2024 AND chargeable_income_from = 3500001;
UPDATE pcb_brackets SET cumulative_tax = 150000      WHERE effective_year = 2024 AND chargeable_income_from = 5000001;
UPDATE pcb_brackets SET cumulative_tax = 370000      WHERE effective_year = 2024 AND chargeable_income_from = 7000001;
UPDATE pcb_brackets SET cumulative_tax = 940000      WHERE effective_year = 2024 AND chargeable_income_from = 10000001;
UPDATE pcb_brackets SET cumulative_tax = 8440000     WHERE effective_year = 2024 AND chargeable_income_from = 40000001;
UPDATE pcb_brackets SET cumulative_tax = 13640000    WHERE effective_year = 2024 AND chargeable_income_from = 60000001;
UPDATE pcb_brackets SET cumulative_tax = 52840000    WHERE effective_year = 2024 AND chargeable_income_from = 200000001;

