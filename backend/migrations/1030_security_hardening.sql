-- Security audit hardening migration:
-- 1. Upload ownership registry (`uploaded_files`) + scheme CHECK backstops.
-- 2. TOTP anti-replay (`last_used_step`) & brute-force lockout columns (`failed_attempts`, `locked_until`).
-- 3. Payroll maker-checker attribution (`submitted_by`) & missing user FKs.
-- 4. Platform-only permission prohibition on company-scoped user groups.
-- 5. Tenant-consistent composite foreign keys on `email_logs`.
-- 6. Immutable `company_id` triggers across remaining tenant-scoped tables.
-- 7. Automatic cleanup of company-scoped group memberships and notifications when
--    a `user_companies` membership is revoked.
-- 8. At-rest SHA-256 hashing of existing `attendance_qr_tokens.token` secrets.

-- ─── 1. Upload ownership registry & URL scheme guardrails ───

CREATE TABLE IF NOT EXISTS public.uploaded_files (
    stored_name character varying(200) PRIMARY KEY,
    company_id uuid NOT NULL REFERENCES public.companies(id) ON DELETE CASCADE,
    uploaded_by uuid REFERENCES public.users(id) ON DELETE SET NULL,
    created_at timestamp with time zone NOT NULL DEFAULT now(),
    CONSTRAINT uploaded_files_stored_name_safe_chk CHECK (
        stored_name ~ '^[^/\\:]{1,200}$'
        AND stored_name NOT LIKE '%..%'
    )
);

CREATE INDEX IF NOT EXISTS idx_uploaded_files_company
    ON public.uploaded_files (company_id);

-- Backfill existing managed local upload paths so pre-existing records carry
-- explicit tenant ownership alongside newly uploaded files.
INSERT INTO public.uploaded_files (stored_name, company_id)
SELECT substring(file_url FROM 14)::character varying(200), company_id
  FROM public.documents
 WHERE file_url LIKE '/api/uploads/%'
   AND substring(file_url FROM 14) ~ '^[^/\\:]{1,200}$'
   AND substring(file_url FROM 14) NOT LIKE '%..%'
ON CONFLICT (stored_name) DO NOTHING;

INSERT INTO public.uploaded_files (stored_name, company_id)
SELECT substring(receipt_url FROM 14)::character varying(200), company_id
  FROM public.claims
 WHERE receipt_url LIKE '/api/uploads/%'
   AND substring(receipt_url FROM 14) ~ '^[^/\\:]{1,200}$'
   AND substring(receipt_url FROM 14) NOT LIKE '%..%'
ON CONFLICT (stored_name) DO NOTHING;

INSERT INTO public.uploaded_files (stored_name, company_id)
SELECT substring(attachment_url FROM 14)::character varying(200), company_id
  FROM public.leave_requests
 WHERE attachment_url LIKE '/api/uploads/%'
   AND substring(attachment_url FROM 14) ~ '^[^/\\:]{1,200}$'
   AND substring(attachment_url FROM 14) NOT LIKE '%..%'
ON CONFLICT (stored_name) DO NOTHING;

INSERT INTO public.uploaded_files (stored_name, company_id)
SELECT substring(logo_url FROM 14)::character varying(200), id
  FROM public.companies
 WHERE logo_url LIKE '/api/uploads/%'
   AND substring(logo_url FROM 14) ~ '^[^/\\:]{1,200}$'
   AND substring(logo_url FROM 14) NOT LIKE '%..%'
ON CONFLICT (stored_name) DO NOTHING;

-- Scrub any legacy nullable URLs that violate the managed-upload / http(s) rule
-- before attaching scheme constraints (leaving *_no_traversal to govern the
-- path-segment check for `/api/uploads/...`).
UPDATE public.claims
   SET receipt_url = NULL
 WHERE receipt_url IS NOT NULL
   AND receipt_url NOT LIKE '/api/uploads/%'
   AND receipt_url !~* '^https?://';

UPDATE public.leave_requests
   SET attachment_url = NULL
 WHERE attachment_url IS NOT NULL
   AND attachment_url NOT LIKE '/api/uploads/%'
   AND attachment_url !~* '^https?://';

DO $url_scheme_constraints$
BEGIN
    ALTER TABLE public.documents
        DROP CONSTRAINT IF EXISTS documents_file_url_scheme_chk;
    ALTER TABLE public.documents
        ADD CONSTRAINT documents_file_url_scheme_chk CHECK (
            file_url = ''
            OR file_url LIKE '/api/uploads/%'
            OR file_url ~* '^https?://'
        ) NOT VALID;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
         WHERE conname = 'claims_receipt_url_scheme_chk'
           AND conrelid = 'public.claims'::regclass
    ) THEN
        ALTER TABLE public.claims
            ADD CONSTRAINT claims_receipt_url_scheme_chk CHECK (
                receipt_url IS NULL
                OR receipt_url LIKE '/api/uploads/%'
                OR receipt_url ~* '^https?://'
            ) NOT VALID;
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
         WHERE conname = 'leave_requests_attachment_url_scheme_chk'
           AND conrelid = 'public.leave_requests'::regclass
    ) THEN
        ALTER TABLE public.leave_requests
            ADD CONSTRAINT leave_requests_attachment_url_scheme_chk CHECK (
                attachment_url IS NULL
                OR attachment_url LIKE '/api/uploads/%'
                OR attachment_url ~* '^https?://'
            ) NOT VALID;
    END IF;
