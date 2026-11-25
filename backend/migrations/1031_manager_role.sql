-- Adds the `manager` role: every company-management capability except payroll.
--
-- Capabilities live in `backend/src/core/permission.rs` (MANAGER), not here;
-- this migration only widens the role CHECK so the value can be stored. A
-- manager is also never shown users holding `admin` or `super_admin` — that is
-- enforced in the user-directory reads, not by the schema.

ALTER TABLE users DROP CONSTRAINT users_roles_valid;

ALTER TABLE users ADD CONSTRAINT users_roles_valid CHECK (
    cardinality(roles) >= 1
    AND roles <@ ARRAY[
        'super_admin'::varchar(50),
        'admin'::varchar(50),
        'manager'::varchar(50),
        'payroll_admin'::varchar(50),
        'hr_manager'::varchar(50),
        'finance'::varchar(50),
        'exec'::varchar(50),
        'employee'::varchar(50)
    ]
);
