-- Companies (multi-company support)
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE companies (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(255) NOT NULL,
    registration_number VARCHAR(50), -- SSM number
    tax_number VARCHAR(50), -- LHDN employer number
    epf_number VARCHAR(50), -- KWSP employer number
    socso_code VARCHAR(50), -- PERKESO code
    eis_code VARCHAR(50),
    hrdf_number VARCHAR(50),
    address_line1 VARCHAR(255),
    address_line2 VARCHAR(255),
    city VARCHAR(100),
    state VARCHAR(50),
    postcode VARCHAR(10),
    country VARCHAR(50) DEFAULT 'Malaysia',
    phone VARCHAR(20),
    email VARCHAR(255),
    logo_url VARCHAR(500),
    hrdf_enabled BOOLEAN DEFAULT FALSE,
    unpaid_leave_divisor INTEGER DEFAULT 26,
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_by UUID,
    updated_by UUID
);

-- Users for auth
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    email VARCHAR(255) NOT NULL UNIQUE,
    password_hash VARCHAR(255) NOT NULL,
    full_name VARCHAR(255) NOT NULL,
    role VARCHAR(50) NOT NULL DEFAULT 'employee'
        CHECK (role IN ('super_admin', 'payroll_admin', 'hr_manager', 'finance', 'employee')),
    company_id UUID REFERENCES companies(id),
    employee_id UUID, -- linked later
    is_active BOOLEAN DEFAULT TRUE,
    last_login TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