END
$url_scheme_constraints$;

-- ─── 2. TOTP replay protection and brute-force lockout ───

ALTER TABLE public.user_totp
    ADD COLUMN IF NOT EXISTS last_used_step bigint,
    ADD COLUMN IF NOT EXISTS failed_attempts integer NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS locked_until timestamp with time zone;

-- ─── 3. Payroll maker-checker attribution & missing user FKs ───

ALTER TABLE public.payroll_runs
    ADD COLUMN IF NOT EXISTS submitted_by uuid REFERENCES public.users(id) ON DELETE SET NULL;

DO $audit_actor_fks$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
         WHERE conname = 'payroll_runs_cancelled_by_fkey'
           AND conrelid = 'public.payroll_runs'::regclass
    ) THEN
        ALTER TABLE public.payroll_runs
            ADD CONSTRAINT payroll_runs_cancelled_by_fkey
            FOREIGN KEY (cancelled_by) REFERENCES public.users(id) ON DELETE SET NULL NOT VALID;
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
         WHERE conname = 'salary_history_approved_by_fkey'
           AND conrelid = 'public.salary_history'::regclass
    ) THEN
        ALTER TABLE public.salary_history
            ADD CONSTRAINT salary_history_approved_by_fkey
            FOREIGN KEY (approved_by) REFERENCES public.users(id) ON DELETE SET NULL NOT VALID;
    END IF;
END
$audit_actor_fks$;

-- ─── 4. Prevent platform-only permissions inside tenant user groups ───

DELETE FROM public.user_group_permissions
 WHERE permission IN (
     'manage_companies',
     'manage_platform_settings',
     'manage_backups',
     'manage_users'
 );

DO $group_perm_checks$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
         WHERE conname = 'user_group_permissions_no_platform_only_check'
           AND conrelid = 'public.user_group_permissions'::regclass
    ) THEN
        ALTER TABLE public.user_group_permissions
            ADD CONSTRAINT user_group_permissions_no_platform_only_check
            CHECK (permission NOT IN (
                'manage_companies',
                'manage_platform_settings',
                'manage_backups',
                'manage_users'
            ));
    END IF;
END
$group_perm_checks$;

-- ─── 5. Tenant-consistent composite foreign keys on `email_logs` ───

CREATE UNIQUE INDEX IF NOT EXISTS email_templates_id_company_key
    ON public.email_templates (id, company_id);

UPDATE public.email_logs el
   SET employee_id = NULL
 WHERE el.employee_id IS NOT NULL
   AND NOT EXISTS (
       SELECT 1 FROM public.employees e
        WHERE e.id = el.employee_id
          AND e.company_id = el.company_id
   );

UPDATE public.email_logs el
   SET template_id = NULL
 WHERE el.template_id IS NOT NULL
   AND NOT EXISTS (
       SELECT 1 FROM public.email_templates et
        WHERE et.id = el.template_id
          AND et.company_id = el.company_id
   );

DO $email_logs_tenant_fks$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
         WHERE conname = 'email_logs_employee_tenant_fkey'
           AND conrelid = 'public.email_logs'::regclass
    ) THEN
        ALTER TABLE public.email_logs
            ADD CONSTRAINT email_logs_employee_tenant_fkey
            FOREIGN KEY (employee_id, company_id)
            REFERENCES public.employees(id, company_id)
            ON DELETE SET NULL (employee_id)
            NOT VALID;
        ALTER TABLE public.email_logs VALIDATE CONSTRAINT email_logs_employee_tenant_fkey;
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
         WHERE conname = 'email_logs_template_tenant_fkey'
           AND conrelid = 'public.email_logs'::regclass
    ) THEN
        ALTER TABLE public.email_logs
            ADD CONSTRAINT email_logs_template_tenant_fkey
            FOREIGN KEY (template_id, company_id)
            REFERENCES public.email_templates(id, company_id)
            ON DELETE SET NULL (template_id)
            NOT VALID;
        ALTER TABLE public.email_logs VALIDATE CONSTRAINT email_logs_template_tenant_fkey;
    END IF;
END
$email_logs_tenant_fks$;

ALTER TABLE public.email_logs DROP CONSTRAINT IF EXISTS email_logs_employee_id_fkey;
ALTER TABLE public.email_logs DROP CONSTRAINT IF EXISTS email_logs_template_id_fkey;

-- ─── 6. Immutable company_id triggers on tenant-owned tables ───

DROP TRIGGER IF EXISTS documents_company_immutable_trigger ON public.documents;
CREATE TRIGGER documents_company_immutable_trigger
    BEFORE UPDATE OF company_id ON public.documents
    FOR EACH ROW EXECUTE FUNCTION public.enforce_immutable_company_id();

