-- Add attachment support for leave requests
ALTER TABLE leave_requests ADD COLUMN attachment_url VARCHAR(500);
ALTER TABLE leave_requests ADD COLUMN attachment_name VARCHAR(255);
