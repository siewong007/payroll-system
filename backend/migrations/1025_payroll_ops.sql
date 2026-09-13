-- Payroll operations surface: run cancellation evidence + salary-change metadata.
--
-- `payroll_runs` gained a `cancelled` status value in the original schema and
-- the one-active-period index already exempts it, but no transition could reach
-- it — a run that was wrong after approval had no supported recovery. The
-- cancel endpoint keeps the row (it is the audit record of what was computed)
-- while these columns carry who cancelled it and why. `paid` stays terminal:
-- money already moved, so recovery there is a corrective run, not a status
-- change.
--
-- `salary_history` recorded old/new/date with no classification, so the audit
-- view could not tell an annual increment from a correction without parsing the
-- free-text reason. `change_type` is the controlled vocabulary; `approved_*`
-- columns are NULL for directly-applied changes today and are the anchor for a
-- future approval workflow — they exist now so history rows written from day
-- one can distinguish "self-approved" edits from workflow-approved ones.

ALTER TABLE public.payroll_runs
    ADD COLUMN cancelled_by uuid,
    ADD COLUMN cancelled_at timestamp with time zone,
    ADD COLUMN cancel_reason character varying(500);

-- Runs cancelled before this migration recorded no actor. Timestamp them from
-- updated_at (the write that last changed the status) rather than fabricating a
-- canceller. The constraint therefore requires the timestamp only — the cancel
-- and reverse endpoints always set cancelled_by too.
UPDATE public.payroll_runs
SET cancelled_at = updated_at
WHERE status = 'cancelled'::public.payroll_status AND cancelled_at IS NULL;

ALTER TABLE public.payroll_runs
    ADD CONSTRAINT payroll_runs_cancel_consistency_check CHECK (
        (status <> 'cancelled'::public.payroll_status)
        OR (cancelled_at IS NOT NULL)
    );

ALTER TABLE public.salary_history
    ADD COLUMN change_type character varying(30) NOT NULL DEFAULT 'adjustment',
    ADD COLUMN notes character varying(500),
    ADD COLUMN approved_by uuid,
    ADD COLUMN approved_at timestamp with time zone;

ALTER TABLE public.salary_history
    ADD CONSTRAINT salary_history_change_type_check CHECK (
        (change_type)::text = ANY (ARRAY[
            'new_hire'::character varying,
            'increment'::character varying,
            'promotion'::character varying,
            'demotion'::character varying,
            'contract_change'::character varying,
            'correction'::character varying,
            'backdated_adjustment'::character varying,
            'termination'::character varying,
            'adjustment'::character varying
        ]::text[])
    );

-- An imported/new employee's first row is a hiring salary, not a change.
UPDATE public.salary_history
SET change_type = 'new_hire'
WHERE old_salary = 0 AND change_type = 'adjustment';

ALTER TABLE public.salary_history
    ADD CONSTRAINT salary_history_approval_pair_check CHECK (
        (approved_at IS NULL) = (approved_by IS NULL)
    );
