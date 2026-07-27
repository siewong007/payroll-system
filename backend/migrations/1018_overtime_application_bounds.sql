-- Database backstop for the overtime-hours rule.
--
-- `overtime_applications.hours` is multiplied by an hourly rate to stage a
-- payroll earning on approval, and the admin create/update paths took the value
-- straight from the request with no bound at all: 999.99 hours over a declared
-- one-hour window staged roughly RM65,000, and a negative value staged a
-- negative earning. Only the self-service portal checked, and it owned its own
-- copy of the rule.
--
-- The service-side rule is the real gate — it caps `hours` against the window
-- the applicant declared, which the database cannot see. This CHECK is the
-- outer bound that rule already implies: the midnight-wrap makes a declared
-- window at most 24 h, so `hours <= declared_hours` entails `hours <= 24`. It
-- is a consistent backstop, not a second and looser rule.
--
-- NOT VALID with a *conditional* VALIDATE: migrations run at container start and
-- a failed one freezes production, so one out-of-range legacy row must not be
-- able to wedge the deploy. A NOT VALID constraint is still enforced against
-- every INSERT and UPDATE; skipping VALIDATE only forgoes the backfill scan.
-- `approve_overtime` re-validates before staging, so such a row cannot be paid
-- in the meantime either.

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'overtime_applications_hours_check'
          AND conrelid = 'public.overtime_applications'::regclass
    ) THEN
        ALTER TABLE public.overtime_applications
            ADD CONSTRAINT overtime_applications_hours_check
            CHECK (hours > 0 AND hours <= 24) NOT VALID;
    END IF;
END $$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM public.overtime_applications WHERE hours <= 0 OR hours > 24
    ) THEN
        ALTER TABLE public.overtime_applications
            VALIDATE CONSTRAINT overtime_applications_hours_check;
    ELSE
        RAISE WARNING 'overtime_applications_hours_check left NOT VALID: % existing row(s) hold hours outside (0, 24]. New writes are still rejected and approval refuses them; correct those rows and run VALIDATE CONSTRAINT manually.',
            (SELECT count(*) FROM public.overtime_applications WHERE hours <= 0 OR hours > 24);
    END IF;
END $$;
