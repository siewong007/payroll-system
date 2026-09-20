-- Relax the cancel-evidence constraint added by 1025, as a separate migration.
--
-- 1025 shipped requiring both cancelled_by and cancelled_at on cancelled runs.
-- Runs cancelled before it existed record neither: the constraint therefore
-- requires the timestamp only — the cancel and reverse endpoints always set
-- cancelled_by going forward. Legacy rows are timestamped from updated_at (the
-- write that last changed the status) rather than fabricating a canceller.

UPDATE public.payroll_runs
SET cancelled_at = updated_at
WHERE status = 'cancelled'::public.payroll_status AND cancelled_at IS NULL;

ALTER TABLE public.payroll_runs
    DROP CONSTRAINT payroll_runs_cancel_consistency_check;

ALTER TABLE public.payroll_runs
    ADD CONSTRAINT payroll_runs_cancel_consistency_check CHECK (
        (status <> 'cancelled'::public.payroll_status)
        OR (cancelled_at IS NOT NULL)
    );
