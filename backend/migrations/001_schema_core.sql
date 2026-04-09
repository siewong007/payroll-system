-- Core schema: companies, users, employees, payroll groups, salary history
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE companies (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(255) NOT NULL,
    registration_number VARCHAR(50),
    tax_number VARCHAR(50),
    epf_number VARCHAR(50),
    socso_code VARCHAR(50),
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

CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    email VARCHAR(255) NOT NULL UNIQUE,
    password_hash VARCHAR(255) NOT NULL,
    full_name VARCHAR(255) NOT NULL,
    role VARCHAR(50) NOT NULL DEFAULT 'employee'
        CHECK (role IN ('super_admin', 'admin', 'payroll_admin', 'hr_manager', 'finance', 'exec', 'employee')),
    company_id UUID REFERENCES companies(id),
    employee_id UUID,
    is_active BOOLEAN DEFAULT TRUE,
    last_login TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE user_companies (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, company_id)
);

CREATE INDEX idx_user_companies_user ON user_companies(user_id);
CREATE INDEX idx_user_companies_company ON user_companies(company_id);

CREATE TYPE employment_type AS ENUM (
    'permanent', 'contract', 'part_time', 'intern', 'daily_rated', 'hourly_rated'
);
CREATE TYPE gender_type AS ENUM ('male', 'female');
CREATE TYPE marital_status AS ENUM ('single', 'married', 'divorced', 'widowed');
CREATE TYPE residency_status AS ENUM ('citizen', 'permanent_resident', 'foreigner');
CREATE TYPE race_type AS ENUM ('malay', 'chinese', 'indian', 'other');

CREATE TABLE employees (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    company_id UUID NOT NULL REFERENCES companies(id),
    employee_number VARCHAR(50) NOT NULL,
    full_name VARCHAR(255) NOT NULL,
    ic_number VARCHAR(20),
    passport_number VARCHAR(50),
    date_of_birth DATE,
    gender gender_type,
    nationality VARCHAR(50) DEFAULT 'Malaysian',
    race race_type,
    residency_status residency_status NOT NULL DEFAULT 'citizen',
    marital_status marital_status DEFAULT 'single',
    email VARCHAR(255),
    phone VARCHAR(20),
    address_line1 VARCHAR(255),
    address_line2 VARCHAR(255),
    city VARCHAR(100),
    state VARCHAR(50),
    postcode VARCHAR(10),
    department VARCHAR(100),
    designation VARCHAR(100),
    cost_centre VARCHAR(50),
    branch VARCHAR(100),
    employment_type employment_type NOT NULL DEFAULT 'permanent',
    date_joined DATE NOT NULL,
    probation_start DATE,
    probation_end DATE,
    confirmation_date DATE,
    date_resigned DATE,
    resignation_reason VARCHAR(500),
    basic_salary BIGINT NOT NULL DEFAULT 0,
    hourly_rate BIGINT,
    daily_rate BIGINT,
    bank_name VARCHAR(100),
    bank_account_number VARCHAR(50),
    bank_account_type VARCHAR(20) DEFAULT 'savings',
    tax_identification_number VARCHAR(50),
    epf_number VARCHAR(50),
    socso_number VARCHAR(50),
    eis_number VARCHAR(50),
    working_spouse BOOLEAN DEFAULT FALSE,
    num_children INTEGER DEFAULT 0,
    epf_category CHAR(1) DEFAULT 'A',
    is_muslim BOOLEAN DEFAULT FALSE,
    zakat_eligible BOOLEAN DEFAULT FALSE,
    zakat_monthly_amount BIGINT DEFAULT 0,
    ptptn_monthly_amount BIGINT DEFAULT 0,
    tabung_haji_amount BIGINT DEFAULT 0,
    hrdf_contribution BOOLEAN DEFAULT TRUE,
    payroll_group_id UUID,
    salary_group VARCHAR(50) DEFAULT 'standard',
    is_active BOOLEAN DEFAULT TRUE,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_by UUID,
    updated_by UUID,
    UNIQUE(company_id, employee_number)
);

CREATE TABLE salary_history (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    employee_id UUID NOT NULL REFERENCES employees(id),
    old_salary BIGINT NOT NULL,
    new_salary BIGINT NOT NULL,
    effective_date DATE NOT NULL,
    reason VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_by UUID
);

CREATE TABLE tp3_records (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    employee_id UUID NOT NULL REFERENCES employees(id),
    tax_year INTEGER NOT NULL,
    previous_employer_name VARCHAR(255),
    previous_income_ytd BIGINT NOT NULL DEFAULT 0,
    previous_epf_ytd BIGINT NOT NULL DEFAULT 0,
    previous_pcb_ytd BIGINT NOT NULL DEFAULT 0,
    previous_socso_ytd BIGINT NOT NULL DEFAULT 0,
    previous_zakat_ytd BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_by UUID,
    UNIQUE(employee_id, tax_year)
);

CREATE TABLE payroll_groups (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    company_id UUID NOT NULL REFERENCES companies(id),
    name VARCHAR(100) NOT NULL,
    description VARCHAR(255),
    cutoff_day INTEGER NOT NULL DEFAULT 25,
    payment_day INTEGER NOT NULL DEFAULT 28,
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_by UUID,
    updated_by UUID
);

ALTER TABLE employees
    ADD CONSTRAINT fk_employees_payroll_group
    FOREIGN KEY (payroll_group_id) REFERENCES payroll_groups(id);

CREATE INDEX idx_employees_company ON employees(company_id);
CREATE INDEX idx_employees_active ON employees(is_active) WHERE is_active = TRUE;
CREATE INDEX idx_employees_payroll_group ON employees(payroll_group_id);
CREATE INDEX idx_salary_history_employee ON salary_history(employee_id);
