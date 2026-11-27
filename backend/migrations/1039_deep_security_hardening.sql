-- Deep Second-Pass Security & Concurrency Hardening:
--   1. Append-only `editor_ids uuid[]` on `payroll_entries` so subsequent edits or
--      `revert_for_run` never erase earlier financial editors from four-eyes checks (D-H09).
--   2. Persist `file_size_bytes` on `uploaded_files` + composite lookup index to enforce
--      per-company and per-user upload storage quotas cleanly (D-H13).
--   3. Per-account step-up password confirmation failure tracking on `users` (D-H08).

ALTER TABLE public.payroll_entries
    ADD COLUMN IF NOT EXISTS editor_ids uuid[] NOT NULL DEFAULT '{}';

UPDATE public.payroll_entries pe
SET editor_ids = ARRAY(
    SELECT DISTINCT x
    FROM unnest(ARRAY[pe.created_by, pe.updated_by]) AS x
    WHERE x IS NOT NULL
    ORDER BY x
)
WHERE cardinality(pe.editor_ids) = 0
  AND (pe.created_by IS NOT NULL OR pe.updated_by IS NOT NULL);

ALTER TABLE public.uploaded_files
    ADD COLUMN IF NOT EXISTS file_size_bytes bigint NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_uploaded_files_company_uploader
    ON public.uploaded_files (company_id, uploaded_by);

ALTER TABLE public.users
    ADD COLUMN IF NOT EXISTS step_up_failed_attempts integer NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS step_up_locked_until timestamp with time zone;
