-- Indexes backing the audit filter vocabulary endpoint and the filtered log
-- reads it makes reachable.
--
-- `audit_logs` is append-only and has no retention path, so every query over it
-- degrades monotonically. Today the table carries five indexes, none of which
-- can answer "which entity types / actions exist for this company":
--
--   idx_audit_logs_company_created  (company_id, created_at DESC)
--   idx_audit_logs_action           (action)
--   idx_audit_logs_date_range       (created_at DESC, entity_type)
--   idx_audit_logs_entity           (entity_type, entity_id)
--   idx_audit_logs_user             (user_id)
--
-- The column order below is (company_id, <facet>, created_at DESC) rather than
-- the narrower (company_id, <facet>). The third column is not decoration — it
-- is what stops this change from introducing a regression:
--
--   * The facet enumeration is a recursive loose index scan skipping from one
--     distinct value to the next. It needs only the first two columns, and is
--     served as an index-only scan either way.
--
--   * `reads::audit::list_filtered` filters on company_id + one facet and then
--     does ORDER BY created_at DESC LIMIT n. With only two columns the planner
--     prefers the already-ordered idx_audit_logs_company_created and discards
--     every non-matching row to fill one page — measured at 5M rows for a
--     company: ~546 ms and ~1 GB of buffers for a selective action, throwing
--     away 3.08M rows. With created_at in the index the same query is an
--     ordered index scan that stops after the page: ~0.09 ms, 29 buffers.
--
-- That second path is the one this feature *creates demand for*. The UI
-- currently offers twelve hardcoded actions; the endpoint exposes all 31 that
-- are actually written, so filters that were previously unreachable become one
-- click away. Adding the dropdown without the third column would ship a menu of
-- options that are individually slow.
--
-- Cost of the choice: roughly 300-370 MB per index at 6M rows versus ~42 MB for
-- the two-column form. That is the price of the filtered reads being indexed at
-- all, not a premium for the same benefit.
--
-- These are plain CREATE INDEX statements, not CONCURRENTLY: sqlx runs each
-- migration inside a transaction and CONCURRENTLY cannot run there. The build
-- takes ACCESS EXCLUSIVE on audit_logs, blocking audited writes for its
-- duration (measured 4-6 s at 6M rows on NVMe; proportionally worse on the
-- Lightsail host's slower storage). If this table has grown large enough for
-- that pause to matter, build the indexes by hand with CONCURRENTLY outside a
-- transaction first — `CREATE INDEX IF NOT EXISTS` then makes this file a
-- no-op, and a hand-built index that fails leaves an INVALID index that this
-- migration will NOT repair, so check `pg_index.indisvalid` afterwards.

CREATE INDEX IF NOT EXISTS idx_audit_logs_company_entity_created
    ON public.audit_logs USING btree (company_id, entity_type, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_audit_logs_company_action_created
    ON public.audit_logs USING btree (company_id, action, created_at DESC);
