-- =====================================================
-- Seed Documents for Demo Company Employees
-- Company ID: 00000000-0000-0000-0000-000000000001
-- Admin User: 00000000-0000-0000-0000-000000000002
-- =====================================================

-- Use a DO block to look up category IDs dynamically
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

    -- =====================================================
    -- EMP001 - Ahmad bin Razak (Engineering Manager)
    -- =====================================================
    INSERT INTO documents (company_id, employee_id, category_id, title, description, file_name, file_url, file_size, mime_type, status, issue_date, is_confidential, created_by, updated_by)
    VALUES
    (v_company_id, 'a0000000-0000-0000-0000-000000000001', v_cat_ic,
     'MyKad Copy', 'IC copy - 850615-14-5231', 'ahmad_razak_ic.pdf', '/documents/emp001/ahmad_razak_ic.pdf',
     245000, 'application/pdf', 'active', '2020-02-25', TRUE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000001', v_cat_offer,
     'Offer Letter', 'Engineering Manager offer letter', 'ahmad_razak_offer.pdf', '/documents/emp001/ahmad_razak_offer.pdf',
     180000, 'application/pdf', 'active', '2020-02-15', FALSE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000001', v_cat_contract,
     'Employment Contract', 'Permanent employment contract', 'ahmad_razak_contract.pdf', '/documents/emp001/ahmad_razak_contract.pdf',
     320000, 'application/pdf', 'active', '2020-03-01', TRUE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000001', v_cat_tax,
     'EA Form 2024', 'Annual remuneration statement for YA2024', 'ahmad_razak_ea_2024.pdf', '/documents/emp001/ahmad_razak_ea_2024.pdf',
     150000, 'application/pdf', 'active', '2025-02-28', FALSE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000001', v_cat_cert,
     'PMP Certification', 'Project Management Professional certificate', 'ahmad_razak_pmp.pdf', '/documents/emp001/ahmad_razak_pmp.pdf',
     520000, 'application/pdf', 'active', '2022-05-10', FALSE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000001', v_cat_tax,
     'EA Form 2023', 'Annual remuneration statement for YA2023', 'ahmad_razak_ea_2023.pdf', '/documents/emp001/ahmad_razak_ea_2023.pdf',
     148000, 'application/pdf', 'active', '2024-02-28', FALSE, v_admin_id, v_admin_id);

    -- =====================================================
    -- EMP002 - Siti Nurhaliza binti Abdullah (Senior Accountant)
    -- =====================================================
    INSERT INTO documents (company_id, employee_id, category_id, title, description, file_name, file_url, file_size, mime_type, status, issue_date, is_confidential, created_by, updated_by)
    VALUES
    (v_company_id, 'a0000000-0000-0000-0000-000000000002', v_cat_ic,
     'MyKad Copy', 'IC copy - 880922-10-6754', 'siti_nurhaliza_ic.pdf', '/documents/emp002/siti_nurhaliza_ic.pdf',
     230000, 'application/pdf', 'active', '2019-07-01', TRUE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000002', v_cat_offer,
     'Offer Letter', 'Senior Accountant offer letter', 'siti_nurhaliza_offer.pdf', '/documents/emp002/siti_nurhaliza_offer.pdf',
     175000, 'application/pdf', 'active', '2019-06-20', FALSE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000002', v_cat_contract,
     'Employment Contract', 'Permanent employment contract', 'siti_nurhaliza_contract.pdf', '/documents/emp002/siti_nurhaliza_contract.pdf',
     310000, 'application/pdf', 'active', '2019-07-15', TRUE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000002', v_cat_tax,
     'EA Form 2024', 'Annual remuneration statement for YA2024', 'siti_nurhaliza_ea_2024.pdf', '/documents/emp002/siti_nurhaliza_ea_2024.pdf',
     145000, 'application/pdf', 'active', '2025-02-28', FALSE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000002', v_cat_cert,
     'ACCA Membership', 'Association of Chartered Certified Accountants', 'siti_nurhaliza_acca.pdf', '/documents/emp002/siti_nurhaliza_acca.pdf',
     480000, 'application/pdf', 'active', '2018-12-01', FALSE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000002', v_cat_cert,
     'MIA Registration', 'Malaysian Institute of Accountants membership', 'siti_nurhaliza_mia.pdf', '/documents/emp002/siti_nurhaliza_mia.pdf',
     290000, 'application/pdf', 'active', '2019-03-15', FALSE, v_admin_id, v_admin_id);

    -- =====================================================
    -- EMP003 - Lee Wei Ming (Software Developer)
    -- =====================================================
    INSERT INTO documents (company_id, employee_id, category_id, title, description, file_name, file_url, file_size, mime_type, status, issue_date, is_confidential, created_by, updated_by)
    VALUES
    (v_company_id, 'a0000000-0000-0000-0000-000000000003', v_cat_ic,
     'MyKad Copy', 'IC copy - 920310-08-4523', 'lee_wei_ming_ic.pdf', '/documents/emp003/lee_wei_ming_ic.pdf',
     240000, 'application/pdf', 'active', '2021-01-05', TRUE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000003', v_cat_offer,
     'Offer Letter', 'Software Developer offer letter', 'lee_wei_ming_offer.pdf', '/documents/emp003/lee_wei_ming_offer.pdf',
     170000, 'application/pdf', 'active', '2020-12-15', FALSE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000003', v_cat_contract,
     'Employment Contract', 'Permanent employment contract', 'lee_wei_ming_contract.pdf', '/documents/emp003/lee_wei_ming_contract.pdf',
     305000, 'application/pdf', 'active', '2021-01-10', TRUE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000003', v_cat_tax,
     'EA Form 2024', 'Annual remuneration statement for YA2024', 'lee_wei_ming_ea_2024.pdf', '/documents/emp003/lee_wei_ming_ea_2024.pdf',
     142000, 'application/pdf', 'active', '2025-02-28', FALSE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000003', v_cat_cert,
     'AWS Solutions Architect', 'AWS Certified Solutions Architect - Associate', 'lee_wei_ming_aws_sa.pdf', '/documents/emp003/lee_wei_ming_aws_sa.pdf',
     350000, 'application/pdf', 'active', '2023-08-20', FALSE, v_admin_id, v_admin_id);

    -- =====================================================
    -- EMP004 - Priya a/p Nair (HR Executive)
    -- =====================================================
    INSERT INTO documents (company_id, employee_id, category_id, title, description, file_name, file_url, file_size, mime_type, status, issue_date, is_confidential, created_by, updated_by)
    VALUES
    (v_company_id, 'a0000000-0000-0000-0000-000000000004', v_cat_ic,
     'MyKad Copy', 'IC copy - 900715-07-8812', 'priya_nair_ic.pdf', '/documents/emp004/priya_nair_ic.pdf',
     235000, 'application/pdf', 'active', '2022-03-20', TRUE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000004', v_cat_offer,
     'Offer Letter', 'HR Executive offer letter', 'priya_nair_offer.pdf', '/documents/emp004/priya_nair_offer.pdf',
     168000, 'application/pdf', 'active', '2022-03-15', FALSE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000004', v_cat_contract,
     'Employment Contract', 'Permanent employment contract', 'priya_nair_contract.pdf', '/documents/emp004/priya_nair_contract.pdf',
     298000, 'application/pdf', 'active', '2022-04-01', TRUE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000004', v_cat_tax,
     'EA Form 2024', 'Annual remuneration statement for YA2024', 'priya_nair_ea_2024.pdf', '/documents/emp004/priya_nair_ea_2024.pdf',
     140000, 'application/pdf', 'active', '2025-02-28', FALSE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000004', v_cat_cert,
     'SHRM-CP Certification', 'Society for Human Resource Management - Certified Professional', 'priya_nair_shrm.pdf', '/documents/emp004/priya_nair_shrm.pdf',
     410000, 'application/pdf', 'active', '2023-11-05', FALSE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000004', v_cat_medical,
     'Pre-Employment Medical Report', 'Medical check-up report', 'priya_nair_medical.pdf', '/documents/emp004/priya_nair_medical.pdf',
     890000, 'application/pdf', 'active', '2022-03-25', TRUE, v_admin_id, v_admin_id);

    -- =====================================================
    -- EMP005 - Muhammad Faiz bin Ismail (Junior Developer - Contract)
    -- =====================================================
    INSERT INTO documents (company_id, employee_id, category_id, title, description, file_name, file_url, file_size, mime_type, status, issue_date, is_confidential, created_by, updated_by)
    VALUES
    (v_company_id, 'a0000000-0000-0000-0000-000000000005', v_cat_ic,
     'MyKad Copy', 'IC copy - 970420-01-3345', 'faiz_ismail_ic.pdf', '/documents/emp005/faiz_ismail_ic.pdf',
     228000, 'application/pdf', 'active', '2024-05-20', TRUE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000005', v_cat_offer,
     'Offer Letter', 'Junior Developer contract offer', 'faiz_ismail_offer.pdf', '/documents/emp005/faiz_ismail_offer.pdf',
     165000, 'application/pdf', 'active', '2024-05-15', FALSE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000005', v_cat_contract,
     'Employment Contract', 'Fixed-term contract (12 months)', 'faiz_ismail_contract.pdf', '/documents/emp005/faiz_ismail_contract.pdf',
     340000, 'application/pdf', 'active', '2024-06-01', TRUE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000005', v_cat_other,
     'Degree Certificate', 'Bachelor of Computer Science - UTM', 'faiz_ismail_degree.pdf', '/documents/emp005/faiz_ismail_degree.pdf',
     560000, 'application/pdf', 'active', '2023-10-20', FALSE, v_admin_id, v_admin_id);

    -- =====================================================
    -- EMP006 - Tan Mei Ling (Marketing Manager)
    -- =====================================================
    INSERT INTO documents (company_id, employee_id, category_id, title, description, file_name, file_url, file_size, mime_type, status, issue_date, is_confidential, created_by, updated_by)
    VALUES
    (v_company_id, 'a0000000-0000-0000-0000-000000000006', v_cat_ic,
     'MyKad Copy', 'IC copy - 870812-10-2298', 'tan_mei_ling_ic.pdf', '/documents/emp006/tan_mei_ling_ic.pdf',
     242000, 'application/pdf', 'active', '2018-10-15', TRUE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000006', v_cat_offer,
     'Offer Letter', 'Marketing Manager offer letter', 'tan_mei_ling_offer.pdf', '/documents/emp006/tan_mei_ling_offer.pdf',
     182000, 'application/pdf', 'active', '2018-10-10', FALSE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000006', v_cat_contract,
     'Employment Contract', 'Permanent employment contract', 'tan_mei_ling_contract.pdf', '/documents/emp006/tan_mei_ling_contract.pdf',
     315000, 'application/pdf', 'active', '2018-11-01', TRUE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000006', v_cat_tax,
     'EA Form 2024', 'Annual remuneration statement for YA2024', 'tan_mei_ling_ea_2024.pdf', '/documents/emp006/tan_mei_ling_ea_2024.pdf',
     147000, 'application/pdf', 'active', '2025-02-28', FALSE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000006', v_cat_cert,
     'Google Ads Certification', 'Google Ads Search certification', 'tan_mei_ling_google_ads.pdf', '/documents/emp006/tan_mei_ling_google_ads.pdf',
     280000, 'application/pdf', 'active', '2024-07-15', FALSE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000006', v_cat_tax,
     'EA Form 2023', 'Annual remuneration statement for YA2023', 'tan_mei_ling_ea_2023.pdf', '/documents/emp006/tan_mei_ling_ea_2023.pdf',
     144000, 'application/pdf', 'active', '2024-02-28', FALSE, v_admin_id, v_admin_id);

    -- =====================================================
    -- EMP007 - Rajesh a/l Kumar (Operations Executive)
    -- =====================================================
    INSERT INTO documents (company_id, employee_id, category_id, title, description, file_name, file_url, file_size, mime_type, status, issue_date, is_confidential, created_by, updated_by)
    VALUES
    (v_company_id, 'a0000000-0000-0000-0000-000000000007', v_cat_ic,
     'MyKad Copy', 'IC copy - 910530-14-7761', 'rajesh_kumar_ic.pdf', '/documents/emp007/rajesh_kumar_ic.pdf',
     238000, 'application/pdf', 'active', '2023-02-01', TRUE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000007', v_cat_offer,
     'Offer Letter', 'Operations Executive offer letter', 'rajesh_kumar_offer.pdf', '/documents/emp007/rajesh_kumar_offer.pdf',
     172000, 'application/pdf', 'active', '2023-01-20', FALSE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000007', v_cat_contract,
     'Employment Contract', 'Permanent employment contract', 'rajesh_kumar_contract.pdf', '/documents/emp007/rajesh_kumar_contract.pdf',
     308000, 'application/pdf', 'active', '2023-02-15', TRUE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000007', v_cat_tax,
     'EA Form 2024', 'Annual remuneration statement for YA2024', 'rajesh_kumar_ea_2024.pdf', '/documents/emp007/rajesh_kumar_ea_2024.pdf',
     141000, 'application/pdf', 'active', '2025-02-28', FALSE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000007', v_cat_medical,
     'Pre-Employment Medical Report', 'Medical check-up report', 'rajesh_kumar_medical.pdf', '/documents/emp007/rajesh_kumar_medical.pdf',
     920000, 'application/pdf', 'active', '2023-02-10', TRUE, v_admin_id, v_admin_id);

    -- =====================================================
    -- EMP008 - Nurul Aisyah binti Hassan (Finance Assistant)
    -- =====================================================
    INSERT INTO documents (company_id, employee_id, category_id, title, description, file_name, file_url, file_size, mime_type, status, issue_date, is_confidential, created_by, updated_by)
    VALUES
    (v_company_id, 'a0000000-0000-0000-0000-000000000008', v_cat_ic,
     'MyKad Copy', 'IC copy - 950218-02-4456', 'nurul_aisyah_ic.pdf', '/documents/emp008/nurul_aisyah_ic.pdf',
     232000, 'application/pdf', 'active', '2023-08-20', TRUE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000008', v_cat_offer,
     'Offer Letter', 'Finance Assistant offer letter', 'nurul_aisyah_offer.pdf', '/documents/emp008/nurul_aisyah_offer.pdf',
     162000, 'application/pdf', 'active', '2023-08-15', FALSE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000008', v_cat_contract,
     'Employment Contract', 'Permanent employment contract', 'nurul_aisyah_contract.pdf', '/documents/emp008/nurul_aisyah_contract.pdf',
     295000, 'application/pdf', 'active', '2023-09-01', TRUE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000008', v_cat_tax,
     'EA Form 2024', 'Annual remuneration statement for YA2024', 'nurul_aisyah_ea_2024.pdf', '/documents/emp008/nurul_aisyah_ea_2024.pdf',
     138000, 'application/pdf', 'active', '2025-02-28', FALSE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000008', v_cat_other,
     'Diploma Certificate', 'Diploma in Accounting - Politeknik', 'nurul_aisyah_diploma.pdf', '/documents/emp008/nurul_aisyah_diploma.pdf',
     480000, 'application/pdf', 'active', '2017-09-15', FALSE, v_admin_id, v_admin_id);

    -- =====================================================
    -- EMP009 - Wong Jia Hao (QA Intern)
    -- =====================================================
    INSERT INTO documents (company_id, employee_id, category_id, title, description, file_name, file_url, file_size, mime_type, status, issue_date, is_confidential, created_by, updated_by)
    VALUES
    (v_company_id, 'a0000000-0000-0000-0000-000000000009', v_cat_ic,
     'MyKad Copy', 'IC copy - 000815-14-1122', 'wong_jia_hao_ic.pdf', '/documents/emp009/wong_jia_hao_ic.pdf',
     225000, 'application/pdf', 'active', '2025-09-20', TRUE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000009', v_cat_offer,
     'Internship Offer Letter', 'QA Intern offer letter (6-month internship)', 'wong_jia_hao_offer.pdf', '/documents/emp009/wong_jia_hao_offer.pdf',
     155000, 'application/pdf', 'active', '2025-09-10', FALSE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000009', v_cat_contract,
     'Internship Agreement', 'Industrial training agreement with university', 'wong_jia_hao_intern_agreement.pdf', '/documents/emp009/wong_jia_hao_intern_agreement.pdf',
     280000, 'application/pdf', 'active', '2025-10-01', FALSE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000009', v_cat_other,
     'University Endorsement Letter', 'Letter from university confirming internship placement', 'wong_jia_hao_uni_letter.pdf', '/documents/emp009/wong_jia_hao_uni_letter.pdf',
     190000, 'application/pdf', 'active', '2025-09-05', FALSE, v_admin_id, v_admin_id);

    -- =====================================================
    -- EMP010 - Amirah binti Zainal (Office Manager)
    -- =====================================================
    INSERT INTO documents (company_id, employee_id, category_id, title, description, file_name, file_url, file_size, mime_type, status, issue_date, is_confidential, created_by, updated_by)
    VALUES
    (v_company_id, 'a0000000-0000-0000-0000-000000000010', v_cat_ic,
     'MyKad Copy', 'IC copy - 930401-06-9988', 'amirah_zainal_ic.pdf', '/documents/emp010/amirah_zainal_ic.pdf',
     237000, 'application/pdf', 'active', '2021-08-01', TRUE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000010', v_cat_offer,
     'Offer Letter', 'Office Manager offer letter', 'amirah_zainal_offer.pdf', '/documents/emp010/amirah_zainal_offer.pdf',
     176000, 'application/pdf', 'active', '2021-07-25', FALSE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000010', v_cat_contract,
     'Employment Contract', 'Permanent employment contract', 'amirah_zainal_contract.pdf', '/documents/emp010/amirah_zainal_contract.pdf',
     302000, 'application/pdf', 'active', '2021-08-16', TRUE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000010', v_cat_tax,
     'EA Form 2024', 'Annual remuneration statement for YA2024', 'amirah_zainal_ea_2024.pdf', '/documents/emp010/amirah_zainal_ea_2024.pdf',
     139000, 'application/pdf', 'active', '2025-02-28', FALSE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000010', v_cat_medical,
     'Pre-Employment Medical Report', 'Medical check-up report', 'amirah_zainal_medical.pdf', '/documents/emp010/amirah_zainal_medical.pdf',
     875000, 'application/pdf', 'active', '2021-08-10', TRUE, v_admin_id, v_admin_id),

    (v_company_id, 'a0000000-0000-0000-0000-000000000010', v_cat_cert,
     'OSH Certification', 'Occupational Safety and Health awareness certificate', 'amirah_zainal_osh.pdf', '/documents/emp010/amirah_zainal_osh.pdf',
     310000, 'application/pdf', 'active', '2024-03-20', FALSE, v_admin_id, v_admin_id);

END $$;
