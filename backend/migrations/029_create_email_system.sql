-- Email templates for HR letters
CREATE TABLE IF NOT EXISTS email_templates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    company_id UUID NOT NULL REFERENCES companies(id),
    name VARCHAR(100) NOT NULL,
    letter_type VARCHAR(50) NOT NULL, -- welcome, offer, appointment, warning, termination, promotion
    subject VARCHAR(500) NOT NULL,
    body_html TEXT NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_by UUID REFERENCES users(id),
    updated_by UUID REFERENCES users(id)
);

CREATE INDEX idx_email_templates_company ON email_templates(company_id);
CREATE INDEX idx_email_templates_type ON email_templates(company_id, letter_type);

-- Email send log
CREATE TABLE IF NOT EXISTS email_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    company_id UUID NOT NULL REFERENCES companies(id),
    employee_id UUID REFERENCES employees(id),
    template_id UUID REFERENCES email_templates(id),
    letter_type VARCHAR(50) NOT NULL,
    recipient_email VARCHAR(500) NOT NULL,
    recipient_name VARCHAR(500),
    subject VARCHAR(500) NOT NULL,
    body_html TEXT NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'pending', -- pending, sent, failed
    error_message TEXT,
    sent_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_by UUID REFERENCES users(id)
);

CREATE INDEX idx_email_logs_company ON email_logs(company_id);
CREATE INDEX idx_email_logs_employee ON email_logs(company_id, employee_id);
CREATE INDEX idx_email_logs_status ON email_logs(status);

-- Company email settings
INSERT INTO company_settings (company_id, category, key, value, label, description)
SELECT id, 'email', 'auto_welcome_email', 'true'::jsonb, 'Auto Welcome Email', 'Automatically send welcome email when a new employee is created'
FROM companies
ON CONFLICT DO NOTHING;
