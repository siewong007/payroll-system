-- Fix existing soft-deleted employees so their employee_number no longer collides
UPDATE employees
SET employee_number = employee_number || '_DEL_' || id::text
WHERE deleted_at IS NOT NULL
  AND employee_number NOT LIKE '%\_DEL\_%';

-- Drop the old unconditional unique constraint
ALTER TABLE employees DROP CONSTRAINT IF EXISTS employees_company_id_employee_number_key;

-- Add a partial unique index that only enforces uniqueness for active (non-deleted) rows
CREATE UNIQUE INDEX employees_company_employee_number_active
ON employees (company_id, employee_number)
WHERE deleted_at IS NULL;
