-- Add an explicit review state between calculation and approval.
ALTER TYPE payroll_status ADD VALUE IF NOT EXISTS 'pending_approval' AFTER 'processed';
