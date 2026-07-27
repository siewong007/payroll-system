-- Concurrency guard for the auto-absent placeholder.
--
-- `attendance_records::mark_absent` inserted `WHERE NOT EXISTS (...)` with no
-- unique constraint behind it. Under READ COMMITTED two concurrent executions
-- both pass the NOT EXISTS and both insert a full set of placeholders, and
-- nothing in the schema stopped them: `attendance_one_open_per_employee` is
-- partial on `check_out_at IS NULL`, and a placeholder closes itself at its own
-- check-in, so it sits outside that index. Every other attendance index is
-- non-unique.
--
-- The callers make the race easy to reach rather than theoretical: the daily
-- catch-up runs per company per due date on the pool (each insert autocommits),
-- it also runs once at startup, and `POST /api/attendance/absent-run` has no
-- debounce — a double-clicked button is two concurrent requests. A restart
-- during the daily window runs the timer pass and the startup pass together.
--
-- Why the key is (employee_id, check_in_at) and NOT a local-day expression,
-- which is what the defect is conceptually about: an index key or predicate
-- must be IMMUTABLE, and `check_in_at AT TIME ZONE <text>` is only STABLE, so a
-- local-day expression cannot be indexed at all. It does not need to be.
-- `mark_absent` writes `($2::date)::timestamp AT TIME ZONE $1`, so for a given
-- (employee, date, timezone) every concurrent run computes the *identical*
-- instant — the stored column is already a faithful proxy for the local day.
-- Consequence worth stating: if a tenant changes its timezone, the same local
-- day maps to a different instant and a second placeholder becomes insertable.
-- That is arguably correct (the day genuinely moved) and is not this defect.
--
-- The marker predicate is repeated verbatim in `mark_absent`'s ON CONFLICT
-- clause, because Postgres infers the arbiter index by proving the index
-- predicate from the clause and cannot do that through a bind parameter. The
-- literal below is therefore load-bearing in two places at once;
-- `attendance_records::AUTO_ABSENT_NOTE` is the Rust half and
-- `the_auto_absent_index_predicate_matches_the_marker_note` is the test that
-- catches them drifting apart. Silent decoupling is the failure mode: reword
-- the note in Rust and every write simply stops matching the predicate, with no
-- error anywhere and duplicates quietly back.

-- Dedupe FIRST. A unique index build that fails on pre-existing duplicates
-- aborts the migration, and migrations run at container start, so a failed one
-- freezes the deploy behind it (the hazard 1012's header is written to avoid;
-- there is no NOT VALID escape hatch for an index). Keeping the lowest id keeps
-- the earliest row — ids are uuidv7, so id order is creation order. The marker
-- predicate is applied to BOTH sides of the self-join, so an HR-edited row can
-- neither be deleted nor cause one to be. No foreign key points at
-- attendance_records, so this cannot cascade.
DELETE FROM public.attendance_records a
USING public.attendance_records b
WHERE a.employee_id = b.employee_id
  AND a.check_in_at = b.check_in_at
  AND a.id > b.id
  AND a.created_by IS NULL
  AND a.status = 'absent'
  AND a.method = 'manual'
  AND a.notes = 'Auto-marked absent (no check-in recorded)'
  AND b.created_by IS NULL
  AND b.status = 'absent'
  AND b.method = 'manual'
  AND b.notes = 'Auto-marked absent (no check-in recorded)';

-- Not CONCURRENTLY: sqlx wraps each migration in a transaction, which forbids
-- it. The dedupe above makes the plain build safe. It holds a SHARE lock on
-- attendance_records for the build, which is brief and happens at container
-- start before the process starts serving traffic.
CREATE UNIQUE INDEX IF NOT EXISTS attendance_auto_absent_one_per_employee_day
    ON public.attendance_records (employee_id, check_in_at)
    WHERE created_by IS NULL
      AND status = 'absent'
      AND method = 'manual'
      AND notes = 'Auto-marked absent (no check-in recorded)';
