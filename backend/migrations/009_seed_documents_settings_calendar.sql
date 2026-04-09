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
