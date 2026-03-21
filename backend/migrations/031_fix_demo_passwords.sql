-- Fix demo user passwords to match login screen prefill values
-- admin123 for admin/exec/sarah, employee123 for other employee accounts

-- admin@demo.com: admin123
UPDATE users SET password_hash = '$2b$12$fRjUOo/hbE.JoyBkrDmmg.0nkGxVzL6dQXJuUHZ2dgDBZcdvX3yA2'
WHERE email = 'admin@demo.com';

-- exec@demo.com: admin123
UPDATE users SET password_hash = '$2b$12$fRjUOo/hbE.JoyBkrDmmg.0nkGxVzL6dQXJuUHZ2dgDBZcdvX3yA2'
WHERE email = 'exec@demo.com';

-- sarah@demo.com: admin123
UPDATE users SET password_hash = '$2b$12$fRjUOo/hbE.JoyBkrDmmg.0nkGxVzL6dQXJuUHZ2dgDBZcdvX3yA2'
WHERE email = 'sarah@demo.com';
