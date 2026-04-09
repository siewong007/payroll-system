-- Male employee portal user (ahmad@demo.com) linked to EMP001
INSERT INTO users (id, email, password_hash, full_name, role, company_id, employee_id)
VALUES (
    '00000000-0000-0000-0000-000000000011',
    'ahmad@demo.com',
    '$2b$12$fRjUOo/hbE.JoyBkrDmmg.0nkGxVzL6dQXJuUHZ2dgDBZcdvX3yA2',
    'Ahmad bin Razak',
    'employee',
    '00000000-0000-0000-0000-000000000001',
    'a0000000-0000-0000-0000-000000000001'
) ON CONFLICT (id) DO NOTHING;

INSERT INTO user_companies (user_id, company_id)
VALUES ('00000000-0000-0000-0000-000000000011', '00000000-0000-0000-0000-000000000001')
ON CONFLICT DO NOTHING;
