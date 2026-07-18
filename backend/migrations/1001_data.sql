-- PostgreSQL 19 canonical data and idempotent legacy repairs.
--
-- This migration is intentionally free of demo companies, employees, PII and
-- login credentials. The inherited statutory rows below are retained only as
-- academic prototype fixtures: they are NOT verified Malaysian schedules and
-- automatic payroll refuses to use them until a complete source-derived rule
-- set is installed and marked verified in statutory_rule_sets.

INSERT INTO statutory_rule_sets (
    dataset_key, rule_code, effective_from, effective_to, status, verification_notes
) VALUES
    ('legacy-prototype-epf-2024', 'epf', '2024-01-01', '2024-12-31', 'prototype',
     'Legacy academic approximation; replace with complete official KWSP schedule.'),
    ('legacy-prototype-socso-2024', 'socso', '2024-01-01', '2024-12-31', 'prototype',
     'Legacy academic approximation; replace with complete effective-dated PERKESO Act 4 schedules.'),
    ('legacy-prototype-eis-2024', 'eis', '2024-01-01', '2024-12-31', 'prototype',
     'Legacy academic approximation; replace with complete effective-dated PERKESO Act 800 schedules.'),
    ('legacy-prototype-pcb-2024', 'pcb', '2024-01-01', '2024-12-31', 'prototype',
     'Legacy simplified calculator inputs; not LHDN MTD conformance data.')
ON CONFLICT (dataset_key) DO NOTHING;

-- Attach rows from the retired migration chain to the explicit prototype
-- registry entries before filling any missing rows. They remain unusable for
-- automatic payroll because the registry status is not verified.
UPDATE epf_rates rates
SET rule_set_id = rules.id
FROM statutory_rule_sets rules
WHERE rates.rule_set_id IS NULL
  AND rates.effective_from = DATE '2024-01-01'
  AND rules.dataset_key = 'legacy-prototype-epf-2024';

UPDATE socso_rates rates
SET rule_set_id = rules.id
FROM statutory_rule_sets rules
WHERE rates.rule_set_id IS NULL
  AND rates.effective_from = DATE '2024-01-01'
  AND rules.dataset_key = 'legacy-prototype-socso-2024';

UPDATE eis_rates rates
SET rule_set_id = rules.id
FROM statutory_rule_sets rules
WHERE rates.rule_set_id IS NULL
  AND rates.effective_from = DATE '2024-01-01'
  AND rules.dataset_key = 'legacy-prototype-eis-2024';

UPDATE pcb_brackets rates
SET rule_set_id = rules.id
FROM statutory_rule_sets rules
WHERE rates.rule_set_id IS NULL
  AND rates.effective_year = 2024
  AND rules.dataset_key = 'legacy-prototype-pcb-2024';

UPDATE pcb_reliefs rates
SET rule_set_id = rules.id
FROM statutory_rule_sets rules
WHERE rates.rule_set_id IS NULL
  AND rates.effective_year = 2024
  AND rules.dataset_key = 'legacy-prototype-pcb-2024';

-- Legacy prototype EPF lookup values (not an official Third Schedule).
WITH source_rows (
    wage_from, wage_to, employee_contribution, employer_contribution, category, effective_from
) AS (VALUES
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
(1900001, 2000000, 214500, 234000, 'A', '2024-01-01')
)
INSERT INTO epf_rates (
    wage_from, wage_to, employee_contribution, employer_contribution,
    category, effective_from, rule_set_id
)
SELECT source_rows.wage_from,
       source_rows.wage_to,
       source_rows.employee_contribution,
       source_rows.employer_contribution,
       source_rows.category,
       source_rows.effective_from::date,
       rules.id
FROM source_rows
CROSS JOIN statutory_rule_sets rules
WHERE rules.dataset_key = 'legacy-prototype-epf-2024'
ON CONFLICT (rule_set_id, category, effective_from, wage_from, wage_to) DO NOTHING;

-- Legacy prototype SOCSO lookup values (not an official Act 4 schedule).
WITH source_rows (
    wage_from, wage_to, first_cat_employee, first_cat_employer,
    second_cat_employer, effective_from
) AS (VALUES
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
(500001, 600000, 2175, 3985, 2415, '2024-01-01')
)
INSERT INTO socso_rates (
    wage_from, wage_to, first_cat_employee, first_cat_employer,
    second_cat_employer, effective_from, rule_set_id
)
SELECT source_rows.wage_from,
       source_rows.wage_to,
       source_rows.first_cat_employee,
       source_rows.first_cat_employer,
       source_rows.second_cat_employer,
       source_rows.effective_from::date,
       rules.id
