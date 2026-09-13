-- Unpaid break minutes on the company's default work schedule (plan item 12).
--
-- Attendance-derived overtime compared elapsed presence against shift length
-- alone, so a standard lunch break inside a long day was paid at the overtime
-- multiplier: 09:00–19:00 against an 8-hour shift rated 2 overtime hours when
-- the employee worked 9. The break is a company policy figure, not a statutory
-- one, so it lives on the schedule row rather than in rule tables.
ALTER TABLE public.company_work_schedules
    ADD COLUMN unpaid_break_minutes integer NOT NULL DEFAULT 60;

ALTER TABLE public.company_work_schedules
    ADD CONSTRAINT company_work_schedules_unpaid_break_check
    CHECK (unpaid_break_minutes >= 0 AND unpaid_break_minutes <= 480);
