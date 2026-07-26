-- Index the columns that make an uploaded file reachable.
--
-- `GET /api/uploads/{filename}` is no longer served to anyone holding the URL.
-- Authorizing it means asking which record references the file, and an upload
-- has no owning table — the answer lives in three columns spread across three
-- tables (see `repositories::reads::uploads`). Unindexed, that lookup
-- sequential-scans claims, leave_requests and documents on *every* attachment
-- view: the preview thumbnails on the approvals workbench alone issue one
-- request per row on screen.
--
-- Each index leads with company_id because the lookup is always tenant-scoped,
-- and each is partial: a row with no attachment can never satisfy the equality
-- on the URL, so indexing it only pays storage. In practice most rows in all
-- three tables have no attachment at all, which keeps these small.

CREATE INDEX IF NOT EXISTS idx_claims_receipt_url
    ON public.claims USING btree (company_id, receipt_url)
    WHERE receipt_url IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_leave_requests_attachment_url
    ON public.leave_requests USING btree (company_id, attachment_url)
    WHERE attachment_url IS NOT NULL;

-- `documents.file_url` is NOT NULL, so the partial predicate here excludes
-- soft-deleted rows instead — matching the `deleted_at IS NULL` filter the
-- lookup applies.
CREATE INDEX IF NOT EXISTS idx_documents_file_url
    ON public.documents USING btree (company_id, file_url)
    WHERE deleted_at IS NULL;
