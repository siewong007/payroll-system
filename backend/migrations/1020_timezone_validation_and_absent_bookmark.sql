-- Per-company auto-absent bookmark. The single global platform_settings key
-- could not express "company A is done through the 9th, B through the 10th",
-- which is what a per-tenant calendar requires.
ALTER TABLE public.companies
    ADD COLUMN IF NOT EXISTS auto_absent_last_run_date date;

-- Seed from the retiring global key so the first tick after deploy neither
-- re-backfills 14 days for every tenant nor skips a day. The regex guard keeps
-- a garbage stored value from failing the migration itself.
UPDATE public.companies c
SET auto_absent_last_run_date = s.value::date
FROM public.platform_settings s
WHERE s.key = 'auto_absent_last_run_date'
  AND s.value ~ '^\d{4}-\d{2}-\d{2}$'
  AND c.auto_absent_last_run_date IS NULL;

-- Repair any stored zone Postgres would reject, BEFORE the guard exists.
UPDATE public.company_work_schedules ws
SET timezone = 'Asia/Kuala_Lumpur'
WHERE ws.timezone IS NULL
   OR NOT EXISTS (SELECT 1 FROM pg_timezone_names z WHERE z.name = ws.timezone);

UPDATE public.companies c
SET timezone = 'Asia/Kuala_Lumpur'
WHERE c.timezone IS NULL
   OR NOT EXISTS (SELECT 1 FROM pg_timezone_names z WHERE z.name = c.timezone);

-- A CHECK constraint cannot contain a subquery and pg_timezone_names is not
-- immutable, so the database-level guard has to be a trigger. Defence in depth
-- only: core::timezone::validate is the primary guard and yields a clean 400.
CREATE OR REPLACE FUNCTION public.assert_valid_timezone() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.timezone IS NULL
       OR NOT EXISTS (SELECT 1 FROM pg_timezone_names WHERE name = NEW.timezone) THEN
        RAISE EXCEPTION 'invalid IANA timezone: %', NEW.timezone USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS company_work_schedules_timezone_valid ON public.company_work_schedules;
CREATE TRIGGER company_work_schedules_timezone_valid
    BEFORE INSERT OR UPDATE OF timezone ON public.company_work_schedules
    FOR EACH ROW EXECUTE FUNCTION public.assert_valid_timezone();

DROP TRIGGER IF EXISTS companies_timezone_valid ON public.companies;
CREATE TRIGGER companies_timezone_valid
    BEFORE INSERT OR UPDATE OF timezone ON public.companies
    FOR EACH ROW EXECUTE FUNCTION public.assert_valid_timezone();
