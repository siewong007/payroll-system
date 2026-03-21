CREATE TYPE document_status AS ENUM ('active', 'expired', 'archived');

CREATE TABLE document_categories (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    company_id UUID NOT NULL REFERENCES companies(id),
    name VARCHAR(100) NOT NULL,
    description VARCHAR(500),
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(company_id, name)
);

CREATE TABLE documents (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    company_id UUID NOT NULL REFERENCES companies(id),
    employee_id UUID REFERENCES employees(id),
    category_id UUID REFERENCES document_categories(id),

    title VARCHAR(255) NOT NULL,
    description VARCHAR(1000),
    file_name VARCHAR(255) NOT NULL,
    file_url VARCHAR(1000) NOT NULL,
    file_size BIGINT,
    mime_type VARCHAR(100),

    status document_status NOT NULL DEFAULT 'active',
    issue_date DATE,
    expiry_date DATE,
    is_confidential BOOLEAN DEFAULT FALSE,
    tags VARCHAR(500),

    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_by UUID,
    updated_by UUID
);

CREATE INDEX idx_documents_company ON documents(company_id);
CREATE INDEX idx_documents_employee ON documents(employee_id);
CREATE INDEX idx_documents_category ON documents(category_id);
CREATE INDEX idx_documents_expiry ON documents(expiry_date) WHERE expiry_date IS NOT NULL;
CREATE INDEX idx_document_categories_company ON document_categories(company_id);

-- Seed default categories for existing companies
INSERT INTO document_categories (company_id, name, description)
SELECT c.id, cat.name, cat.description
FROM companies c
CROSS JOIN (VALUES
    ('IC / Passport', 'Identity card or passport copy'),
    ('Offer Letter', 'Employment offer letter'),
    ('Contract', 'Employment contract'),
    ('Tax Form', 'EA form, TP3, or other tax documents'),
    ('Certification', 'Professional certifications and licenses'),
    ('Medical', 'Medical reports or insurance documents'),
    ('Leave', 'Leave application forms'),
    ('Other', 'Miscellaneous documents')
) AS cat(name, description);