FROM source_rows
CROSS JOIN statutory_rule_sets rules
WHERE rules.dataset_key = 'legacy-prototype-socso-2024'
ON CONFLICT (rule_set_id, effective_from, wage_from, wage_to) DO NOTHING;

-- Legacy prototype EIS lookup values (not an official Act 800 schedule).
WITH source_rows (
    wage_from, wage_to, employee_contribution, employer_contribution, effective_from
) AS (VALUES
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
(490001, 500000, 990, 990, '2024-01-01')
)
INSERT INTO eis_rates (
    wage_from, wage_to, employee_contribution, employer_contribution,
    effective_from, rule_set_id
)
SELECT source_rows.wage_from,
       source_rows.wage_to,
       source_rows.employee_contribution,
       source_rows.employer_contribution,
       source_rows.effective_from::date,
       rules.id
FROM source_rows
CROSS JOIN statutory_rule_sets rules
WHERE rules.dataset_key = 'legacy-prototype-eis-2024'
ON CONFLICT (rule_set_id, effective_from, wage_from, wage_to) DO NOTHING;

-- Prototype annual tax brackets used by the simplified academic PCB model.
WITH source_rows (
    chargeable_income_from, chargeable_income_to, tax_rate_percent,
    cumulative_tax, effective_year
) AS (VALUES
(0, 500000, 0, 0, 2024),
(500001, 2000000, 1, 0, 2024),
(2000001, 3500000, 3, 15000, 2024),
(3500001, 5000000, 6, 60000, 2024),
(5000001, 7000000, 11, 150000, 2024),
(7000001, 10000000, 19, 370000, 2024),
(10000001, 40000000, 25, 940000, 2024),
(40000001, 60000000, 26, 8440000, 2024),
(60000001, 200000000, 28, 13640000, 2024),
(200000001, 9999999999, 30, 52840000, 2024)
)
INSERT INTO pcb_brackets (
    chargeable_income_from, chargeable_income_to, tax_rate_percent,
    cumulative_tax, effective_year, rule_set_id
)
SELECT source_rows.*, rules.id
FROM source_rows
CROSS JOIN statutory_rule_sets rules
WHERE rules.dataset_key = 'legacy-prototype-pcb-2024'
ON CONFLICT (
    rule_set_id, effective_year, chargeable_income_from, chargeable_income_to
) DO NOTHING;

-- Prototype relief inputs used by the simplified academic PCB model.
WITH source_rows (relief_type, amount, effective_year, description) AS (VALUES
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
('tax_rebate_individual', 40000, 2024, 'Tax rebate RM400 if chargeable income <= RM35,000')
)
INSERT INTO pcb_reliefs (
    relief_type, amount, effective_year, description, rule_set_id
)
SELECT source_rows.*, rules.id
FROM source_rows
CROSS JOIN statutory_rule_sets rules
WHERE rules.dataset_key = 'legacy-prototype-pcb-2024'
ON CONFLICT (rule_set_id, effective_year, relief_type) DO NOTHING;

-- Production-safe platform defaults.
INSERT INTO platform_settings (key, value) VALUES
    ('attendance_method', 'qr_code'),
    ('allow_company_override', 'false')
ON CONFLICT (key) DO NOTHING;

-- The former demo seed was also the only source of essential tenant setup.
-- Provision safe, idempotent defaults for every real company without creating
-- demo people, credentials, holidays, or organization-specific records.
SELECT public.provision_company_defaults(id, NULL)
FROM companies;

-- Existing active employees need a balance row for each active leave type or
-- leave submission cannot reserve pending days. Preserve any existing balance.
INSERT INTO leave_balances (employee_id, leave_type_id, year, entitled_days)
SELECT
    e.id,
    lt.id,
    EXTRACT(YEAR FROM CURRENT_DATE)::integer,
    CASE
        WHEN EXTRACT(YEAR FROM e.date_joined) < EXTRACT(YEAR FROM CURRENT_DATE)
            THEN lt.default_days
        WHEN EXTRACT(YEAR FROM e.date_joined) = EXTRACT(YEAR FROM CURRENT_DATE)
            THEN ROUND(
                lt.default_days
                * (13 - EXTRACT(MONTH FROM e.date_joined))::numeric
                / 12
                * 2
            ) / 2
        ELSE 0
    END
