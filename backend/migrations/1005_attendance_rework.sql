-- Attendance rework: supporting index for QR-token purge, and audit backfill.
--
-- 1. attendance_records.qr_token_id has an FK to attendance_qr_tokens but no
--    supporting index, so deleting old tokens paid a sequential scan of
--    attendance_records per deleted row. The daily cleanup task now purges
--    expired, unreferenced tokens; this partial index makes both the FK check
--    and the purge's NOT EXISTS probe cheap.
CREATE INDEX IF NOT EXISTS idx_attendance_qr_token_ref
    ON attendance_records (qr_token_id)
    WHERE qr_token_id IS NOT NULL;

-- 2. Attendance-correction audit rows were written without company_id, which
--    makes them invisible to the company-scoped audit listing. Backfill from
--    the corrected record's company so historical corrections become visible.
--    (New corrections are written with company_id by the application.)
UPDATE audit_logs al
SET company_id = ar.company_id
FROM attendance_records ar
WHERE al.entity_type = 'attendance_record'
  AND al.action = 'update'
  AND al.company_id IS NULL
  AND al.entity_id = ar.id;
