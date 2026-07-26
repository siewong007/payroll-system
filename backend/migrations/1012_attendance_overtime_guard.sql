-- Guards for the forgotten-check-out defect. Derived overtime was written from
-- wall-clock elapsed time with no ceiling, and check-out matches any open record
-- inside a 24h window, so a Monday check-in closed on Tuesday produced ~14 h of
-- paid overtime that also inflated gross and therefore every statutory
-- contribution derived from it.
--
-- 1. Structural invariant. The per-day ceiling itself is a company setting
--    (tenant-specific, changeable) and deliberately NOT expressed here. What IS
--    tenant-independent is that overtime is a subset of the hours it was derived
--    from. Both write paths already satisfy it
--    (overtime = GREATEST(0, elapsed - shift), hours_worked = elapsed), so no
--    automatically written row can fail.
--
--    The VALIDATE is nonetheless conditional. `attendance_records::update` —
--    the HR correction path — is deliberately uncapped, so a hand-corrected
--    legacy row may already hold overtime_hours > hours_worked. Migrations run
--    on container start, and a failed one freezes the deploy, so a single such
--    row must not be able to wedge production. A NOT VALID constraint is still
--    enforced against every INSERT and UPDATE; skipping VALIDATE only forgoes
--    the backfill scan, which is the half that can fail.

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'attendance_records_overtime_within_hours_check'
          AND conrelid = 'public.attendance_records'::regclass
    ) THEN
        ALTER TABLE public.attendance_records
            ADD CONSTRAINT attendance_records_overtime_within_hours_check
            CHECK (
                overtime_hours IS NULL
                OR hours_worked IS NULL
                OR overtime_hours <= hours_worked
            ) NOT VALID;
    END IF;
END $$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM public.attendance_records
        WHERE overtime_hours IS NOT NULL
          AND hours_worked IS NOT NULL
          AND overtime_hours > hours_worked
    ) THEN
        ALTER TABLE public.attendance_records
            VALIDATE CONSTRAINT attendance_records_overtime_within_hours_check;
    ELSE
        RAISE WARNING 'attendance_records_overtime_within_hours_check left NOT VALID: % existing row(s) hold overtime_hours > hours_worked. New writes are still rejected; correct those records and run VALIDATE CONSTRAINT manually.',
            (SELECT count(*) FROM public.attendance_records
              WHERE overtime_hours IS NOT NULL
                AND hours_worked IS NOT NULL
                AND overtime_hours > hours_worked);
    END IF;
END $$;

-- 2. The per-day overtime ceiling, above which an automatically derived figure
--    is left unrated (NULL) for HR review instead of being paid.
--
--    4 is an anomaly threshold, not a statutory derivation: it separates a
--    plausible long day from a check-out nobody performed. It is deliberately
--    not tied to the Employment Act 1955 s.60A(1)(b) 12 h cap, because the
--    check-out SQL falls back to a 9 h shift when a company has no default
--    schedule, and 9 + 4 exceeds that cap. Companies running genuine long
--    shifts (12 h rotations, security) raise this before the deploy.
--
--    The application falls back to 4 when the row is absent, so existing and
--    future companies behave correctly either way; this backfill exists so the
--    value is discoverable and editable in the settings UI. Extending
--    provision_company_defaults to seed it for NEW companies is tracked
--    separately — re-issuing that ~90-line immutable-baseline function by hand
--    to add one tuple risks breaking provisioning for every future tenant.

INSERT INTO public.company_settings (company_id, category, key, value, label, description)
SELECT c.id,
       'payroll',
       'max_overtime_hours_per_day',
       '"4"'::jsonb,
       'Max Overtime Hours Per Day',
       'Overtime beyond this in a single check-out is left unrated for review instead of paid automatically'
FROM public.companies c
ON CONFLICT (company_id, category, key) DO NOTHING;
