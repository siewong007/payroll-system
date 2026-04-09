-- Statutory tables: EPF, SOCSO, EIS, PCB
CREATE TABLE epf_rates (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    wage_from BIGINT NOT NULL,
    wage_to BIGINT NOT NULL,
    employee_contribution BIGINT NOT NULL,
    employer_contribution BIGINT NOT NULL,
    category CHAR(1) NOT NULL DEFAULT 'A',
    effective_from DATE NOT NULL,
    effective_to DATE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_epf_rates_lookup ON epf_rates(category, effective_from, wage_from, wage_to);

CREATE TABLE socso_rates (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    wage_from BIGINT NOT NULL,
    wage_to BIGINT NOT NULL,
    first_cat_employee BIGINT NOT NULL,
    first_cat_employer BIGINT NOT NULL,
    second_cat_employer BIGINT NOT NULL,
    effective_from DATE NOT NULL,
    effective_to DATE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_socso_rates_lookup ON socso_rates(effective_from, wage_from, wage_to);

CREATE TABLE eis_rates (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    wage_from BIGINT NOT NULL,
    wage_to BIGINT NOT NULL,
    employee_contribution BIGINT NOT NULL,
    employer_contribution BIGINT NOT NULL,
    effective_from DATE NOT NULL,
    effective_to DATE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_eis_rates_lookup ON eis_rates(effective_from, wage_from, wage_to);

CREATE TABLE pcb_brackets (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    chargeable_income_from BIGINT NOT NULL,
    chargeable_income_to BIGINT NOT NULL,
    tax_rate_percent NUMERIC(5, 2) NOT NULL,
    cumulative_tax BIGINT NOT NULL,
    effective_year INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_pcb_brackets_lookup ON pcb_brackets(effective_year, chargeable_income_from);

CREATE TABLE pcb_reliefs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    relief_type VARCHAR(50) NOT NULL,
    amount BIGINT NOT NULL,
    effective_year INTEGER NOT NULL,
    description VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

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
