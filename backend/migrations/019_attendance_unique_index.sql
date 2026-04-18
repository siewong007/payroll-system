-- Fix attendance check-in race condition
-- Only one open attendance record (check_out_at IS NULL) allowed per employee.

CREATE UNIQUE INDEX attendance_one_open_per_employee
ON attendance_records (employee_id)
WHERE check_out_at IS NULL;
