-- Bulk employee import sessions
CREATE TABLE bulk_import_sessions (
    id UUID PRIMARY KEY,
    company_id UUID NOT NULL REFERENCES companies(id),
    user_id UUID NOT NULL,
    file_name TEXT NOT NULL,
    row_count INTEGER NOT NULL,
    valid_count INTEGER NOT NULL,
    validated_data JSONB NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    confirmed_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ NOT NULL DEFAULT NOW() + INTERVAL '1 hour'
);

CREATE INDEX idx_bulk_import_sessions_company ON bulk_import_sessions(company_id);
CREATE INDEX idx_bulk_import_sessions_status ON bulk_import_sessions(status);
