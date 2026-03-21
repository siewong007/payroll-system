-- EPF (KWSP) Third Schedule contribution table
-- Amounts stored in sen (cents) for exact cent accuracy
CREATE TABLE epf_rates (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    wage_from BIGINT NOT NULL, -- in sen
    wage_to BIGINT NOT NULL,   -- in sen
    employee_contribution BIGINT NOT NULL, -- in sen
    employer_contribution BIGINT NOT NULL, -- in sen
    category CHAR(1) NOT NULL DEFAULT 'A',
        -- A: citizen/PR < 60 (11% employee, 13%/12% employer)
        -- B: citizen/PR < 60, elected 9% (Form KWSP 17A)
        -- C: PR > 60 (employment injury only)
        -- D: citizen > 60
    effective_from DATE NOT NULL,
    effective_to DATE, -- NULL = current
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_epf_rates_lookup ON epf_rates(category, effective_from, wage_from, wage_to);

-- SOCSO (PERKESO) First Schedule contribution table
CREATE TABLE socso_rates (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    wage_from BIGINT NOT NULL, -- in sen
    wage_to BIGINT NOT NULL,   -- in sen
    first_cat_employee BIGINT NOT NULL, -- First Category employee share (sen)
    first_cat_employer BIGINT NOT NULL, -- First Category employer share (sen)
    second_cat_employer BIGINT NOT NULL, -- Second Category employer only (sen)
    effective_from DATE NOT NULL,
    effective_to DATE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_socso_rates_lookup ON socso_rates(effective_from, wage_from, wage_to);

-- EIS contribution table
CREATE TABLE eis_rates (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    wage_from BIGINT NOT NULL, -- in sen
    wage_to BIGINT NOT NULL,   -- in sen
    employee_contribution BIGINT NOT NULL, -- in sen
    employer_contribution BIGINT NOT NULL, -- in sen
    effective_from DATE NOT NULL,
    effective_to DATE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_eis_rates_lookup ON eis_rates(effective_from, wage_from, wage_to);

-- PCB tax brackets (Schedule 1)
CREATE TABLE pcb_brackets (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    chargeable_income_from BIGINT NOT NULL, -- annual, in sen
    chargeable_income_to BIGINT NOT NULL,   -- annual, in sen
    tax_rate_percent NUMERIC(5, 2) NOT NULL,
    cumulative_tax BIGINT NOT NULL, -- in sen, tax for all brackets below
    effective_year INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_pcb_brackets_lookup ON pcb_brackets(effective_year, chargeable_income_from);

-- PCB relief amounts
CREATE TABLE pcb_reliefs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    relief_type VARCHAR(50) NOT NULL,
    -- Types: individual, spouse, child, child_18_plus_education,
    --        disabled_individual, disabled_spouse, disabled_child,
    --        life_insurance, medical_insurance, education_fees,
    --        epf_relief, socso_relief, eis_relief
    amount BIGINT NOT NULL, -- annual, in sen
    effective_year INTEGER NOT NULL,
    description VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- System settings (minimum wage, working hours, etc.)
CREATE TABLE system_settings (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    setting_key VARCHAR(100) NOT NULL UNIQUE,
    setting_value VARCHAR(500) NOT NULL,
    description VARCHAR(255),
    effective_from DATE NOT NULL,
    effective_to DATE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