DROP TRIGGER IF EXISTS claims_company_immutable_trigger ON public.claims;
CREATE TRIGGER claims_company_immutable_trigger
    BEFORE UPDATE OF company_id ON public.claims
    FOR EACH ROW EXECUTE FUNCTION public.enforce_immutable_company_id();

DROP TRIGGER IF EXISTS leave_requests_company_immutable_trigger ON public.leave_requests;
CREATE TRIGGER leave_requests_company_immutable_trigger
    BEFORE UPDATE OF company_id ON public.leave_requests
    FOR EACH ROW EXECUTE FUNCTION public.enforce_immutable_company_id();

DROP TRIGGER IF EXISTS overtime_applications_company_immutable_trigger ON public.overtime_applications;
CREATE TRIGGER overtime_applications_company_immutable_trigger
    BEFORE UPDATE OF company_id ON public.overtime_applications
    FOR EACH ROW EXECUTE FUNCTION public.enforce_immutable_company_id();

DROP TRIGGER IF EXISTS attendance_records_company_immutable_trigger ON public.attendance_records;
CREATE TRIGGER attendance_records_company_immutable_trigger
    BEFORE UPDATE OF company_id ON public.attendance_records
    FOR EACH ROW EXECUTE FUNCTION public.enforce_immutable_company_id();

DROP TRIGGER IF EXISTS attendance_kiosk_credentials_company_immutable_trigger ON public.attendance_kiosk_credentials;
CREATE TRIGGER attendance_kiosk_credentials_company_immutable_trigger
    BEFORE UPDATE OF company_id ON public.attendance_kiosk_credentials
    FOR EACH ROW EXECUTE FUNCTION public.enforce_immutable_company_id();

DROP TRIGGER IF EXISTS company_networks_company_immutable_trigger ON public.company_networks;
CREATE TRIGGER company_networks_company_immutable_trigger
    BEFORE UPDATE OF company_id ON public.company_networks
    FOR EACH ROW EXECUTE FUNCTION public.enforce_immutable_company_id();

DROP TRIGGER IF EXISTS email_templates_company_immutable_trigger ON public.email_templates;
CREATE TRIGGER email_templates_company_immutable_trigger
    BEFORE UPDATE OF company_id ON public.email_templates
    FOR EACH ROW EXECUTE FUNCTION public.enforce_immutable_company_id();

DROP TRIGGER IF EXISTS email_logs_company_immutable_trigger ON public.email_logs;
CREATE TRIGGER email_logs_company_immutable_trigger
    BEFORE UPDATE OF company_id ON public.email_logs
    FOR EACH ROW EXECUTE FUNCTION public.enforce_immutable_company_id();

DROP TRIGGER IF EXISTS background_jobs_company_immutable_trigger ON public.background_jobs;
CREATE TRIGGER background_jobs_company_immutable_trigger
    BEFORE UPDATE OF company_id ON public.background_jobs
    FOR EACH ROW EXECUTE FUNCTION public.enforce_immutable_company_id();

DROP TRIGGER IF EXISTS bulk_import_sessions_company_immutable_trigger ON public.bulk_import_sessions;
CREATE TRIGGER bulk_import_sessions_company_immutable_trigger
    BEFORE UPDATE OF company_id ON public.bulk_import_sessions
    FOR EACH ROW EXECUTE FUNCTION public.enforce_immutable_company_id();

DROP TRIGGER IF EXISTS uploaded_files_company_immutable_trigger ON public.uploaded_files;
CREATE TRIGGER uploaded_files_company_immutable_trigger
    BEFORE UPDATE OF company_id ON public.uploaded_files
    FOR EACH ROW EXECUTE FUNCTION public.enforce_immutable_company_id();

-- ─── 7. Purge tenant-scoped memberships & notifications on company removal ───

CREATE OR REPLACE FUNCTION public.cleanup_user_company_dependents()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = public, pg_temp
AS $cleanup_user_company_dependents$
BEGIN
    DELETE FROM public.user_group_members ugm
     USING public.user_groups ug
     WHERE ugm.group_id = ug.id
       AND ugm.user_id = OLD.user_id
       AND ug.company_id = OLD.company_id;

    DELETE FROM public.notifications
     WHERE user_id = OLD.user_id
       AND company_id = OLD.company_id;

    RETURN OLD;
END
$cleanup_user_company_dependents$;

DROP TRIGGER IF EXISTS user_companies_cleanup_dependents_trigger ON public.user_companies;
CREATE TRIGGER user_companies_cleanup_dependents_trigger
    AFTER DELETE ON public.user_companies
    FOR EACH ROW EXECUTE FUNCTION public.cleanup_user_company_dependents();

-- ─── 8. Hash existing plaintext attendance QR tokens at rest ───

UPDATE public.attendance_qr_tokens
   SET token = encode(sha256(convert_to(token, 'UTF8')), 'hex')
 WHERE token !~ '^[0-9a-f]{64}$';