FROM employees e
JOIN leave_types lt ON lt.company_id = e.company_id AND lt.is_active = true
WHERE e.deleted_at IS NULL
  AND e.is_active = true
ON CONFLICT (employee_id, leave_type_id, year) DO NOTHING;

-- One-time legacy repairs are idempotent and remain in the tracked data
-- migration instead of running as hidden startup mutations in Rust.
UPDATE attendance_records
SET check_out_at = check_in_at,
    updated_at = NOW()
WHERE status = 'absent'
  AND check_out_at IS NULL;

-- Older company backups may contain employees without portal accounts. Link
-- only unlinked employee-only users; never reassign a privileged account that
-- happens to share an employee email address.
WITH candidates AS (
    SELECT DISTINCT ON (lower(BTRIM(e.email)))
        lower(BTRIM(e.email)) AS email,
        e.full_name,
        e.company_id,
        e.id AS employee_id
    FROM employees e
    WHERE e.deleted_at IS NULL
      AND COALESCE(e.is_active, TRUE)
      AND e.email IS NOT NULL
      AND BTRIM(e.email) <> ''
      AND NOT EXISTS (
          SELECT 1
          FROM users linked
          WHERE linked.employee_id = e.id
            AND linked.deleted_at IS NULL
      )
    ORDER BY lower(BTRIM(e.email)), e.updated_at DESC, e.id DESC
), user_matches AS (
    SELECT DISTINCT ON (c.employee_id)
        u.id AS user_id,
        c.company_id,
        c.employee_id
    FROM candidates c
    JOIN users u ON lower(BTRIM(u.email)) = c.email
    WHERE u.employee_id IS NULL
      AND u.deleted_at IS NULL
      AND u.roles = ARRAY['employee']::VARCHAR(50)[]
    ORDER BY c.employee_id, u.created_at, u.id
)
UPDATE users u
SET employee_id = m.employee_id,
    company_id = m.company_id,
    updated_at = NOW()
FROM user_matches m
WHERE u.id = m.user_id;

WITH candidates AS (
    SELECT DISTINCT ON (lower(BTRIM(e.email)))
        lower(BTRIM(e.email)) AS email,
        e.full_name,
        e.company_id,
        e.id AS employee_id
    FROM employees e
    WHERE e.deleted_at IS NULL
      AND COALESCE(e.is_active, TRUE)
      AND e.email IS NOT NULL
      AND BTRIM(e.email) <> ''
      AND NOT EXISTS (
          SELECT 1
          FROM users linked
          WHERE linked.employee_id = e.id
            AND linked.deleted_at IS NULL
      )
    ORDER BY lower(BTRIM(e.email)), e.updated_at DESC, e.id DESC
)
INSERT INTO users (
    email,
    password_hash,
    full_name,
    roles,
    company_id,
    employee_id,
    must_change_password
)
SELECT
    c.email,
    '$2b$12$4KlNiC0qvbl15bM6tKnolOTcd0lWLSMNSDI2IUG9qZxA0MJQWNhze',
    c.full_name,
    ARRAY['employee']::VARCHAR(50)[],
    c.company_id,
    c.employee_id,
    TRUE
FROM candidates c
WHERE NOT EXISTS (
    SELECT 1 FROM users u WHERE lower(BTRIM(u.email)) = c.email
);

INSERT INTO user_companies (user_id, company_id)
SELECT u.id, u.company_id
FROM users u
WHERE u.company_id IS NOT NULL
  AND u.deleted_at IS NULL
ON CONFLICT DO NOTHING;

-- Employee accounts are single-company identities. Remove stale memberships
-- left behind when an old unlinked employee-only user was attached elsewhere.
DELETE FROM user_companies membership
USING users u
WHERE membership.user_id = u.id
  AND u.employee_id IS NOT NULL
  AND u.deleted_at IS NULL
  AND u.roles = ARRAY['employee']::VARCHAR(50)[]
  AND u.company_id IS NOT NULL
  AND membership.company_id <> u.company_id;
