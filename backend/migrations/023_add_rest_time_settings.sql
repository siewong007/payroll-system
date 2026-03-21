-- Add working hours and rest time configuration to company settings
INSERT INTO company_settings (company_id, category, key, value, label, description)
SELECT c.id, d.category, d.key, d.value::jsonb, d.label, d.description
FROM companies c
CROSS JOIN (VALUES
    ('payroll', 'working_hours_per_day', '"9"', 'Working Hours Per Day', 'Total office hours per day including rest time (e.g. 9 for 9am-6pm)'),
    ('payroll', 'rest_time_minutes', '"60"', 'Rest Time (minutes)', 'Afternoon rest/lunch break duration in minutes. This time is excluded from payroll hour calculations (e.g. overtime hourly rate).'),
    ('payroll', 'effective_hours_per_day', '"8"', 'Effective Hours Per Day', 'Actual working hours per day after deducting rest time. Used to calculate hourly rate from monthly salary.')
) AS d(category, key, value, label, description)
ON CONFLICT (company_id, category, key) DO NOTHING;
