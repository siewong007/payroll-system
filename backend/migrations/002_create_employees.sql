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
    employee_number VARCHAR(50) NOT NULL, -- company-assigned ID

    -- Personal details
    full_name VARCHAR(255) NOT NULL,
    ic_number VARCHAR(20), -- NRIC / MyKad
    passport_number VARCHAR(50),
    date_of_birth DATE,
    gender gender_type,
    nationality VARCHAR(50) DEFAULT 'Malaysian',
    race race_type,
    residency_status residency_status NOT NULL DEFAULT 'citizen',
    marital_status marital_status DEFAULT 'single',

    -- Contact
    email VARCHAR(255),
    phone VARCHAR(20),
    address_line1 VARCHAR(255),
    address_line2 VARCHAR(255),
    city VARCHAR(100),
    state VARCHAR(50),
    postcode VARCHAR(10),

    -- Employment
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

    -- Salary
    basic_salary BIGINT NOT NULL DEFAULT 0, -- in sen (cents)
    hourly_rate BIGINT, -- in sen, for hourly-rated employees
    daily_rate BIGINT, -- in sen, for daily-rated employees

    -- Banking
    bank_name VARCHAR(100),
    bank_account_number VARCHAR(50),
    bank_account_type VARCHAR(20) DEFAULT 'savings',

    -- Statutory numbers
    tax_identification_number VARCHAR(50), -- TIN
    epf_number VARCHAR(50),
    socso_number VARCHAR(50),
    eis_number VARCHAR(50),

    -- Tax factors
    working_spouse BOOLEAN DEFAULT FALSE,
    num_children INTEGER DEFAULT 0,
    epf_category CHAR(1) DEFAULT 'A', -- A: <60 citizen, B: <60 PR, C: >60 PR, D: >60 citizen

    -- Islamic / special deductions
    is_muslim BOOLEAN DEFAULT FALSE,
    zakat_eligible BOOLEAN DEFAULT FALSE,
    zakat_monthly_amount BIGINT DEFAULT 0, -- in sen
    ptptn_monthly_amount BIGINT DEFAULT 0, -- in sen
    tabung_haji_amount BIGINT DEFAULT 0, -- in sen

    -- HRDF
    hrdf_contribution BOOLEAN DEFAULT TRUE,

    -- Payroll group
    payroll_group_id UUID,

    -- Security grouping
    salary_group VARCHAR(50) DEFAULT 'standard', -- for restricting payroll access

    is_active BOOLEAN DEFAULT TRUE,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_by UUID,
    updated_by UUID,

    UNIQUE(company_id, employee_number)
);

-- Salary history (append-only)
CREATE TABLE salary_history (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    employee_id UUID NOT NULL REFERENCES employees(id),
    old_salary BIGINT NOT NULL, -- in sen
    new_salary BIGINT NOT NULL, -- in sen
    effective_date DATE NOT NULL,
    reason VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_by UUID
);

-- TP3 opening balances for mid-year joiners
CREATE TABLE tp3_records (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    employee_id UUID NOT NULL REFERENCES employees(id),
    tax_year INTEGER NOT NULL,
    previous_employer_name VARCHAR(255),
    previous_income_ytd BIGINT NOT NULL DEFAULT 0, -- in sen
    previous_epf_ytd BIGINT NOT NULL DEFAULT 0,
    previous_pcb_ytd BIGINT NOT NULL DEFAULT 0,
    previous_socso_ytd BIGINT NOT NULL DEFAULT 0,
    previous_zakat_ytd BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_by UUID,

    UNIQUE(employee_id, tax_year)
);

CREATE INDEX idx_employees_company ON employees(company_id);
CREATE INDEX idx_employees_active ON employees(is_active) WHERE is_active = TRUE;
CREATE INDEX idx_employees_payroll_group ON employees(payroll_group_id);
CREATE INDEX idx_salary_history_employee ON salary_history(employee_id);
