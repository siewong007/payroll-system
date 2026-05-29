-- Adopt PostgreSQL 18's built-in uuidv7() for primary-key defaults.
--
-- uuidv7() produces time-ordered UUIDs (RFC 9562). Compared with the random
-- uuid_generate_v4() previously used, sequential keys keep newly inserted rows
-- clustered at the right edge of each primary-key B-tree, which reduces page
-- splits / WAL churn and improves index locality on insert-heavy tables.
--
-- REQUIRES PostgreSQL 18+ (uuidv7() is built in; no extension needed). This
-- migration will fail on older servers — that is intentional: the project
-- targets Postgres 18 from this migration onward.
--
-- Only the column DEFAULT changes. Existing v4 ids are left untouched, and
-- application code that supplies an explicit id keeps working unchanged.
-- Security tokens (refresh_tokens, password_reset_requests, oauth2_accounts)
-- are switched too for consistency, but their ids are generated explicitly in
-- application code, so the default is rarely exercised there.

ALTER TABLE attendance_kiosk_credentials ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE attendance_qr_tokens         ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE attendance_records           ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE audit_logs                   ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE claims                       ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE companies                    ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE company_locations            ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE company_settings             ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE company_work_schedules       ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE document_categories          ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE documents                    ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE eis_rates                    ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE employee_allowances          ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE employee_work_schedules      ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE employees                    ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE epf_rates                    ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE leave_balances               ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE leave_requests               ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE leave_types                  ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE notifications                ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE oauth2_accounts              ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE overtime_applications        ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE password_reset_requests      ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE payroll_entries              ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE payroll_groups               ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE payroll_item_details         ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE payroll_items                ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE payroll_runs                 ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE pcb_brackets                 ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE pcb_reliefs                  ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE refresh_tokens               ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE salary_history               ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE socso_rates                  ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE system_settings              ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE tp3_records                  ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE users                        ALTER COLUMN id SET DEFAULT uuidv7();
