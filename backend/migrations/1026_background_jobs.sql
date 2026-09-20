-- Background job tracking for work that outgrew the request lifecycle.
--
-- Payroll runs and bulk employee imports used to execute inside the HTTP
-- request that asked for them, bounded by the 30s TimeoutLayer: a large
-- payroll group or a first-tenant whole-headcount import could not finish in
-- time. Both now enqueue a row here and answer 202; a spawned task does the
-- work and writes status/progress/result back to the row for the client to
-- poll.
--
-- The row is also the crash record. The executor is an in-memory task, not a
-- daemon: a restart marks every stale 'pending'/'running' row 'failed' at
-- boot rather than leaving it spinning forever.
--
-- The partial unique index is the submit-time dedup for payroll runs: without
-- it two submissions of the same (company, group, period) would both enqueue
-- and race into the same payroll_runs claim, so the second is refused at
-- INSERT — mapped to a friendly 409 by known_constraint — instead of failing
-- minutes later inside the engine.

CREATE TABLE public.background_jobs (
    id uuid DEFAULT uuidv7() NOT NULL,
    company_id uuid NOT NULL,
    job_type character varying(50) NOT NULL,
    status text DEFAULT 'pending'::text NOT NULL,
    payload jsonb NOT NULL,
    progress_done integer DEFAULT 0 NOT NULL,
    progress_total integer DEFAULT 0 NOT NULL,
    result jsonb,
    error text,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    started_at timestamp with time zone,
    finished_at timestamp with time zone,
    CONSTRAINT background_jobs_status_check CHECK ((status = ANY (ARRAY['pending'::text, 'running'::text, 'succeeded'::text, 'failed'::text]))),
    CONSTRAINT background_jobs_progress_check CHECK (((progress_done >= 0) AND (progress_total >= 0) AND ((progress_total = 0) OR (progress_done <= progress_total))))
);

ALTER TABLE public.background_jobs
    ADD CONSTRAINT background_jobs_pkey PRIMARY KEY (id);

ALTER TABLE public.background_jobs
    ADD CONSTRAINT background_jobs_company_id_fkey FOREIGN KEY (company_id) REFERENCES public.companies(id);

ALTER TABLE public.background_jobs
    ADD CONSTRAINT background_jobs_created_by_fkey FOREIGN KEY (created_by) REFERENCES public.users(id);

CREATE INDEX background_jobs_company_created ON public.background_jobs USING btree (company_id, created_at DESC);

CREATE UNIQUE INDEX background_jobs_one_active_payroll_run
    ON public.background_jobs USING btree (
        company_id,
        ((payload ->> 'payroll_group_id'::text)),
        ((payload ->> 'period_year'::text)),
        ((payload ->> 'period_month'::text))
    )
    WHERE ((job_type)::text = 'payroll_run'::text) AND (status = ANY (ARRAY['pending'::text, 'running'::text]));
