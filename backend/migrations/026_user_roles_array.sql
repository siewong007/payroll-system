ALTER TABLE users
ADD COLUMN roles VARCHAR(50)[] NOT NULL DEFAULT ARRAY['employee']::VARCHAR(50)[];

UPDATE users
SET roles = ARRAY[role]::VARCHAR(50)[];

ALTER TABLE users
ADD CONSTRAINT users_roles_valid CHECK (
    cardinality(roles) >= 1
    AND roles <@ ARRAY[
        'super_admin',
        'admin',
        'payroll_admin',
        'hr_manager',
        'finance',
        'exec',
        'employee'
    ]::VARCHAR(50)[]
);

CREATE INDEX idx_users_roles_gin ON users USING GIN (roles);
