-- PostgreSQL 19 canonical schema and live-schema reconciliation.
-- Generated from PostgreSQL 19 Beta 2; requires PostgreSQL 18+ for uuidv7().
-- Fresh databases execute the canonical bootstrap; existing v1-v4 databases
-- skip object creation and run the idempotent reconciliation section.

--
-- PostgreSQL database dump
--

-- Dumped from database version 19beta2
-- Dumped by pg_dump version 19beta2

DO $minimum_version$
BEGIN
    IF current_setting('server_version_num')::integer < 180000 THEN
        RAISE EXCEPTION 'PostgreSQL 18 or newer is required; PostgreSQL 19 is the canonical target';
    END IF;
END
$minimum_version$;

DO $pg19_bootstrap$
BEGIN
    IF to_regclass('public.companies') IS NULL THEN

--
-- Name: pg_trgm; Type: EXTENSION; Schema: -; Owner: -
--

CREATE EXTENSION IF NOT EXISTS pg_trgm WITH SCHEMA public;


--
-- Name: EXTENSION pg_trgm; Type: COMMENT; Schema: -; Owner: -
--

COMMENT ON EXTENSION pg_trgm IS 'text similarity measurement and index searching based on trigrams';


--
-- Name: document_status; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.document_status AS ENUM (
    'active',
    'expired',
    'archived'
);


--
-- Name: employment_type; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.employment_type AS ENUM (
    'permanent',
    'contract',
    'part_time',
    'intern',
    'daily_rated',
    'hourly_rated'
);


--
-- Name: gender_type; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.gender_type AS ENUM (
    'male',
    'female'
);


--
-- Name: marital_status; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.marital_status AS ENUM (
    'single',
    'married',
    'divorced',
    'widowed'
);


--
-- Name: payroll_status; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.payroll_status AS ENUM (
    'draft',
    'processing',
    'processed',
    'pending_approval',
    'approved',
    'paid',
    'cancelled'
);


--
-- Name: race_type; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.race_type AS ENUM (
    'malay',
    'chinese',
    'indian',
    'other'
);


--
-- Name: residency_status; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.residency_status AS ENUM (
    'citizen',
    'permanent_resident',
    'foreigner'
);


--
-- Name: attendance_kiosk_credentials; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.attendance_kiosk_credentials (
    id uuid DEFAULT uuidv7() NOT NULL,
    company_id uuid NOT NULL,
    label character varying(100) NOT NULL,
    token_hash character varying(128) NOT NULL,
    token_prefix character varying(12) NOT NULL,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    last_used_at timestamp with time zone,
    last_used_ip text,
    revoked_at timestamp with time zone
);


--
-- Name: attendance_qr_tokens; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.attendance_qr_tokens (
    id uuid DEFAULT uuidv7() NOT NULL,
    company_id uuid NOT NULL,
    token character varying(128) NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    used boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: attendance_records; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.attendance_records (
    id uuid DEFAULT uuidv7() NOT NULL,
    company_id uuid NOT NULL,
    employee_id uuid NOT NULL,
    check_in_at timestamp with time zone DEFAULT now() NOT NULL,
    check_out_at timestamp with time zone,
    method character varying(20) NOT NULL,
    status character varying(20) DEFAULT 'present'::character varying NOT NULL,
    latitude double precision,
    longitude double precision,
    checkout_latitude double precision,
    checkout_longitude double precision,
    notes text,
    qr_token_id uuid,
    created_by uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    hours_worked numeric(5,2),
    overtime_hours numeric(5,2) DEFAULT 0,
    is_outside_geofence boolean DEFAULT false,
    CONSTRAINT attendance_records_checkout_order_check CHECK (((check_out_at IS NULL) OR (check_out_at >= check_in_at))),
    CONSTRAINT attendance_records_checkin_coordinates_check CHECK ((((latitude IS NULL) = (longitude IS NULL)) AND ((latitude IS NULL) OR ((latitude >= ('-90'::integer)::double precision) AND (latitude <= (90)::double precision) AND (longitude >= ('-180'::integer)::double precision) AND (longitude <= (180)::double precision))))),
    CONSTRAINT attendance_records_checkout_coordinates_check CHECK ((((checkout_latitude IS NULL) = (checkout_longitude IS NULL)) AND ((checkout_latitude IS NULL) OR ((checkout_latitude >= ('-90'::integer)::double precision) AND (checkout_latitude <= (90)::double precision) AND (checkout_longitude >= ('-180'::integer)::double precision) AND (checkout_longitude <= (180)::double precision))))),
    CONSTRAINT attendance_records_hours_check CHECK ((((hours_worked IS NULL) OR (hours_worked >= (0)::numeric)) AND ((overtime_hours IS NULL) OR (overtime_hours >= (0)::numeric)))),
    CONSTRAINT attendance_records_method_check CHECK (((method)::text = ANY (ARRAY[('qr_code'::character varying)::text, ('face_id'::character varying)::text, ('manual'::character varying)::text]))),
    CONSTRAINT attendance_records_status_check CHECK (((status)::text = ANY (ARRAY[('present'::character varying)::text, ('late'::character varying)::text, ('absent'::character varying)::text, ('half_day'::character varying)::text])))
);


--
-- Name: audit_logs; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.audit_logs (
    id uuid DEFAULT uuidv7() NOT NULL,
    user_id uuid,
    action character varying(50) NOT NULL,
    entity_type character varying(50) NOT NULL,
    entity_id uuid,
    old_values jsonb,
    new_values jsonb,
    ip_address character varying(45),
    user_agent character varying(500),
    description character varying(500),
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    company_id uuid
);


--
-- Name: bulk_import_sessions; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.bulk_import_sessions (
    id uuid DEFAULT uuidv7() NOT NULL,
    company_id uuid NOT NULL,
    user_id uuid NOT NULL,
    file_name text NOT NULL,
    row_count integer NOT NULL,
    valid_count integer NOT NULL,
    validated_data jsonb NOT NULL,
    status text DEFAULT 'pending'::text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    confirmed_at timestamp with time zone,
    expires_at timestamp with time zone DEFAULT (now() + '01:00:00'::interval) NOT NULL,
    CONSTRAINT bulk_import_sessions_counts_check CHECK (((row_count >= 0) AND (valid_count >= 0) AND (valid_count <= row_count))),
    CONSTRAINT bulk_import_sessions_expiry_check CHECK ((expires_at > created_at)),
    CONSTRAINT bulk_import_sessions_status_check CHECK ((status = ANY (ARRAY['pending'::text, 'confirmed'::text, 'expired'::text, 'failed'::text])))
);


--
-- Name: claims; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.claims (
    id uuid DEFAULT uuidv7() NOT NULL,
    employee_id uuid NOT NULL,
    company_id uuid NOT NULL,
    title character varying(255) NOT NULL,
    description text,
    amount bigint NOT NULL,
    category character varying(100),
    receipt_url character varying(500),
    receipt_file_name character varying(255),
    expense_date date NOT NULL,
    status character varying(20) DEFAULT 'draft'::character varying NOT NULL,
    submitted_at timestamp with time zone,
    reviewed_by uuid,
    reviewed_at timestamp with time zone,
    review_notes text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT claims_status_check CHECK (((status)::text = ANY (ARRAY[('draft'::character varying)::text, ('pending'::character varying)::text, ('approved'::character varying)::text, ('rejected'::character varying)::text, ('processed'::character varying)::text, ('cancelled'::character varying)::text])))
);


--
-- Name: companies; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.companies (
    id uuid DEFAULT uuidv7() NOT NULL,
    name character varying(255) NOT NULL,
    registration_number character varying(50),
    tax_number character varying(50),
    epf_number character varying(50),
    socso_code character varying(50),
    eis_code character varying(50),
    hrdf_number character varying(50),
    address_line1 character varying(255),
    address_line2 character varying(255),
    city character varying(100),
    state character varying(50),
    postcode character varying(10),
    country character varying(50) DEFAULT 'Malaysia'::character varying,
    phone character varying(20),
    email character varying(255),
    logo_url character varying(500),
    hrdf_enabled boolean DEFAULT false,
    unpaid_leave_divisor integer DEFAULT 26,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    created_by uuid,
    updated_by uuid,
    attendance_method character varying(20) DEFAULT NULL::character varying,
    timezone character varying(50) DEFAULT 'Asia/Kuala_Lumpur'::character varying NOT NULL,
    geofence_mode character varying(10) DEFAULT 'none'::character varying NOT NULL,
    CONSTRAINT companies_attendance_method_check CHECK (((attendance_method)::text = ANY (ARRAY[('qr_code'::character varying)::text, ('face_id'::character varying)::text]))),
    CONSTRAINT companies_geofence_mode_check CHECK (((geofence_mode)::text = ANY (ARRAY[('none'::character varying)::text, ('warn'::character varying)::text, ('enforce'::character varying)::text]))),
    CONSTRAINT companies_unpaid_leave_divisor_check CHECK ((unpaid_leave_divisor > 0))
);


--
-- Name: company_locations; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.company_locations (
    id uuid DEFAULT uuidv7() NOT NULL,
    company_id uuid NOT NULL,
    name character varying(150) NOT NULL,
    latitude double precision NOT NULL,
    longitude double precision NOT NULL,
    radius_meters integer DEFAULT 200 NOT NULL,
    is_active boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT company_locations_coordinates_check CHECK ((((latitude >= ('-90'::integer)::double precision) AND (latitude <= (90)::double precision)) AND ((longitude >= ('-180'::integer)::double precision) AND (longitude <= (180)::double precision)))),
    CONSTRAINT company_locations_radius_check CHECK ((radius_meters > 0))
);


--
-- Name: company_settings; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.company_settings (
    id uuid DEFAULT uuidv7() NOT NULL,
    company_id uuid NOT NULL,
    category character varying(50) NOT NULL,
    key character varying(100) NOT NULL,
    value jsonb NOT NULL,
    label character varying(255),
    description character varying(500),
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_by uuid
);


--
-- Name: company_work_schedules; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.company_work_schedules (
    id uuid DEFAULT uuidv7() NOT NULL,
    company_id uuid NOT NULL,
    name character varying(100) DEFAULT 'Default'::character varying NOT NULL,
    start_time time without time zone DEFAULT '09:00:00'::time without time zone NOT NULL,
    end_time time without time zone DEFAULT '18:00:00'::time without time zone NOT NULL,
    grace_minutes integer DEFAULT 15 NOT NULL,
    half_day_hours numeric(4,2) DEFAULT 4.0 NOT NULL,
    timezone character varying(50) DEFAULT 'Asia/Kuala_Lumpur'::character varying NOT NULL,
    is_default boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: document_categories; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.document_categories (
    id uuid DEFAULT uuidv7() NOT NULL,
    company_id uuid NOT NULL,
    name character varying(100) NOT NULL,
    description character varying(500),
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: documents; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.documents (
    id uuid DEFAULT uuidv7() NOT NULL,
    company_id uuid NOT NULL,
    employee_id uuid,
    category_id uuid,
    title character varying(255) NOT NULL,
    description character varying(1000),
    file_name character varying(255) NOT NULL,
    file_url character varying(1000) NOT NULL,
    file_size bigint,
    mime_type character varying(100),
    status public.document_status DEFAULT 'active'::public.document_status NOT NULL,
    issue_date date,
    expiry_date date,
    is_confidential boolean DEFAULT false,
    tags character varying(500),
    deleted_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    created_by uuid,
    updated_by uuid
);


--
-- Name: eis_rates; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.eis_rates (
    id uuid DEFAULT uuidv7() NOT NULL,
    wage_from bigint NOT NULL,
    wage_to bigint NOT NULL,
    employee_contribution bigint NOT NULL,
    employer_contribution bigint NOT NULL,
    effective_from date NOT NULL,
    effective_to date,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT eis_rates_contributions_check CHECK (((employee_contribution >= 0) AND (employer_contribution >= 0))),
    CONSTRAINT eis_rates_effective_dates_check CHECK (((effective_to IS NULL) OR (effective_to >= effective_from))),
    CONSTRAINT eis_rates_range_check CHECK (((wage_from >= 0) AND (wage_to >= wage_from)))
);


--
-- Name: email_logs; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.email_logs (
    id uuid DEFAULT uuidv7() NOT NULL,
    company_id uuid NOT NULL,
    employee_id uuid,
    template_id uuid,
    letter_type character varying(50) NOT NULL,
    recipient_email character varying(500) NOT NULL,
    recipient_name character varying(500),
    subject character varying(500) NOT NULL,
    body_html text NOT NULL,
    status character varying(20) DEFAULT 'pending'::character varying NOT NULL,
    error_message text,
    sent_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    created_by uuid
);


--
-- Name: email_templates; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.email_templates (
    id uuid DEFAULT uuidv7() NOT NULL,
    company_id uuid NOT NULL,
    name character varying(100) NOT NULL,
    letter_type character varying(50) NOT NULL,
    subject character varying(500) NOT NULL,
    body_html text NOT NULL,
    is_active boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    created_by uuid,
    updated_by uuid
);


--
-- Name: employee_allowances; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.employee_allowances (
    id uuid DEFAULT uuidv7() NOT NULL,
    employee_id uuid NOT NULL,
    category character varying(20) NOT NULL,
    name character varying(100) NOT NULL,
    description character varying(255),
    amount bigint NOT NULL,
    is_taxable boolean DEFAULT true,
    is_recurring boolean DEFAULT true,
    effective_from date NOT NULL,
    effective_to date,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    created_by uuid,
    updated_by uuid,
    CONSTRAINT employee_allowances_category_check CHECK (((category)::text = ANY (ARRAY[('earning'::character varying)::text, ('deduction'::character varying)::text])))
);


--
-- Name: employee_work_schedules; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.employee_work_schedules (
    id uuid DEFAULT uuidv7() NOT NULL,
    employee_id uuid NOT NULL,
    company_id uuid NOT NULL,
    day_of_week smallint NOT NULL,
    start_time time without time zone DEFAULT '09:00:00'::time without time zone NOT NULL,
    end_time time without time zone DEFAULT '18:00:00'::time without time zone NOT NULL,
    grace_minutes integer DEFAULT 15 NOT NULL,
    is_active boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT employee_work_schedules_day_of_week_check CHECK (((day_of_week >= 0) AND (day_of_week <= 6)))
);


--
-- Name: employees; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.employees (
    id uuid DEFAULT uuidv7() NOT NULL,
    company_id uuid NOT NULL,
    employee_number character varying(50) NOT NULL,
    full_name character varying(255) NOT NULL,
    ic_number character varying(20),
    passport_number character varying(50),
    date_of_birth date,
    gender public.gender_type,
    nationality character varying(50) DEFAULT 'Malaysian'::character varying,
    race public.race_type,
    residency_status public.residency_status DEFAULT 'citizen'::public.residency_status NOT NULL,
    marital_status public.marital_status DEFAULT 'single'::public.marital_status,
    email character varying(255),
    phone character varying(20),
    address_line1 character varying(255),
    address_line2 character varying(255),
    city character varying(100),
    state character varying(50),
    postcode character varying(10),
    department character varying(100),
    designation character varying(100),
    cost_centre character varying(50),
    branch character varying(100),
    employment_type public.employment_type DEFAULT 'permanent'::public.employment_type NOT NULL,
    date_joined date NOT NULL,
    probation_start date,
    probation_end date,
    confirmation_date date,
    date_resigned date,
    resignation_reason character varying(500),
    basic_salary bigint DEFAULT 0 NOT NULL,
    hourly_rate bigint,
    daily_rate bigint,
    bank_name character varying(100),
    bank_account_number character varying(50),
    bank_account_type character varying(20) DEFAULT 'savings'::character varying,
    tax_identification_number character varying(50),
    epf_number character varying(50),
    socso_number character varying(50),
    eis_number character varying(50),
    working_spouse boolean DEFAULT false,
    num_children integer DEFAULT 0,
    epf_category character(1) DEFAULT 'A'::bpchar,
    is_muslim boolean DEFAULT false,
    zakat_eligible boolean DEFAULT false,
    zakat_monthly_amount bigint DEFAULT 0,
    ptptn_monthly_amount bigint DEFAULT 0,
    tabung_haji_amount bigint DEFAULT 0,
    hrdf_contribution boolean DEFAULT true,
    payroll_group_id uuid,
    salary_group character varying(50) DEFAULT 'standard'::character varying,
    is_active boolean DEFAULT true,
    deleted_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    created_by uuid,
    updated_by uuid
);


--
-- Name: epf_rates; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.epf_rates (
    id uuid DEFAULT uuidv7() NOT NULL,
    wage_from bigint NOT NULL,
    wage_to bigint NOT NULL,
    employee_contribution bigint NOT NULL,
    employer_contribution bigint NOT NULL,
    category character(1) DEFAULT 'A'::bpchar NOT NULL,
    effective_from date NOT NULL,
    effective_to date,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT epf_rates_contributions_check CHECK (((employee_contribution >= 0) AND (employer_contribution >= 0))),
    CONSTRAINT epf_rates_effective_dates_check CHECK (((effective_to IS NULL) OR (effective_to >= effective_from))),
    CONSTRAINT epf_rates_range_check CHECK (((wage_from >= 0) AND (wage_to >= wage_from)))
);


--
-- Name: holidays; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.holidays (
    id uuid DEFAULT uuidv7() NOT NULL,
    company_id uuid NOT NULL,
    name character varying(255) NOT NULL,
    date date NOT NULL,
    holiday_type character varying(50) DEFAULT 'public_holiday'::character varying NOT NULL,
    description text,
    is_recurring boolean DEFAULT false NOT NULL,
    state character varying(100),
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    created_by uuid,
    updated_by uuid,
    CONSTRAINT holidays_holiday_type_check CHECK (((holiday_type)::text = ANY (ARRAY[('public_holiday'::character varying)::text, ('company_holiday'::character varying)::text, ('replacement_leave'::character varying)::text, ('state_holiday'::character varying)::text])))
);


--
-- Name: leave_balances; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.leave_balances (
    id uuid DEFAULT uuidv7() NOT NULL,
    employee_id uuid NOT NULL,
    leave_type_id uuid NOT NULL,
    year integer NOT NULL,
    entitled_days numeric(5,1) DEFAULT 0 NOT NULL,
    taken_days numeric(5,1) DEFAULT 0 NOT NULL,
    pending_days numeric(5,1) DEFAULT 0 NOT NULL,
    carried_forward numeric(5,1) DEFAULT 0 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: leave_requests; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.leave_requests (
    id uuid DEFAULT uuidv7() NOT NULL,
    employee_id uuid NOT NULL,
    company_id uuid NOT NULL,
    leave_type_id uuid NOT NULL,
    start_date date NOT NULL,
    end_date date NOT NULL,
    days numeric(5,1) NOT NULL,
    reason text,
    status character varying(20) DEFAULT 'pending'::character varying NOT NULL,
    reviewed_by uuid,
    reviewed_at timestamp with time zone,
    review_notes text,
    attachment_url character varying(500),
    attachment_name character varying(255),
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT leave_requests_dates_check CHECK ((start_date <= end_date)),
    CONSTRAINT leave_requests_days_check CHECK ((days > (0)::numeric)),
    CONSTRAINT leave_requests_status_check CHECK (((status)::text = ANY (ARRAY[('pending'::character varying)::text, ('approved'::character varying)::text, ('rejected'::character varying)::text, ('cancelled'::character varying)::text])))
);


--
-- Name: leave_types; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.leave_types (
    id uuid DEFAULT uuidv7() NOT NULL,
    company_id uuid NOT NULL,
    name character varying(100) NOT NULL,
    description text,
    default_days numeric(5,1) DEFAULT 0 NOT NULL,
    is_paid boolean DEFAULT true NOT NULL,
    is_active boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    max_carry_forward numeric(5,1) DEFAULT 0 NOT NULL,
    carry_forward_expiry_months integer DEFAULT 3 NOT NULL,
    is_system boolean DEFAULT false NOT NULL
);


--
-- Name: notifications; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.notifications (
    id uuid DEFAULT uuidv7() NOT NULL,
    user_id uuid NOT NULL,
    company_id uuid NOT NULL,
    notification_type character varying(50) NOT NULL,
    title character varying(255) NOT NULL,
    message text NOT NULL,
    entity_type character varying(50),
    entity_id uuid,
    is_read boolean DEFAULT false NOT NULL,
    read_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: oauth2_accounts; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.oauth2_accounts (
    id uuid DEFAULT uuidv7() NOT NULL,
    user_id uuid NOT NULL,
    provider character varying(50) NOT NULL,
    provider_user_id character varying(255) NOT NULL,
    provider_email character varying(255),
    provider_name character varying(255),
    avatar_url text,
    access_token_hash character varying(255),
    refresh_token_hash character varying(255),
    token_expires_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: oauth2_states; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.oauth2_states (
    state character varying(255) NOT NULL,
    code_verifier character varying(128) NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: overtime_applications; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.overtime_applications (
    id uuid DEFAULT uuidv7() NOT NULL,
    employee_id uuid NOT NULL,
    company_id uuid NOT NULL,
    ot_date date NOT NULL,
    start_time time without time zone NOT NULL,
    end_time time without time zone NOT NULL,
    hours numeric(5,2) NOT NULL,
    ot_type character varying(20) DEFAULT 'normal'::character varying NOT NULL,
    reason character varying(500),
    status character varying(20) DEFAULT 'pending'::character varying NOT NULL,
    reviewed_by uuid,
    reviewed_at timestamp with time zone,
    review_notes character varying(500),
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT overtime_applications_ot_type_check CHECK (((ot_type)::text = ANY (ARRAY[('normal'::character varying)::text, ('rest_day'::character varying)::text, ('public_holiday'::character varying)::text]))),
    CONSTRAINT overtime_applications_status_check CHECK (((status)::text = ANY (ARRAY[('pending'::character varying)::text, ('approved'::character varying)::text, ('rejected'::character varying)::text, ('cancelled'::character varying)::text])))
);


--
-- Name: passkey_challenges; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.passkey_challenges (
    id uuid DEFAULT uuidv7() NOT NULL,
    user_id uuid,
    challenge_type character varying(20) NOT NULL,
    state_json jsonb NOT NULL,
    email character varying(255),
    expires_at timestamp with time zone DEFAULT (now() + '00:05:00'::interval) NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: passkey_credentials; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.passkey_credentials (
    id uuid DEFAULT uuidv7() NOT NULL,
    user_id uuid NOT NULL,
    credential_name character varying(255) DEFAULT 'My Passkey'::character varying NOT NULL,
    credential_json jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    last_used_at timestamp with time zone
);


--
-- Name: password_reset_requests; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.password_reset_requests (
    id uuid DEFAULT uuidv7() NOT NULL,
    user_id uuid NOT NULL,
    status character varying(20) DEFAULT 'pending'::character varying NOT NULL,
    requested_at timestamp with time zone DEFAULT now() NOT NULL,
    reviewed_by uuid,
    reviewed_at timestamp with time zone,
    reset_token_hash character varying(255),
    reset_token_expires_at timestamp with time zone,
    completed_at timestamp with time zone,
    CONSTRAINT password_reset_requests_status_check CHECK (((status)::text = ANY (ARRAY[('pending'::character varying)::text, ('approved'::character varying)::text, ('rejected'::character varying)::text, ('completed'::character varying)::text, ('expired'::character varying)::text])))
);


--
-- Name: payroll_entries; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.payroll_entries (
    id uuid DEFAULT uuidv7() NOT NULL,
    employee_id uuid NOT NULL,
    company_id uuid NOT NULL,
    period_year integer NOT NULL,
    period_month integer NOT NULL,
    category character varying(20) NOT NULL,
    item_type character varying(50) NOT NULL,
    description character varying(255) NOT NULL,
    amount bigint NOT NULL,
    quantity numeric(10,2),
    rate bigint,
    is_taxable boolean DEFAULT true,
    is_processed boolean DEFAULT false,
    payroll_run_id uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    created_by uuid,
    updated_by uuid,
    CONSTRAINT payroll_entries_category_check CHECK (((category)::text = ANY (ARRAY[('earning'::character varying)::text, ('deduction'::character varying)::text])))
);


--
-- Name: payroll_groups; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.payroll_groups (
    id uuid DEFAULT uuidv7() NOT NULL,
    company_id uuid NOT NULL,
    name character varying(100) NOT NULL,
    description character varying(255),
    cutoff_day integer DEFAULT 25 NOT NULL,
    payment_day integer DEFAULT 28 NOT NULL,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    created_by uuid,
    updated_by uuid,
    CONSTRAINT payroll_groups_days_check CHECK ((((cutoff_day >= 1) AND (cutoff_day <= 31)) AND ((payment_day >= 1) AND (payment_day <= 31))))
);


--
-- Name: payroll_item_details; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.payroll_item_details (
    id uuid DEFAULT uuidv7() NOT NULL,
    payroll_item_id uuid NOT NULL,
    category character varying(20) NOT NULL,
    item_type character varying(50) NOT NULL,
    description character varying(255) NOT NULL,
    amount bigint NOT NULL,
    is_taxable boolean DEFAULT true,
    is_statutory boolean DEFAULT false,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT payroll_item_details_category_check CHECK (((category)::text = ANY (ARRAY[('earning'::character varying)::text, ('deduction'::character varying)::text])))
);


--
-- Name: payroll_items; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.payroll_items (
    id uuid DEFAULT uuidv7() NOT NULL,
    payroll_run_id uuid NOT NULL,
    employee_id uuid NOT NULL,
    basic_salary bigint DEFAULT 0 NOT NULL,
    gross_salary bigint DEFAULT 0 NOT NULL,
    total_allowances bigint DEFAULT 0 NOT NULL,
    total_overtime bigint DEFAULT 0 NOT NULL,
    total_bonus bigint DEFAULT 0 NOT NULL,
    total_commission bigint DEFAULT 0 NOT NULL,
    total_claims bigint DEFAULT 0 NOT NULL,
    epf_employee bigint DEFAULT 0 NOT NULL,
    epf_employer bigint DEFAULT 0 NOT NULL,
    socso_employee bigint DEFAULT 0 NOT NULL,
    socso_employer bigint DEFAULT 0 NOT NULL,
    eis_employee bigint DEFAULT 0 NOT NULL,
    eis_employer bigint DEFAULT 0 NOT NULL,
    pcb_amount bigint DEFAULT 0 NOT NULL,
    zakat_amount bigint DEFAULT 0 NOT NULL,
    ptptn_amount bigint DEFAULT 0 NOT NULL,
    tabung_haji_amount bigint DEFAULT 0 NOT NULL,
    total_loan_deductions bigint DEFAULT 0 NOT NULL,
    total_other_deductions bigint DEFAULT 0 NOT NULL,
    unpaid_leave_deduction bigint DEFAULT 0 NOT NULL,
    unpaid_leave_days numeric(5,2) DEFAULT 0 NOT NULL,
    total_deductions bigint DEFAULT 0 NOT NULL,
    net_salary bigint DEFAULT 0 NOT NULL,
    employer_cost bigint DEFAULT 0 NOT NULL,
    ytd_gross bigint DEFAULT 0 NOT NULL,
    ytd_epf_employee bigint DEFAULT 0 NOT NULL,
    ytd_pcb bigint DEFAULT 0 NOT NULL,
    ytd_socso_employee bigint DEFAULT 0 NOT NULL,
    ytd_eis_employee bigint DEFAULT 0 NOT NULL,
    ytd_zakat bigint DEFAULT 0 NOT NULL,
    ytd_net bigint DEFAULT 0 NOT NULL,
    working_days integer,
    days_worked numeric(5,2),
    is_prorated boolean DEFAULT false,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: payroll_runs; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.payroll_runs (
    id uuid DEFAULT uuidv7() NOT NULL,
    company_id uuid NOT NULL,
    payroll_group_id uuid NOT NULL,
    period_year integer NOT NULL,
    period_month integer NOT NULL,
    period_start date NOT NULL,
    period_end date NOT NULL,
    pay_date date NOT NULL,
    status public.payroll_status DEFAULT 'draft'::public.payroll_status NOT NULL,
    total_gross bigint DEFAULT 0 NOT NULL,
    total_net bigint DEFAULT 0 NOT NULL,
    total_employer_cost bigint DEFAULT 0 NOT NULL,
    total_epf_employee bigint DEFAULT 0 NOT NULL,
    total_epf_employer bigint DEFAULT 0 NOT NULL,
    total_socso_employee bigint DEFAULT 0 NOT NULL,
    total_socso_employer bigint DEFAULT 0 NOT NULL,
    total_eis_employee bigint DEFAULT 0 NOT NULL,
    total_eis_employer bigint DEFAULT 0 NOT NULL,
    total_pcb bigint DEFAULT 0 NOT NULL,
    total_zakat bigint DEFAULT 0 NOT NULL,
    employee_count integer DEFAULT 0 NOT NULL,
    version integer DEFAULT 1 NOT NULL,
    processed_by uuid,
    processed_at timestamp with time zone,
    approved_by uuid,
    approved_at timestamp with time zone,
    locked_at timestamp with time zone,
    locked_by uuid,
    notes text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    created_by uuid,
    updated_by uuid,
    CONSTRAINT payroll_runs_period_check CHECK ((((period_month >= 1) AND (period_month <= 12)) AND (period_start <= period_end))),
    CONSTRAINT payroll_runs_version_count_check CHECK (((version > 0) AND (employee_count >= 0)))
);


--
-- Name: pcb_brackets; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.pcb_brackets (
    id uuid DEFAULT uuidv7() NOT NULL,
    chargeable_income_from bigint NOT NULL,
    chargeable_income_to bigint NOT NULL,
    tax_rate_percent numeric(5,2) NOT NULL,
    cumulative_tax bigint NOT NULL,
    effective_year integer NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT pcb_brackets_range_check CHECK (((chargeable_income_from >= 0) AND (chargeable_income_to >= chargeable_income_from))),
    CONSTRAINT pcb_brackets_rate_check CHECK ((((tax_rate_percent >= (0)::numeric) AND (tax_rate_percent <= (100)::numeric)) AND (cumulative_tax >= 0))),
    CONSTRAINT pcb_brackets_year_check CHECK (((effective_year >= 2000) AND (effective_year <= 2200)))
);


--
-- Name: pcb_reliefs; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.pcb_reliefs (
    id uuid DEFAULT uuidv7() NOT NULL,
    relief_type character varying(50) NOT NULL,
    amount bigint NOT NULL,
    effective_year integer NOT NULL,
    description character varying(255),
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT pcb_reliefs_amount_year_check CHECK (((amount >= 0) AND ((effective_year >= 2000) AND (effective_year <= 2200))))
);


--
-- Name: platform_settings; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.platform_settings (
    key character varying(100) NOT NULL,
    value text NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_by uuid
);


--
-- Name: refresh_tokens; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.refresh_tokens (
    id uuid DEFAULT uuidv7() NOT NULL,
    user_id uuid NOT NULL,
    token_hash character varying(255) NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    revoked boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: salary_history; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.salary_history (
    id uuid DEFAULT uuidv7() NOT NULL,
    employee_id uuid NOT NULL,
    old_salary bigint NOT NULL,
    new_salary bigint NOT NULL,
    effective_date date NOT NULL,
    reason character varying(255),
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    created_by uuid
);


--
-- Name: socso_rates; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.socso_rates (
    id uuid DEFAULT uuidv7() NOT NULL,
    wage_from bigint NOT NULL,
    wage_to bigint NOT NULL,
    first_cat_employee bigint NOT NULL,
    first_cat_employer bigint NOT NULL,
    second_cat_employer bigint NOT NULL,
    effective_from date NOT NULL,
    effective_to date,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT socso_rates_contributions_check CHECK (((first_cat_employee >= 0) AND (first_cat_employer >= 0) AND (second_cat_employer >= 0))),
    CONSTRAINT socso_rates_effective_dates_check CHECK (((effective_to IS NULL) OR (effective_to >= effective_from))),
    CONSTRAINT socso_rates_range_check CHECK (((wage_from >= 0) AND (wage_to >= wage_from)))
);


--
-- Name: team_members; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.team_members (
    id uuid DEFAULT uuidv7() NOT NULL,
    team_id uuid NOT NULL,
    employee_id uuid NOT NULL,
    role character varying(20) DEFAULT 'member'::character varying NOT NULL,
    joined_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT team_members_role_check CHECK (((role)::text = ANY (ARRAY[('member'::character varying)::text, ('lead'::character varying)::text])))
);


--
-- Name: teams; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.teams (
    id uuid DEFAULT uuidv7() NOT NULL,
    company_id uuid NOT NULL,
    name character varying(100) NOT NULL,
    description text,
    tag character varying(50) DEFAULT 'general'::character varying NOT NULL,
    is_active boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    created_by uuid,
    updated_by uuid
);


--
-- Name: tp3_records; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.tp3_records (
    id uuid DEFAULT uuidv7() NOT NULL,
    employee_id uuid NOT NULL,
    tax_year integer NOT NULL,
    previous_employer_name character varying(255),
    previous_income_ytd bigint DEFAULT 0 NOT NULL,
    previous_epf_ytd bigint DEFAULT 0 NOT NULL,
    previous_pcb_ytd bigint DEFAULT 0 NOT NULL,
    previous_socso_ytd bigint DEFAULT 0 NOT NULL,
    previous_zakat_ytd bigint DEFAULT 0 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    created_by uuid
);


--
-- Name: user_companies; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.user_companies (
    user_id uuid NOT NULL,
    company_id uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: users; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.users (
    id uuid DEFAULT uuidv7() NOT NULL,
    email character varying(255) NOT NULL,
    password_hash character varying(255) NOT NULL,
    full_name character varying(255) NOT NULL,
    company_id uuid,
    employee_id uuid,
    is_active boolean DEFAULT true,
    last_login timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    must_change_password boolean DEFAULT false NOT NULL,
    roles character varying(50)[] DEFAULT ARRAY['employee'::character varying(50)] NOT NULL,
    deleted_at timestamp with time zone,
    deleted_by uuid,
    CONSTRAINT users_roles_valid CHECK (((cardinality(roles) >= 1) AND (roles <@ ARRAY['super_admin'::character varying(50), 'admin'::character varying(50), 'payroll_admin'::character varying(50), 'hr_manager'::character varying(50), 'finance'::character varying(50), 'exec'::character varying(50), 'employee'::character varying(50)])))
);


--
-- Name: working_day_config; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.working_day_config (
    id uuid DEFAULT uuidv7() NOT NULL,
    company_id uuid NOT NULL,
    day_of_week smallint NOT NULL,
    is_working_day boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT working_day_config_day_of_week_check CHECK (((day_of_week >= 0) AND (day_of_week <= 6)))
);


--
-- Name: attendance_kiosk_credentials attendance_kiosk_credentials_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.attendance_kiosk_credentials
    ADD CONSTRAINT attendance_kiosk_credentials_pkey PRIMARY KEY (id);


--
-- Name: attendance_qr_tokens attendance_qr_tokens_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.attendance_qr_tokens
    ADD CONSTRAINT attendance_qr_tokens_pkey PRIMARY KEY (id);


--
-- Name: attendance_qr_tokens attendance_qr_tokens_token_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.attendance_qr_tokens
    ADD CONSTRAINT attendance_qr_tokens_token_key UNIQUE (token);


--
-- Name: attendance_records attendance_records_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.attendance_records
    ADD CONSTRAINT attendance_records_pkey PRIMARY KEY (id);


--
-- Name: audit_logs audit_logs_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.audit_logs
    ADD CONSTRAINT audit_logs_pkey PRIMARY KEY (id);


--
-- Name: bulk_import_sessions bulk_import_sessions_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.bulk_import_sessions
    ADD CONSTRAINT bulk_import_sessions_pkey PRIMARY KEY (id);


--
-- Name: claims claims_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.claims
    ADD CONSTRAINT claims_pkey PRIMARY KEY (id);


--
-- Name: companies companies_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.companies
    ADD CONSTRAINT companies_pkey PRIMARY KEY (id);


--
-- Name: company_locations company_locations_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.company_locations
    ADD CONSTRAINT company_locations_pkey PRIMARY KEY (id);


--
-- Name: company_settings company_settings_company_id_category_key_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.company_settings
    ADD CONSTRAINT company_settings_company_id_category_key_key UNIQUE (company_id, category, key);


--
-- Name: company_settings company_settings_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.company_settings
    ADD CONSTRAINT company_settings_pkey PRIMARY KEY (id);


--
-- Name: company_work_schedules company_work_schedules_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.company_work_schedules
    ADD CONSTRAINT company_work_schedules_pkey PRIMARY KEY (id);


--
-- Name: document_categories document_categories_company_id_name_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.document_categories
    ADD CONSTRAINT document_categories_company_id_name_key UNIQUE (company_id, name);


--
-- Name: document_categories document_categories_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.document_categories
    ADD CONSTRAINT document_categories_pkey PRIMARY KEY (id);


--
-- Name: documents documents_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.documents
    ADD CONSTRAINT documents_pkey PRIMARY KEY (id);


--
-- Name: eis_rates eis_rates_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.eis_rates
    ADD CONSTRAINT eis_rates_pkey PRIMARY KEY (id);


--
-- Name: email_logs email_logs_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.email_logs
    ADD CONSTRAINT email_logs_pkey PRIMARY KEY (id);


--
-- Name: email_templates email_templates_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.email_templates
    ADD CONSTRAINT email_templates_pkey PRIMARY KEY (id);


--
-- Name: employee_allowances employee_allowances_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.employee_allowances
    ADD CONSTRAINT employee_allowances_pkey PRIMARY KEY (id);


--
-- Name: employee_work_schedules employee_work_schedules_employee_id_day_of_week_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.employee_work_schedules
    ADD CONSTRAINT employee_work_schedules_employee_id_day_of_week_key UNIQUE (employee_id, day_of_week);


--
-- Name: employee_work_schedules employee_work_schedules_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.employee_work_schedules
    ADD CONSTRAINT employee_work_schedules_pkey PRIMARY KEY (id);


--
-- Name: employees employees_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.employees
    ADD CONSTRAINT employees_pkey PRIMARY KEY (id);


--
-- Name: epf_rates epf_rates_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.epf_rates
    ADD CONSTRAINT epf_rates_pkey PRIMARY KEY (id);


--
-- Name: holidays holidays_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.holidays
    ADD CONSTRAINT holidays_pkey PRIMARY KEY (id);


--
-- Name: leave_balances leave_balances_employee_id_leave_type_id_year_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.leave_balances
    ADD CONSTRAINT leave_balances_employee_id_leave_type_id_year_key UNIQUE (employee_id, leave_type_id, year);


--
-- Name: leave_balances leave_balances_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.leave_balances
    ADD CONSTRAINT leave_balances_pkey PRIMARY KEY (id);


--
-- Name: leave_requests leave_requests_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.leave_requests
    ADD CONSTRAINT leave_requests_pkey PRIMARY KEY (id);


--
-- Name: leave_types leave_types_company_id_name_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.leave_types
    ADD CONSTRAINT leave_types_company_id_name_key UNIQUE (company_id, name);


--
-- Name: leave_types leave_types_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.leave_types
    ADD CONSTRAINT leave_types_pkey PRIMARY KEY (id);


--
-- Name: notifications notifications_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.notifications
    ADD CONSTRAINT notifications_pkey PRIMARY KEY (id);


--
-- Name: oauth2_accounts oauth2_accounts_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.oauth2_accounts
    ADD CONSTRAINT oauth2_accounts_pkey PRIMARY KEY (id);


--
-- Name: oauth2_accounts oauth2_accounts_provider_provider_user_id_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.oauth2_accounts
    ADD CONSTRAINT oauth2_accounts_provider_provider_user_id_key UNIQUE (provider, provider_user_id);


--
-- Name: oauth2_states oauth2_states_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.oauth2_states
    ADD CONSTRAINT oauth2_states_pkey PRIMARY KEY (state);


--
-- Name: overtime_applications overtime_applications_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.overtime_applications
    ADD CONSTRAINT overtime_applications_pkey PRIMARY KEY (id);


--
-- Name: passkey_challenges passkey_challenges_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.passkey_challenges
    ADD CONSTRAINT passkey_challenges_pkey PRIMARY KEY (id);


--
-- Name: passkey_credentials passkey_credentials_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.passkey_credentials
    ADD CONSTRAINT passkey_credentials_pkey PRIMARY KEY (id);


--
-- Name: password_reset_requests password_reset_requests_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.password_reset_requests
    ADD CONSTRAINT password_reset_requests_pkey PRIMARY KEY (id);


--
-- Name: payroll_entries payroll_entries_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.payroll_entries
    ADD CONSTRAINT payroll_entries_pkey PRIMARY KEY (id);


--
-- Name: payroll_groups payroll_groups_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.payroll_groups
    ADD CONSTRAINT payroll_groups_pkey PRIMARY KEY (id);


--
-- Name: payroll_item_details payroll_item_details_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.payroll_item_details
    ADD CONSTRAINT payroll_item_details_pkey PRIMARY KEY (id);


--
-- Name: payroll_items payroll_items_payroll_run_id_employee_id_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.payroll_items
    ADD CONSTRAINT payroll_items_payroll_run_id_employee_id_key UNIQUE (payroll_run_id, employee_id);


--
-- Name: payroll_items payroll_items_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.payroll_items
    ADD CONSTRAINT payroll_items_pkey PRIMARY KEY (id);


--
-- Name: payroll_runs payroll_runs_company_id_payroll_group_id_period_year_period_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.payroll_runs
    ADD CONSTRAINT payroll_runs_company_id_payroll_group_id_period_year_period_key UNIQUE (company_id, payroll_group_id, period_year, period_month, version);


--
-- Name: payroll_runs payroll_runs_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.payroll_runs
    ADD CONSTRAINT payroll_runs_pkey PRIMARY KEY (id);


--
-- Name: pcb_brackets pcb_brackets_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.pcb_brackets
    ADD CONSTRAINT pcb_brackets_pkey PRIMARY KEY (id);


--
-- Name: pcb_reliefs pcb_reliefs_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.pcb_reliefs
    ADD CONSTRAINT pcb_reliefs_pkey PRIMARY KEY (id);


--
-- Name: platform_settings platform_settings_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.platform_settings
    ADD CONSTRAINT platform_settings_pkey PRIMARY KEY (key);


--
-- Name: refresh_tokens refresh_tokens_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.refresh_tokens
    ADD CONSTRAINT refresh_tokens_pkey PRIMARY KEY (id);


--
-- Name: refresh_tokens refresh_tokens_token_hash_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.refresh_tokens
    ADD CONSTRAINT refresh_tokens_token_hash_key UNIQUE (token_hash);


--
-- Name: salary_history salary_history_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.salary_history
    ADD CONSTRAINT salary_history_pkey PRIMARY KEY (id);


--
-- Name: socso_rates socso_rates_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.socso_rates
    ADD CONSTRAINT socso_rates_pkey PRIMARY KEY (id);


--
-- Name: team_members team_members_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.team_members
    ADD CONSTRAINT team_members_pkey PRIMARY KEY (id);


--
-- Name: team_members team_members_team_id_employee_id_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.team_members
    ADD CONSTRAINT team_members_team_id_employee_id_key UNIQUE (team_id, employee_id);


--
-- Name: teams teams_company_id_name_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.teams
    ADD CONSTRAINT teams_company_id_name_key UNIQUE (company_id, name);


--
-- Name: teams teams_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.teams
    ADD CONSTRAINT teams_pkey PRIMARY KEY (id);


--
-- Name: tp3_records tp3_records_employee_id_tax_year_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.tp3_records
    ADD CONSTRAINT tp3_records_employee_id_tax_year_key UNIQUE (employee_id, tax_year);


--
-- Name: tp3_records tp3_records_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.tp3_records
    ADD CONSTRAINT tp3_records_pkey PRIMARY KEY (id);


--
-- Name: user_companies user_companies_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.user_companies
    ADD CONSTRAINT user_companies_pkey PRIMARY KEY (user_id, company_id);


--
-- Name: users users_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_pkey PRIMARY KEY (id);


--
-- Name: working_day_config working_day_config_company_id_day_of_week_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.working_day_config
    ADD CONSTRAINT working_day_config_company_id_day_of_week_key UNIQUE (company_id, day_of_week);


--
-- Name: working_day_config working_day_config_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.working_day_config
    ADD CONSTRAINT working_day_config_pkey PRIMARY KEY (id);


--
-- Name: attendance_one_open_per_employee; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX attendance_one_open_per_employee ON public.attendance_records USING btree (employee_id) WHERE (check_out_at IS NULL);


--
-- Name: eis_rates_natural_key; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX eis_rates_natural_key ON public.eis_rates USING btree (effective_from, wage_from, wage_to);


--
-- Name: employees_company_employee_number_active; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX employees_company_employee_number_active ON public.employees USING btree (company_id, employee_number) WHERE (deleted_at IS NULL);


--
-- Name: epf_rates_natural_key; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX epf_rates_natural_key ON public.epf_rates USING btree (category, effective_from, wage_from, wage_to);


--
-- Name: idx_attendance_company_date; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_attendance_company_date ON public.attendance_records USING btree (company_id, check_in_at DESC);


--
-- Name: idx_attendance_employee_recent; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_attendance_employee_recent ON public.attendance_records USING btree (employee_id, check_in_at DESC);


--
-- Name: idx_attendance_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_attendance_status ON public.attendance_records USING btree (company_id, status);


--
-- Name: idx_audit_logs_action; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_audit_logs_action ON public.audit_logs USING btree (action);


--
-- Name: idx_audit_logs_company_created; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_audit_logs_company_created ON public.audit_logs USING btree (company_id, created_at DESC);


--
-- Name: idx_audit_logs_date_range; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_audit_logs_date_range ON public.audit_logs USING btree (created_at DESC, entity_type);


--
-- Name: idx_audit_logs_entity; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_audit_logs_entity ON public.audit_logs USING btree (entity_type, entity_id);


--
-- Name: idx_audit_logs_user; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_audit_logs_user ON public.audit_logs USING btree (user_id);


--
-- Name: idx_bulk_import_sessions_company; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_bulk_import_sessions_company ON public.bulk_import_sessions USING btree (company_id);


--
-- Name: idx_bulk_import_sessions_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_bulk_import_sessions_status ON public.bulk_import_sessions USING btree (status);


--
-- Name: idx_claims_approved_payroll; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_claims_approved_payroll ON public.claims USING btree (company_id, employee_id, expense_date) WHERE ((status)::text = 'approved'::text);


--
-- Name: idx_claims_employee; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_claims_employee ON public.claims USING btree (employee_id);


--
-- Name: idx_claims_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_claims_status ON public.claims USING btree (company_id, status);


--
-- Name: idx_companies_name_unique; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_companies_name_unique ON public.companies USING btree (lower((name)::text));


--
-- Name: idx_company_locations_company; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_company_locations_company ON public.company_locations USING btree (company_id);


--
-- Name: idx_documents_category; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_documents_category ON public.documents USING btree (category_id);


--
-- Name: idx_documents_company; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_documents_company ON public.documents USING btree (company_id);


--
-- Name: idx_documents_employee; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_documents_employee ON public.documents USING btree (employee_id);


--
-- Name: idx_documents_expiry; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_documents_expiry ON public.documents USING btree (expiry_date) WHERE (expiry_date IS NOT NULL);


--
-- Name: idx_email_logs_employee; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_email_logs_employee ON public.email_logs USING btree (company_id, employee_id);


--
-- Name: idx_email_logs_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_email_logs_status ON public.email_logs USING btree (status);


--
-- Name: idx_email_templates_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_email_templates_type ON public.email_templates USING btree (company_id, letter_type);


--
-- Name: idx_employee_allowances; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_employee_allowances ON public.employee_allowances USING btree (employee_id, is_active);


--
-- Name: idx_employees_company; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_employees_company ON public.employees USING btree (company_id);


--
-- Name: idx_employees_payroll_active; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_employees_payroll_active ON public.employees USING btree (company_id, payroll_group_id, date_joined) WHERE ((is_active = true) AND (deleted_at IS NULL));


--
-- Name: idx_employees_payroll_group; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_employees_payroll_group ON public.employees USING btree (payroll_group_id);


--
-- Name: idx_employees_search_trgm; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_employees_search_trgm ON public.employees USING gin (lower((((full_name)::text || ' '::text) || (employee_number)::text)) public.gin_trgm_ops) WHERE (deleted_at IS NULL);


--
-- Name: idx_holidays_company_date; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_holidays_company_date ON public.holidays USING btree (company_id, date);


--
-- Name: idx_holidays_company_year; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_holidays_company_year ON public.holidays USING btree (company_id, EXTRACT(year FROM date));


--
-- Name: idx_kiosk_credentials_company_active; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_kiosk_credentials_company_active ON public.attendance_kiosk_credentials USING btree (company_id) WHERE (revoked_at IS NULL);


--
-- Name: idx_kiosk_credentials_token_hash; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_kiosk_credentials_token_hash ON public.attendance_kiosk_credentials USING btree (token_hash);


--
-- Name: idx_leave_requests_approved_dates; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_leave_requests_approved_dates ON public.leave_requests USING btree (employee_id, start_date, end_date) WHERE ((status)::text = 'approved'::text);


--
-- Name: idx_leave_requests_employee; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_leave_requests_employee ON public.leave_requests USING btree (employee_id);


--
-- Name: idx_leave_requests_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_leave_requests_status ON public.leave_requests USING btree (company_id, status);


--
-- Name: idx_notifications_company; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_notifications_company ON public.notifications USING btree (company_id);


--
-- Name: idx_notifications_created; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_notifications_created ON public.notifications USING btree (created_at DESC);


--
-- Name: idx_notifications_unread_recent; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_notifications_unread_recent ON public.notifications USING btree (user_id, created_at DESC) WHERE (is_read = false);


--
-- Name: idx_notifications_user; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_notifications_user ON public.notifications USING btree (user_id, is_read);


--
-- Name: idx_oauth2_accounts_user; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_oauth2_accounts_user ON public.oauth2_accounts USING btree (user_id);


--
-- Name: idx_oauth2_states_expires; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_oauth2_states_expires ON public.oauth2_states USING btree (expires_at);


--
-- Name: idx_overtime_applications_company; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_overtime_applications_company ON public.overtime_applications USING btree (company_id, status);


--
-- Name: idx_overtime_applications_employee; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_overtime_applications_employee ON public.overtime_applications USING btree (employee_id, status);


--
-- Name: idx_overtime_approved_payroll; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_overtime_approved_payroll ON public.overtime_applications USING btree (employee_id, ot_date) INCLUDE (ot_type, hours) WHERE ((status)::text = 'approved'::text);


--
-- Name: idx_passkey_challenges_email; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_passkey_challenges_email ON public.passkey_challenges USING btree (email);


--
-- Name: idx_passkey_challenges_user; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_passkey_challenges_user ON public.passkey_challenges USING btree (user_id);


--
-- Name: idx_passkey_credentials_user; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_passkey_credentials_user ON public.passkey_credentials USING btree (user_id);


--
-- Name: idx_password_reset_requests_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_password_reset_requests_status ON public.password_reset_requests USING btree (status);


--
-- Name: idx_password_reset_requests_token; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_password_reset_requests_token ON public.password_reset_requests USING btree (reset_token_hash);


--
-- Name: idx_password_reset_requests_user; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_password_reset_requests_user ON public.password_reset_requests USING btree (user_id);


--
-- Name: idx_payroll_entries; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_payroll_entries ON public.payroll_entries USING btree (employee_id, period_year, period_month);


--
-- Name: idx_payroll_entries_run_pending; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_payroll_entries_run_pending ON public.payroll_entries USING btree (payroll_run_id) WHERE (payroll_run_id IS NOT NULL);


--
-- Name: idx_payroll_item_details; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_payroll_item_details ON public.payroll_item_details USING btree (payroll_item_id);


--
-- Name: idx_payroll_items_employee; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_payroll_items_employee ON public.payroll_items USING btree (employee_id);


--
-- Name: idx_payroll_runs_company_period; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_payroll_runs_company_period ON public.payroll_runs USING btree (company_id, period_year DESC, period_month DESC, created_at DESC);


--
-- Name: idx_payroll_runs_period; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_payroll_runs_period ON public.payroll_runs USING btree (period_year, period_month);


--
-- Name: idx_qr_tokens_company_expires; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_qr_tokens_company_expires ON public.attendance_qr_tokens USING btree (company_id, expires_at);


--
-- Name: idx_refresh_tokens_user; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_refresh_tokens_user ON public.refresh_tokens USING btree (user_id);


--
-- Name: idx_salary_history_employee; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_salary_history_employee ON public.salary_history USING btree (employee_id);


--
-- Name: idx_team_members_employee; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_team_members_employee ON public.team_members USING btree (employee_id);


--
-- Name: idx_user_companies_company; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_user_companies_company ON public.user_companies USING btree (company_id);


--
-- Name: idx_users_company_active; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_users_company_active ON public.users USING btree (company_id, created_at DESC) WHERE ((is_active = true) AND (deleted_at IS NULL));


--
-- Name: idx_users_roles_gin; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_users_roles_gin ON public.users USING gin (roles);


--
-- Name: idx_work_schedules_company; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_work_schedules_company ON public.company_work_schedules USING btree (company_id);


--
-- Name: idx_work_schedules_default; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_work_schedules_default ON public.company_work_schedules USING btree (company_id) WHERE (is_default = true);


--
-- Name: payroll_runs_one_active_period; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX payroll_runs_one_active_period ON public.payroll_runs USING btree (company_id, payroll_group_id, period_year, period_month) WHERE (status <> 'cancelled'::public.payroll_status);


--
-- Name: payroll_groups_company_name_key; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX payroll_groups_company_name_key ON public.payroll_groups USING btree (company_id, lower(btrim((name)::text)));


--
-- Name: pcb_brackets_natural_key; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX pcb_brackets_natural_key ON public.pcb_brackets USING btree (effective_year, chargeable_income_from, chargeable_income_to);


--
-- Name: pcb_reliefs_natural_key; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX pcb_reliefs_natural_key ON public.pcb_reliefs USING btree (effective_year, relief_type);


--
-- Name: socso_rates_natural_key; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX socso_rates_natural_key ON public.socso_rates USING btree (effective_from, wage_from, wage_to);


--
-- Name: users_one_active_account_per_employee; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX users_one_active_account_per_employee ON public.users USING btree (employee_id) WHERE ((employee_id IS NOT NULL) AND (deleted_at IS NULL));


--
-- Name: users_email_normalized_key; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX users_email_normalized_key ON public.users USING btree (lower(btrim((email)::text)));


--
-- Name: attendance_tenant_state_stats; Type: STATISTICS; Schema: public; Owner: -
--

CREATE STATISTICS public.attendance_tenant_state_stats (dependencies, mcv) ON company_id, check_in_at, status FROM public.attendance_records;


--
-- Name: employees_tenant_state_stats; Type: STATISTICS; Schema: public; Owner: -
--

CREATE STATISTICS public.employees_tenant_state_stats (dependencies, mcv) ON company_id, payroll_group_id, is_active, deleted_at FROM public.employees;


--
-- Name: payroll_runs_tenant_period_stats; Type: STATISTICS; Schema: public; Owner: -
--

CREATE STATISTICS public.payroll_runs_tenant_period_stats (dependencies, mcv) ON company_id, payroll_group_id, period_year, period_month, status FROM public.payroll_runs;


--
-- Name: attendance_kiosk_credentials attendance_kiosk_credentials_company_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.attendance_kiosk_credentials
    ADD CONSTRAINT attendance_kiosk_credentials_company_id_fkey FOREIGN KEY (company_id) REFERENCES public.companies(id) ON DELETE CASCADE;


--
-- Name: attendance_kiosk_credentials attendance_kiosk_credentials_created_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.attendance_kiosk_credentials
    ADD CONSTRAINT attendance_kiosk_credentials_created_by_fkey FOREIGN KEY (created_by) REFERENCES public.users(id);


--
-- Name: attendance_qr_tokens attendance_qr_tokens_company_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.attendance_qr_tokens
    ADD CONSTRAINT attendance_qr_tokens_company_id_fkey FOREIGN KEY (company_id) REFERENCES public.companies(id) ON DELETE CASCADE;


--
-- Name: attendance_records attendance_records_company_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.attendance_records
    ADD CONSTRAINT attendance_records_company_id_fkey FOREIGN KEY (company_id) REFERENCES public.companies(id) ON DELETE CASCADE;


--
-- Name: attendance_records attendance_records_created_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.attendance_records
    ADD CONSTRAINT attendance_records_created_by_fkey FOREIGN KEY (created_by) REFERENCES public.users(id);


--
-- Name: attendance_records attendance_records_employee_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.attendance_records
    ADD CONSTRAINT attendance_records_employee_id_fkey FOREIGN KEY (employee_id) REFERENCES public.employees(id) ON DELETE CASCADE;


--
-- Name: attendance_records attendance_records_qr_token_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.attendance_records
    ADD CONSTRAINT attendance_records_qr_token_id_fkey FOREIGN KEY (qr_token_id) REFERENCES public.attendance_qr_tokens(id);


--
-- Name: audit_logs audit_logs_company_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.audit_logs
    ADD CONSTRAINT audit_logs_company_id_fkey FOREIGN KEY (company_id) REFERENCES public.companies(id);


--
-- Name: audit_logs audit_logs_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.audit_logs
    ADD CONSTRAINT audit_logs_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id);


--
-- Name: bulk_import_sessions bulk_import_sessions_company_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.bulk_import_sessions
    ADD CONSTRAINT bulk_import_sessions_company_id_fkey FOREIGN KEY (company_id) REFERENCES public.companies(id);


--
-- Name: bulk_import_sessions bulk_import_sessions_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.bulk_import_sessions
    ADD CONSTRAINT bulk_import_sessions_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: claims claims_company_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.claims
    ADD CONSTRAINT claims_company_id_fkey FOREIGN KEY (company_id) REFERENCES public.companies(id);


--
-- Name: claims claims_employee_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.claims
    ADD CONSTRAINT claims_employee_id_fkey FOREIGN KEY (employee_id) REFERENCES public.employees(id);


--
-- Name: claims claims_reviewed_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.claims
    ADD CONSTRAINT claims_reviewed_by_fkey FOREIGN KEY (reviewed_by) REFERENCES public.users(id);


--
-- Name: company_locations company_locations_company_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.company_locations
    ADD CONSTRAINT company_locations_company_id_fkey FOREIGN KEY (company_id) REFERENCES public.companies(id) ON DELETE CASCADE;


--
-- Name: company_settings company_settings_company_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.company_settings
    ADD CONSTRAINT company_settings_company_id_fkey FOREIGN KEY (company_id) REFERENCES public.companies(id);


--
-- Name: company_work_schedules company_work_schedules_company_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.company_work_schedules
    ADD CONSTRAINT company_work_schedules_company_id_fkey FOREIGN KEY (company_id) REFERENCES public.companies(id) ON DELETE CASCADE;


--
-- Name: document_categories document_categories_company_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.document_categories
    ADD CONSTRAINT document_categories_company_id_fkey FOREIGN KEY (company_id) REFERENCES public.companies(id);


--
-- Name: documents documents_category_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.documents
    ADD CONSTRAINT documents_category_id_fkey FOREIGN KEY (category_id) REFERENCES public.document_categories(id);


--
-- Name: documents documents_company_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.documents
    ADD CONSTRAINT documents_company_id_fkey FOREIGN KEY (company_id) REFERENCES public.companies(id);


--
-- Name: documents documents_employee_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.documents
    ADD CONSTRAINT documents_employee_id_fkey FOREIGN KEY (employee_id) REFERENCES public.employees(id);


--
-- Name: email_logs email_logs_company_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.email_logs
    ADD CONSTRAINT email_logs_company_id_fkey FOREIGN KEY (company_id) REFERENCES public.companies(id);


--
-- Name: email_logs email_logs_created_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.email_logs
    ADD CONSTRAINT email_logs_created_by_fkey FOREIGN KEY (created_by) REFERENCES public.users(id);


--
-- Name: email_logs email_logs_employee_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.email_logs
    ADD CONSTRAINT email_logs_employee_id_fkey FOREIGN KEY (employee_id) REFERENCES public.employees(id);


--
-- Name: email_logs email_logs_template_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.email_logs
    ADD CONSTRAINT email_logs_template_id_fkey FOREIGN KEY (template_id) REFERENCES public.email_templates(id);


--
-- Name: email_templates email_templates_company_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.email_templates
    ADD CONSTRAINT email_templates_company_id_fkey FOREIGN KEY (company_id) REFERENCES public.companies(id);


--
-- Name: email_templates email_templates_created_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.email_templates
    ADD CONSTRAINT email_templates_created_by_fkey FOREIGN KEY (created_by) REFERENCES public.users(id);


--
-- Name: email_templates email_templates_updated_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.email_templates
    ADD CONSTRAINT email_templates_updated_by_fkey FOREIGN KEY (updated_by) REFERENCES public.users(id);


--
-- Name: employee_allowances employee_allowances_employee_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.employee_allowances
    ADD CONSTRAINT employee_allowances_employee_id_fkey FOREIGN KEY (employee_id) REFERENCES public.employees(id);


--
-- Name: employee_work_schedules employee_work_schedules_company_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.employee_work_schedules
    ADD CONSTRAINT employee_work_schedules_company_id_fkey FOREIGN KEY (company_id) REFERENCES public.companies(id) ON DELETE CASCADE;


--
-- Name: employee_work_schedules employee_work_schedules_employee_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.employee_work_schedules
    ADD CONSTRAINT employee_work_schedules_employee_id_fkey FOREIGN KEY (employee_id) REFERENCES public.employees(id) ON DELETE CASCADE;


--
-- Name: employees employees_company_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.employees
    ADD CONSTRAINT employees_company_id_fkey FOREIGN KEY (company_id) REFERENCES public.companies(id);


--
-- Name: employees fk_employees_payroll_group; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.employees
    ADD CONSTRAINT fk_employees_payroll_group FOREIGN KEY (payroll_group_id) REFERENCES public.payroll_groups(id);


--
-- Name: holidays holidays_company_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.holidays
    ADD CONSTRAINT holidays_company_id_fkey FOREIGN KEY (company_id) REFERENCES public.companies(id);


--
-- Name: holidays holidays_created_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.holidays
    ADD CONSTRAINT holidays_created_by_fkey FOREIGN KEY (created_by) REFERENCES public.users(id);


--
-- Name: holidays holidays_updated_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.holidays
    ADD CONSTRAINT holidays_updated_by_fkey FOREIGN KEY (updated_by) REFERENCES public.users(id);


--
-- Name: leave_balances leave_balances_employee_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.leave_balances
    ADD CONSTRAINT leave_balances_employee_id_fkey FOREIGN KEY (employee_id) REFERENCES public.employees(id);


--
-- Name: leave_balances leave_balances_leave_type_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.leave_balances
    ADD CONSTRAINT leave_balances_leave_type_id_fkey FOREIGN KEY (leave_type_id) REFERENCES public.leave_types(id);


--
-- Name: leave_requests leave_requests_company_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.leave_requests
    ADD CONSTRAINT leave_requests_company_id_fkey FOREIGN KEY (company_id) REFERENCES public.companies(id);


--
-- Name: leave_requests leave_requests_employee_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.leave_requests
    ADD CONSTRAINT leave_requests_employee_id_fkey FOREIGN KEY (employee_id) REFERENCES public.employees(id);


--
-- Name: leave_requests leave_requests_leave_type_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.leave_requests
    ADD CONSTRAINT leave_requests_leave_type_id_fkey FOREIGN KEY (leave_type_id) REFERENCES public.leave_types(id);


--
-- Name: leave_requests leave_requests_reviewed_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.leave_requests
    ADD CONSTRAINT leave_requests_reviewed_by_fkey FOREIGN KEY (reviewed_by) REFERENCES public.users(id);


--
-- Name: leave_types leave_types_company_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.leave_types
    ADD CONSTRAINT leave_types_company_id_fkey FOREIGN KEY (company_id) REFERENCES public.companies(id);


--
-- Name: notifications notifications_company_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.notifications
    ADD CONSTRAINT notifications_company_id_fkey FOREIGN KEY (company_id) REFERENCES public.companies(id);


--
-- Name: notifications notifications_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.notifications
    ADD CONSTRAINT notifications_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id);


--
-- Name: oauth2_accounts oauth2_accounts_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.oauth2_accounts
    ADD CONSTRAINT oauth2_accounts_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: overtime_applications overtime_applications_company_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.overtime_applications
    ADD CONSTRAINT overtime_applications_company_id_fkey FOREIGN KEY (company_id) REFERENCES public.companies(id);


--
-- Name: overtime_applications overtime_applications_employee_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.overtime_applications
    ADD CONSTRAINT overtime_applications_employee_id_fkey FOREIGN KEY (employee_id) REFERENCES public.employees(id);


--
-- Name: overtime_applications overtime_applications_reviewed_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.overtime_applications
    ADD CONSTRAINT overtime_applications_reviewed_by_fkey FOREIGN KEY (reviewed_by) REFERENCES public.users(id);


--
-- Name: passkey_challenges passkey_challenges_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.passkey_challenges
    ADD CONSTRAINT passkey_challenges_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: passkey_credentials passkey_credentials_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.passkey_credentials
    ADD CONSTRAINT passkey_credentials_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: password_reset_requests password_reset_requests_reviewed_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.password_reset_requests
    ADD CONSTRAINT password_reset_requests_reviewed_by_fkey FOREIGN KEY (reviewed_by) REFERENCES public.users(id);


--
-- Name: password_reset_requests password_reset_requests_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.password_reset_requests
    ADD CONSTRAINT password_reset_requests_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: payroll_entries payroll_entries_company_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.payroll_entries
    ADD CONSTRAINT payroll_entries_company_id_fkey FOREIGN KEY (company_id) REFERENCES public.companies(id);


--
-- Name: payroll_entries payroll_entries_employee_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.payroll_entries
    ADD CONSTRAINT payroll_entries_employee_id_fkey FOREIGN KEY (employee_id) REFERENCES public.employees(id);


--
-- Name: payroll_entries payroll_entries_payroll_run_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.payroll_entries
    ADD CONSTRAINT payroll_entries_payroll_run_id_fkey FOREIGN KEY (payroll_run_id) REFERENCES public.payroll_runs(id);


--
-- Name: payroll_groups payroll_groups_company_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.payroll_groups
    ADD CONSTRAINT payroll_groups_company_id_fkey FOREIGN KEY (company_id) REFERENCES public.companies(id);


--
-- Name: payroll_item_details payroll_item_details_payroll_item_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.payroll_item_details
    ADD CONSTRAINT payroll_item_details_payroll_item_id_fkey FOREIGN KEY (payroll_item_id) REFERENCES public.payroll_items(id);


--
-- Name: payroll_items payroll_items_employee_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.payroll_items
    ADD CONSTRAINT payroll_items_employee_id_fkey FOREIGN KEY (employee_id) REFERENCES public.employees(id);


--
-- Name: payroll_items payroll_items_payroll_run_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.payroll_items
    ADD CONSTRAINT payroll_items_payroll_run_id_fkey FOREIGN KEY (payroll_run_id) REFERENCES public.payroll_runs(id);


--
-- Name: payroll_runs payroll_runs_approved_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.payroll_runs
    ADD CONSTRAINT payroll_runs_approved_by_fkey FOREIGN KEY (approved_by) REFERENCES public.users(id);


--
-- Name: payroll_runs payroll_runs_company_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.payroll_runs
    ADD CONSTRAINT payroll_runs_company_id_fkey FOREIGN KEY (company_id) REFERENCES public.companies(id);


--
-- Name: payroll_runs payroll_runs_locked_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.payroll_runs
    ADD CONSTRAINT payroll_runs_locked_by_fkey FOREIGN KEY (locked_by) REFERENCES public.users(id);


--
-- Name: payroll_runs payroll_runs_payroll_group_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.payroll_runs
    ADD CONSTRAINT payroll_runs_payroll_group_id_fkey FOREIGN KEY (payroll_group_id) REFERENCES public.payroll_groups(id);


--
-- Name: payroll_runs payroll_runs_processed_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.payroll_runs
    ADD CONSTRAINT payroll_runs_processed_by_fkey FOREIGN KEY (processed_by) REFERENCES public.users(id);


--
-- Name: platform_settings platform_settings_updated_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.platform_settings
    ADD CONSTRAINT platform_settings_updated_by_fkey FOREIGN KEY (updated_by) REFERENCES public.users(id);


--
-- Name: refresh_tokens refresh_tokens_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.refresh_tokens
    ADD CONSTRAINT refresh_tokens_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: salary_history salary_history_employee_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.salary_history
    ADD CONSTRAINT salary_history_employee_id_fkey FOREIGN KEY (employee_id) REFERENCES public.employees(id);


--
-- Name: team_members team_members_employee_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.team_members
    ADD CONSTRAINT team_members_employee_id_fkey FOREIGN KEY (employee_id) REFERENCES public.employees(id) ON DELETE CASCADE;


--
-- Name: team_members team_members_team_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.team_members
    ADD CONSTRAINT team_members_team_id_fkey FOREIGN KEY (team_id) REFERENCES public.teams(id) ON DELETE CASCADE;


--
-- Name: teams teams_company_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.teams
    ADD CONSTRAINT teams_company_id_fkey FOREIGN KEY (company_id) REFERENCES public.companies(id);


--
-- Name: teams teams_created_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.teams
    ADD CONSTRAINT teams_created_by_fkey FOREIGN KEY (created_by) REFERENCES public.users(id);


--
-- Name: teams teams_updated_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.teams
    ADD CONSTRAINT teams_updated_by_fkey FOREIGN KEY (updated_by) REFERENCES public.users(id);


--
-- Name: tp3_records tp3_records_employee_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.tp3_records
    ADD CONSTRAINT tp3_records_employee_id_fkey FOREIGN KEY (employee_id) REFERENCES public.employees(id);


--
-- Name: user_companies user_companies_company_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.user_companies
    ADD CONSTRAINT user_companies_company_id_fkey FOREIGN KEY (company_id) REFERENCES public.companies(id) ON DELETE CASCADE;


--
-- Name: user_companies user_companies_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.user_companies
    ADD CONSTRAINT user_companies_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: users users_company_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_company_id_fkey FOREIGN KEY (company_id) REFERENCES public.companies(id);


--
-- Name: users users_deleted_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_deleted_by_fkey FOREIGN KEY (deleted_by) REFERENCES public.users(id) ON DELETE SET NULL;


--
-- Name: users users_employee_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_employee_id_fkey FOREIGN KEY (employee_id) REFERENCES public.employees(id) ON DELETE SET NULL;


--
-- Name: working_day_config working_day_config_company_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.working_day_config
    ADD CONSTRAINT working_day_config_company_id_fkey FOREIGN KEY (company_id) REFERENCES public.companies(id);


--
    END IF;
END
$pg19_bootstrap$;
--


-- === LIVE SCHEMA RECONCILIATION ============================================
-- Fresh databases already contain these objects from the canonical bootstrap
-- above. Existing databases with the historical v1-v4 SQLx chain execute this
-- idempotent section to reach the same PostgreSQL 19 shape.

CREATE EXTENSION IF NOT EXISTS pg_trgm WITH SCHEMA public;
CREATE EXTENSION IF NOT EXISTS btree_gist WITH SCHEMA public;

-- Retired, unused configuration store. It had effective-date columns but a
-- single-key uniqueness rule, and no application consumer. Company-scoped
-- settings remain in company_settings; platform flags remain in
-- platform_settings. RESTRICT is intentional if an external dependency exists.
DROP TABLE IF EXISTS public.system_settings;

-- A statutory calculation is only eligible for automatic payroll after the
-- corresponding source artifact and imported rules have been independently
-- verified. Date-range exclusion prevents two competing rule sets from being
-- marked for the same statutory domain and period.
CREATE TABLE IF NOT EXISTS public.statutory_rule_sets (
    id uuid DEFAULT uuidv7() PRIMARY KEY,
    dataset_key text NOT NULL,
    rule_code text NOT NULL,
    effective_from date NOT NULL,
    effective_to date,
    status text DEFAULT 'prototype'::text NOT NULL,
    source_url text,
    source_version text,
    source_sha256 character varying(64),
    verification_notes text,
    verified_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT statutory_rule_sets_dataset_key_key UNIQUE (dataset_key),
    CONSTRAINT statutory_rule_sets_dataset_key_check
        CHECK (dataset_key ~ '^[a-z0-9][a-z0-9._-]*$'),
    CONSTRAINT statutory_rule_sets_code_check
        CHECK (rule_code IN ('epf', 'socso', 'eis', 'pcb')),
    CONSTRAINT statutory_rule_sets_dates_check
        CHECK (effective_to IS NULL OR effective_to >= effective_from),
    CONSTRAINT statutory_rule_sets_status_check
        CHECK (status IN ('prototype', 'verified', 'retired')),
    CONSTRAINT statutory_rule_sets_legacy_never_verified_check
        CHECK (status <> 'verified' OR dataset_key !~ '^legacy-prototype-'),
    CONSTRAINT statutory_rule_sets_verified_metadata_check
        CHECK (
            status <> 'verified'
            OR (
                source_url IS NOT NULL
                AND btrim(source_url) <> ''
                AND source_version IS NOT NULL
                AND btrim(source_version) <> ''
                AND source_sha256 ~ '^[0-9a-f]{64}$'
                AND verified_at IS NOT NULL
            )
        ),
    CONSTRAINT statutory_rule_sets_no_overlap
        EXCLUDE USING gist (
            rule_code WITH =,
            daterange(
                effective_from,
                COALESCE(effective_to, 'infinity'::date),
                '[]'
            ) WITH &&
        )
        WHERE (status = 'verified')
);

CREATE INDEX IF NOT EXISTS idx_statutory_rule_sets_verified_lookup
    ON public.statutory_rule_sets (rule_code, effective_from DESC, effective_to)
    WHERE status = 'verified';

ALTER TABLE public.epf_rates ADD COLUMN IF NOT EXISTS rule_set_id uuid;
ALTER TABLE public.socso_rates ADD COLUMN IF NOT EXISTS rule_set_id uuid;
ALTER TABLE public.eis_rates ADD COLUMN IF NOT EXISTS rule_set_id uuid;
ALTER TABLE public.pcb_brackets ADD COLUMN IF NOT EXISTS rule_set_id uuid;
ALTER TABLE public.pcb_reliefs ADD COLUMN IF NOT EXISTS rule_set_id uuid;

DO $statutory_rule_set_foreign_keys$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'epf_rates_rule_set_id_fkey' AND conrelid = 'public.epf_rates'::regclass) THEN
        ALTER TABLE public.epf_rates ADD CONSTRAINT epf_rates_rule_set_id_fkey FOREIGN KEY (rule_set_id) REFERENCES public.statutory_rule_sets(id) ON DELETE RESTRICT;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'socso_rates_rule_set_id_fkey' AND conrelid = 'public.socso_rates'::regclass) THEN
        ALTER TABLE public.socso_rates ADD CONSTRAINT socso_rates_rule_set_id_fkey FOREIGN KEY (rule_set_id) REFERENCES public.statutory_rule_sets(id) ON DELETE RESTRICT;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'eis_rates_rule_set_id_fkey' AND conrelid = 'public.eis_rates'::regclass) THEN
        ALTER TABLE public.eis_rates ADD CONSTRAINT eis_rates_rule_set_id_fkey FOREIGN KEY (rule_set_id) REFERENCES public.statutory_rule_sets(id) ON DELETE RESTRICT;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'pcb_brackets_rule_set_id_fkey' AND conrelid = 'public.pcb_brackets'::regclass) THEN
        ALTER TABLE public.pcb_brackets ADD CONSTRAINT pcb_brackets_rule_set_id_fkey FOREIGN KEY (rule_set_id) REFERENCES public.statutory_rule_sets(id) ON DELETE RESTRICT;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'pcb_reliefs_rule_set_id_fkey' AND conrelid = 'public.pcb_reliefs'::regclass) THEN
        ALTER TABLE public.pcb_reliefs ADD CONSTRAINT pcb_reliefs_rule_set_id_fkey FOREIGN KEY (rule_set_id) REFERENCES public.statutory_rule_sets(id) ON DELETE RESTRICT;
    END IF;
END
$statutory_rule_set_foreign_keys$;

DO $statutory_band_exclusions$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'epf_rates_no_overlapping_bands' AND conrelid = 'public.epf_rates'::regclass) THEN
        ALTER TABLE public.epf_rates ADD CONSTRAINT epf_rates_no_overlapping_bands
            EXCLUDE USING gist (
                rule_set_id WITH =,
                category WITH =,
                int8range(wage_from, wage_to, '[]') WITH &&,
                daterange(effective_from, COALESCE(effective_to, 'infinity'::date), '[]') WITH &&
            ) WHERE (rule_set_id IS NOT NULL);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'socso_rates_no_overlapping_bands' AND conrelid = 'public.socso_rates'::regclass) THEN
        ALTER TABLE public.socso_rates ADD CONSTRAINT socso_rates_no_overlapping_bands
            EXCLUDE USING gist (
                rule_set_id WITH =,
                int8range(wage_from, wage_to, '[]') WITH &&,
                daterange(effective_from, COALESCE(effective_to, 'infinity'::date), '[]') WITH &&
            ) WHERE (rule_set_id IS NOT NULL);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'eis_rates_no_overlapping_bands' AND conrelid = 'public.eis_rates'::regclass) THEN
        ALTER TABLE public.eis_rates ADD CONSTRAINT eis_rates_no_overlapping_bands
            EXCLUDE USING gist (
                rule_set_id WITH =,
                int8range(wage_from, wage_to, '[]') WITH &&,
                daterange(effective_from, COALESCE(effective_to, 'infinity'::date), '[]') WITH &&
            ) WHERE (rule_set_id IS NOT NULL);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'pcb_brackets_no_overlapping_bands' AND conrelid = 'public.pcb_brackets'::regclass) THEN
        ALTER TABLE public.pcb_brackets ADD CONSTRAINT pcb_brackets_no_overlapping_bands
            EXCLUDE USING gist (
                rule_set_id WITH =,
                effective_year WITH =,
                int8range(chargeable_income_from, chargeable_income_to, '[]') WITH &&
            ) WHERE (rule_set_id IS NOT NULL);
    END IF;
END
$statutory_band_exclusions$;

ALTER TABLE public.users DROP COLUMN IF EXISTS role;
ALTER TABLE public.users
    ADD COLUMN IF NOT EXISTS deleted_at timestamp with time zone,
    ADD COLUMN IF NOT EXISTS deleted_by uuid;

-- Normalize every UUID row-identity default to PostgreSQL's native UUIDv7.
DO $uuid_defaults$
DECLARE
    target record;
BEGIN
    FOR target IN
        SELECT n.nspname AS schema_name, c.relname AS table_name
        FROM pg_catalog.pg_attribute a
        JOIN pg_catalog.pg_class c ON c.oid = a.attrelid
        JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
        WHERE n.nspname = 'public'
          AND c.relkind = 'r'
          AND a.attname = 'id'
          AND a.atttypid = 'uuid'::regtype
          AND NOT a.attisdropped
    LOOP
        EXECUTE format(
            'ALTER TABLE %I.%I ALTER COLUMN id SET DEFAULT uuidv7()',
            target.schema_name,
            target.table_name
        );
    END LOOP;
END
$uuid_defaults$;

-- Do not cascade: an unexpected external dependency should stop deployment
-- for review instead of being silently removed.
DROP EXTENSION IF EXISTS "uuid-ossp";

DROP INDEX IF EXISTS public.idx_qr_tokens_token;
DROP INDEX IF EXISTS public.idx_refresh_tokens_hash;
DROP INDEX IF EXISTS public.idx_oauth2_accounts_provider;
DROP INDEX IF EXISTS public.idx_employee_work_schedules_employee;
DROP INDEX IF EXISTS public.idx_payroll_items_run;
DROP INDEX IF EXISTS public.idx_team_members_team;
DROP INDEX IF EXISTS public.idx_user_companies_user;
DROP INDEX IF EXISTS public.idx_document_categories_company;
DROP INDEX IF EXISTS public.idx_company_settings_category;
DROP INDEX IF EXISTS public.idx_company_settings_company;
DROP INDEX IF EXISTS public.idx_payroll_runs_company;
DROP INDEX IF EXISTS public.idx_teams_company;
DROP INDEX IF EXISTS public.idx_employees_active;
DROP INDEX IF EXISTS public.idx_attendance_employee;
DROP INDEX IF EXISTS public.idx_users_active_not_deleted;
DROP INDEX IF EXISTS public.idx_epf_rates_lookup;
DROP INDEX IF EXISTS public.idx_socso_rates_lookup;
DROP INDEX IF EXISTS public.idx_eis_rates_lookup;
DROP INDEX IF EXISTS public.idx_pcb_brackets_lookup;
DROP INDEX IF EXISTS public.idx_audit_logs_created;
DROP INDEX IF EXISTS public.idx_email_logs_company;
DROP INDEX IF EXISTS public.idx_email_templates_company;

DO $normalized_user_emails$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM public.users
        GROUP BY lower(btrim(email::text))
        HAVING count(*) > 1
    ) THEN
        RAISE EXCEPTION 'Cannot enforce case-insensitive user emails: normalized duplicates exist';
    END IF;
END
$normalized_user_emails$;

DO $normalized_payroll_group_names$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM public.payroll_groups
        GROUP BY company_id, lower(btrim(name::text))
        HAVING count(*) > 1
    ) THEN
        RAISE EXCEPTION 'Cannot enforce unique payroll-group names: normalized duplicates exist';
    END IF;
END
$normalized_payroll_group_names$;

ALTER TABLE public.users DROP CONSTRAINT IF EXISTS users_email_key;
CREATE UNIQUE INDEX IF NOT EXISTS users_email_normalized_key
    ON public.users (lower(btrim(email::text)));

-- Composite parent keys let tenant-owned relationships prove company scope in
-- the database instead of trusting every application insert to pair IDs from
-- the same company.
CREATE UNIQUE INDEX IF NOT EXISTS employees_id_company_key
    ON public.employees (id, company_id);
CREATE UNIQUE INDEX IF NOT EXISTS payroll_groups_id_company_key
    ON public.payroll_groups (id, company_id);
CREATE UNIQUE INDEX IF NOT EXISTS payroll_runs_id_company_key
    ON public.payroll_runs (id, company_id);
CREATE UNIQUE INDEX IF NOT EXISTS attendance_qr_tokens_id_company_key
    ON public.attendance_qr_tokens (id, company_id);
CREATE UNIQUE INDEX IF NOT EXISTS leave_types_id_company_key
    ON public.leave_types (id, company_id);
CREATE UNIQUE INDEX IF NOT EXISTS document_categories_id_company_key
    ON public.document_categories (id, company_id);

CREATE UNIQUE INDEX IF NOT EXISTS payroll_runs_one_active_period
    ON public.payroll_runs (company_id, payroll_group_id, period_year, period_month)
    WHERE status <> 'cancelled'::public.payroll_status;
CREATE UNIQUE INDEX IF NOT EXISTS payroll_groups_company_name_key
    ON public.payroll_groups (company_id, lower(btrim(name::text)));
DROP INDEX IF EXISTS public.users_one_account_per_employee;
CREATE UNIQUE INDEX IF NOT EXISTS users_one_active_account_per_employee
    ON public.users (employee_id)
    WHERE employee_id IS NOT NULL AND deleted_at IS NULL;
DROP INDEX IF EXISTS public.epf_rates_natural_key;
DROP INDEX IF EXISTS public.socso_rates_natural_key;
DROP INDEX IF EXISTS public.eis_rates_natural_key;
DROP INDEX IF EXISTS public.pcb_brackets_natural_key;
DROP INDEX IF EXISTS public.pcb_reliefs_natural_key;
CREATE UNIQUE INDEX epf_rates_natural_key
    ON public.epf_rates (rule_set_id, category, effective_from, wage_from, wage_to)
    NULLS NOT DISTINCT;
CREATE UNIQUE INDEX socso_rates_natural_key
    ON public.socso_rates (rule_set_id, effective_from, wage_from, wage_to)
    NULLS NOT DISTINCT;
CREATE UNIQUE INDEX eis_rates_natural_key
    ON public.eis_rates (rule_set_id, effective_from, wage_from, wage_to)
    NULLS NOT DISTINCT;
CREATE UNIQUE INDEX pcb_brackets_natural_key
    ON public.pcb_brackets (rule_set_id, effective_year, chargeable_income_from, chargeable_income_to)
    NULLS NOT DISTINCT;
CREATE UNIQUE INDEX pcb_reliefs_natural_key
    ON public.pcb_reliefs (rule_set_id, effective_year, relief_type)
    NULLS NOT DISTINCT;

CREATE INDEX IF NOT EXISTS idx_employees_search_trgm
    ON public.employees USING gin (
        lower((((full_name)::text || ' '::text) || (employee_number)::text)) public.gin_trgm_ops
    )
    WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_employees_payroll_active
    ON public.employees (company_id, payroll_group_id, date_joined)
    WHERE is_active = true AND deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_attendance_employee_recent
    ON public.attendance_records (employee_id, check_in_at DESC);
CREATE INDEX IF NOT EXISTS idx_payroll_runs_company_period
    ON public.payroll_runs (company_id, period_year DESC, period_month DESC, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_payroll_entries_run_pending
    ON public.payroll_entries (payroll_run_id)
    WHERE payroll_run_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_claims_approved_payroll
    ON public.claims (company_id, employee_id, expense_date)
    WHERE status = 'approved';
CREATE INDEX IF NOT EXISTS idx_overtime_approved_payroll
    ON public.overtime_applications (employee_id, ot_date)
    INCLUDE (ot_type, hours)
    WHERE status = 'approved';
CREATE INDEX IF NOT EXISTS idx_leave_requests_approved_dates
    ON public.leave_requests (employee_id, start_date, end_date)
    WHERE status = 'approved';
CREATE INDEX IF NOT EXISTS idx_notifications_unread_recent
    ON public.notifications (user_id, created_at DESC)
    WHERE is_read = false;
CREATE INDEX IF NOT EXISTS idx_users_company_active
    ON public.users (company_id, created_at DESC)
    WHERE is_active = true AND deleted_at IS NULL;

-- Provision the minimum tenant configuration required by otherwise read-only
-- setup APIs. This function is idempotent and is called for existing companies
-- by 1001_data.sql and transactionally whenever a new company is created.
CREATE OR REPLACE FUNCTION public.provision_company_defaults(
    p_company_id uuid,
    p_actor_id uuid DEFAULT NULL
) RETURNS void
LANGUAGE plpgsql
SET search_path = public, pg_temp
AS $company_defaults$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM payroll_groups WHERE company_id = p_company_id
    ) THEN
        INSERT INTO payroll_groups (
            company_id, name, description, cutoff_day, payment_day, created_by, updated_by
        ) VALUES (
            p_company_id, 'Default', 'Default monthly payroll group', 25, 28,
            p_actor_id, p_actor_id
        )
        ON CONFLICT DO NOTHING;
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM leave_types WHERE company_id = p_company_id
    ) THEN
        INSERT INTO leave_types (
            company_id, name, description, default_days, is_paid, is_system
        ) VALUES
            (p_company_id, 'Annual Leave', 'Paid annual leave entitlement', 14, true, true),
            (p_company_id, 'Sick Leave', 'Paid sick leave (outpatient)', 14, true, true),
            (p_company_id, 'Hospitalisation Leave', 'Paid hospitalisation leave', 60, true, true),
            (p_company_id, 'Compassionate Leave', 'Bereavement / compassionate leave', 3, true, true),
            (p_company_id, 'Maternity Leave', 'Paid maternity leave', 98, true, true),
            (p_company_id, 'Paternity Leave', 'Paid paternity leave', 7, true, true),
            (p_company_id, 'Marriage Leave', 'Leave for own marriage', 3, true, true),
            (p_company_id, 'Unpaid Leave', 'Unpaid leave', 365, false, true)
        ON CONFLICT (company_id, name) DO NOTHING;
    END IF;

    INSERT INTO working_day_config (company_id, day_of_week, is_working_day)
    VALUES
        (p_company_id, 0, false),
        (p_company_id, 1, true),
        (p_company_id, 2, true),
        (p_company_id, 3, true),
        (p_company_id, 4, true),
        (p_company_id, 5, true),
        (p_company_id, 6, false)
    ON CONFLICT (company_id, day_of_week) DO NOTHING;

    INSERT INTO company_work_schedules (
        company_id, name, start_time, end_time, grace_minutes,
        half_day_hours, timezone, is_default
    ) VALUES (
        p_company_id, 'Default', '09:00', '18:00', 15,
        4.0, 'Asia/Kuala_Lumpur', true
    )
    ON CONFLICT (company_id) WHERE is_default = true DO NOTHING;

    INSERT INTO company_settings (
        company_id, category, key, value, label, description
    )
    SELECT p_company_id, defaults.category, defaults.key,
           defaults.value::jsonb, defaults.label, defaults.description
    FROM (VALUES
        ('payroll', 'default_pay_day', '"28"', 'Default Pay Day', 'Day of month for salary payment'),
        ('payroll', 'default_cutoff_day', '"25"', 'Default Cutoff Day', 'Day of month for payroll cutoff'),
        ('payroll', 'overtime_multiplier_normal', '"1.5"', 'OT Multiplier (Normal)', 'Overtime rate multiplier for normal working days'),
        ('payroll', 'overtime_multiplier_rest', '"2.0"', 'OT Multiplier (Rest Day)', 'Overtime rate multiplier for rest days'),
        ('payroll', 'overtime_multiplier_public', '"3.0"', 'OT Multiplier (Public Holiday)', 'Overtime rate multiplier for public holidays'),
        ('payroll', 'unpaid_leave_divisor', '"26"', 'Unpaid Leave Divisor', 'Number of working days used for unpaid leave deduction'),
        ('payroll', 'rounding_method', '"nearest"', 'Rounding Method', 'Salary calculation rounding method'),
        ('payroll', 'working_hours_per_day', '"9"', 'Working Hours Per Day', 'Office hours per day including rest time'),
        ('payroll', 'rest_time_minutes', '"60"', 'Rest Time (minutes)', 'Daily rest time'),
        ('payroll', 'effective_hours_per_day', '"8"', 'Effective Hours Per Day', 'Working hours after rest time'),
        ('statutory', 'epf_employer_rate_below_60', '"13"', 'EPF Employer Rate (< 60)', 'Reference employer rate percentage'),
        ('statutory', 'epf_employer_rate_above_60', '"6.5"', 'EPF Employer Rate (>= 60)', 'Reference employer rate percentage'),
        ('statutory', 'socso_enabled', 'true', 'SOCSO Enabled', 'Whether SOCSO calculations are enabled'),
        ('statutory', 'eis_enabled', 'true', 'EIS Enabled', 'Whether EIS calculations are enabled'),
        ('statutory', 'hrdf_enabled', 'false', 'HRDF Enabled', 'Whether HRDF calculations are enabled'),
        ('statutory', 'hrdf_rate', '"1"', 'HRDF Rate (%)', 'HRDF levy percentage'),
        ('system', 'currency', '"MYR"', 'Currency', 'System currency code'),
        ('system', 'date_format', '"DD/MM/YYYY"', 'Date Format', 'Display date format'),
        ('system', 'financial_year_start_month', '"1"', 'Financial Year Start', 'Financial year start month'),
        ('system', 'payslip_template', '"default"', 'Payslip Template', 'Payslip template key'),
        ('notifications', 'email_payslip', 'true', 'Email Payslips', 'Email payslips after payroll approval'),
        ('notifications', 'expiry_alert_days', '"30"', 'Document Expiry Alert (days)', 'Document expiry warning window'),
        ('notifications', 'probation_alert_days', '"14"', 'Probation End Alert (days)', 'Probation warning window'),
        ('email', 'auto_welcome_email', 'true', 'Auto Welcome Email', 'Email an employee after account creation')
    ) AS defaults(category, key, value, label, description)
    ON CONFLICT (company_id, category, key) DO NOTHING;
END
$company_defaults$;

-- Junction tables without a stored company_id still enforce same-company
-- parentage at write time. Existing legacy rows are audited before cutover;
-- these triggers prevent any new cross-tenant association immediately.
CREATE OR REPLACE FUNCTION public.enforce_payroll_item_company()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = public, pg_temp
AS $payroll_item_company$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM payroll_runs run
        JOIN employees employee ON employee.id = NEW.employee_id
        WHERE run.id = NEW.payroll_run_id
          AND run.company_id = employee.company_id
    ) THEN
        RAISE EXCEPTION 'Payroll item run and employee must belong to the same company'
            USING ERRCODE = '23514', CONSTRAINT = 'payroll_items_same_company_check';
    END IF;
    RETURN NEW;
END
$payroll_item_company$;

DROP TRIGGER IF EXISTS payroll_items_same_company_trigger ON public.payroll_items;
CREATE TRIGGER payroll_items_same_company_trigger
    BEFORE INSERT OR UPDATE ON public.payroll_items
    FOR EACH ROW EXECUTE FUNCTION public.enforce_payroll_item_company();

CREATE OR REPLACE FUNCTION public.enforce_leave_balance_company()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = public, pg_temp
AS $leave_balance_company$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM employees employee
        JOIN leave_types leave_type ON leave_type.id = NEW.leave_type_id
        WHERE employee.id = NEW.employee_id
          AND employee.company_id = leave_type.company_id
    ) THEN
        RAISE EXCEPTION 'Leave balance employee and leave type must belong to the same company'
            USING ERRCODE = '23514', CONSTRAINT = 'leave_balances_same_company_check';
    END IF;
    RETURN NEW;
END
$leave_balance_company$;

DROP TRIGGER IF EXISTS leave_balances_same_company_trigger ON public.leave_balances;
CREATE TRIGGER leave_balances_same_company_trigger
    BEFORE INSERT OR UPDATE ON public.leave_balances
    FOR EACH ROW EXECUTE FUNCTION public.enforce_leave_balance_company();

CREATE OR REPLACE FUNCTION public.enforce_team_member_company()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = public, pg_temp
AS $team_member_company$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM teams team
        JOIN employees employee ON employee.id = NEW.employee_id
        WHERE team.id = NEW.team_id
          AND team.company_id = employee.company_id
    ) THEN
        RAISE EXCEPTION 'Team and employee must belong to the same company'
            USING ERRCODE = '23514', CONSTRAINT = 'team_members_same_company_check';
    END IF;
    RETURN NEW;
END
$team_member_company$;

DROP TRIGGER IF EXISTS team_members_same_company_trigger ON public.team_members;
CREATE TRIGGER team_members_same_company_trigger
    BEFORE INSERT OR UPDATE ON public.team_members
    FOR EACH ROW EXECUTE FUNCTION public.enforce_team_member_company();

CREATE OR REPLACE FUNCTION public.enforce_immutable_company_id()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = public, pg_temp
AS $immutable_company_id$
BEGIN
    IF OLD.company_id IS DISTINCT FROM NEW.company_id THEN
        RAISE EXCEPTION 'Tenant ownership cannot be changed after creation'
            USING ERRCODE = '23514', CONSTRAINT = 'tenant_company_id_immutable_check';
    END IF;
    RETURN NEW;
END
$immutable_company_id$;

DROP TRIGGER IF EXISTS employees_company_immutable_trigger ON public.employees;
CREATE TRIGGER employees_company_immutable_trigger
    BEFORE UPDATE OF company_id ON public.employees
    FOR EACH ROW EXECUTE FUNCTION public.enforce_immutable_company_id();
DROP TRIGGER IF EXISTS payroll_groups_company_immutable_trigger ON public.payroll_groups;
CREATE TRIGGER payroll_groups_company_immutable_trigger
    BEFORE UPDATE OF company_id ON public.payroll_groups
    FOR EACH ROW EXECUTE FUNCTION public.enforce_immutable_company_id();
DROP TRIGGER IF EXISTS payroll_runs_company_immutable_trigger ON public.payroll_runs;
CREATE TRIGGER payroll_runs_company_immutable_trigger
    BEFORE UPDATE OF company_id ON public.payroll_runs
    FOR EACH ROW EXECUTE FUNCTION public.enforce_immutable_company_id();
DROP TRIGGER IF EXISTS attendance_qr_tokens_company_immutable_trigger ON public.attendance_qr_tokens;
CREATE TRIGGER attendance_qr_tokens_company_immutable_trigger
    BEFORE UPDATE OF company_id ON public.attendance_qr_tokens
    FOR EACH ROW EXECUTE FUNCTION public.enforce_immutable_company_id();
DROP TRIGGER IF EXISTS leave_types_company_immutable_trigger ON public.leave_types;
CREATE TRIGGER leave_types_company_immutable_trigger
    BEFORE UPDATE OF company_id ON public.leave_types
    FOR EACH ROW EXECUTE FUNCTION public.enforce_immutable_company_id();
DROP TRIGGER IF EXISTS document_categories_company_immutable_trigger ON public.document_categories;
CREATE TRIGGER document_categories_company_immutable_trigger
    BEFORE UPDATE OF company_id ON public.document_categories
    FOR EACH ROW EXECUTE FUNCTION public.enforce_immutable_company_id();
DROP TRIGGER IF EXISTS teams_company_immutable_trigger ON public.teams;
CREATE TRIGGER teams_company_immutable_trigger
    BEFORE UPDATE OF company_id ON public.teams
    FOR EACH ROW EXECUTE FUNCTION public.enforce_immutable_company_id();

-- Existing rows are audited separately. NOT VALID makes each check/FK apply to
-- new writes immediately without turning application startup into a risky full
-- table validation. Fresh databases have validated versions from the baseline.
DO $constraints$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'attendance_records_checkout_order_check' AND conrelid = 'public.attendance_records'::regclass) THEN
        ALTER TABLE public.attendance_records ADD CONSTRAINT attendance_records_checkout_order_check CHECK (check_out_at IS NULL OR check_out_at >= check_in_at) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'attendance_records_checkin_coordinates_check' AND conrelid = 'public.attendance_records'::regclass) THEN
        ALTER TABLE public.attendance_records ADD CONSTRAINT attendance_records_checkin_coordinates_check CHECK (((latitude IS NULL) = (longitude IS NULL)) AND (latitude IS NULL OR (latitude BETWEEN -90 AND 90 AND longitude BETWEEN -180 AND 180))) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'attendance_records_checkout_coordinates_check' AND conrelid = 'public.attendance_records'::regclass) THEN
        ALTER TABLE public.attendance_records ADD CONSTRAINT attendance_records_checkout_coordinates_check CHECK (((checkout_latitude IS NULL) = (checkout_longitude IS NULL)) AND (checkout_latitude IS NULL OR (checkout_latitude BETWEEN -90 AND 90 AND checkout_longitude BETWEEN -180 AND 180))) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'attendance_records_hours_check' AND conrelid = 'public.attendance_records'::regclass) THEN
        ALTER TABLE public.attendance_records ADD CONSTRAINT attendance_records_hours_check CHECK ((hours_worked IS NULL OR hours_worked >= 0) AND (overtime_hours IS NULL OR overtime_hours >= 0)) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'bulk_import_sessions_counts_check' AND conrelid = 'public.bulk_import_sessions'::regclass) THEN
        ALTER TABLE public.bulk_import_sessions ADD CONSTRAINT bulk_import_sessions_counts_check CHECK (row_count >= 0 AND valid_count >= 0 AND valid_count <= row_count) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'bulk_import_sessions_expiry_check' AND conrelid = 'public.bulk_import_sessions'::regclass) THEN
        ALTER TABLE public.bulk_import_sessions ADD CONSTRAINT bulk_import_sessions_expiry_check CHECK (expires_at > created_at) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'bulk_import_sessions_status_check' AND conrelid = 'public.bulk_import_sessions'::regclass) THEN
        ALTER TABLE public.bulk_import_sessions ADD CONSTRAINT bulk_import_sessions_status_check CHECK (status IN ('pending', 'confirmed', 'expired', 'failed')) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'companies_unpaid_leave_divisor_check' AND conrelid = 'public.companies'::regclass) THEN
        ALTER TABLE public.companies ADD CONSTRAINT companies_unpaid_leave_divisor_check CHECK (unpaid_leave_divisor > 0) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'company_locations_coordinates_check' AND conrelid = 'public.company_locations'::regclass) THEN
        ALTER TABLE public.company_locations ADD CONSTRAINT company_locations_coordinates_check CHECK (latitude BETWEEN -90 AND 90 AND longitude BETWEEN -180 AND 180) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'company_locations_radius_check' AND conrelid = 'public.company_locations'::regclass) THEN
        ALTER TABLE public.company_locations ADD CONSTRAINT company_locations_radius_check CHECK (radius_meters > 0) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'eis_rates_range_check' AND conrelid = 'public.eis_rates'::regclass) THEN
        ALTER TABLE public.eis_rates ADD CONSTRAINT eis_rates_range_check CHECK (wage_from >= 0 AND wage_to >= wage_from) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'eis_rates_contributions_check' AND conrelid = 'public.eis_rates'::regclass) THEN
        ALTER TABLE public.eis_rates ADD CONSTRAINT eis_rates_contributions_check CHECK (employee_contribution >= 0 AND employer_contribution >= 0) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'eis_rates_effective_dates_check' AND conrelid = 'public.eis_rates'::regclass) THEN
        ALTER TABLE public.eis_rates ADD CONSTRAINT eis_rates_effective_dates_check CHECK (effective_to IS NULL OR effective_to >= effective_from) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'epf_rates_range_check' AND conrelid = 'public.epf_rates'::regclass) THEN
        ALTER TABLE public.epf_rates ADD CONSTRAINT epf_rates_range_check CHECK (wage_from >= 0 AND wage_to >= wage_from) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'epf_rates_contributions_check' AND conrelid = 'public.epf_rates'::regclass) THEN
        ALTER TABLE public.epf_rates ADD CONSTRAINT epf_rates_contributions_check CHECK (employee_contribution >= 0 AND employer_contribution >= 0) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'epf_rates_effective_dates_check' AND conrelid = 'public.epf_rates'::regclass) THEN
        ALTER TABLE public.epf_rates ADD CONSTRAINT epf_rates_effective_dates_check CHECK (effective_to IS NULL OR effective_to >= effective_from) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'socso_rates_range_check' AND conrelid = 'public.socso_rates'::regclass) THEN
        ALTER TABLE public.socso_rates ADD CONSTRAINT socso_rates_range_check CHECK (wage_from >= 0 AND wage_to >= wage_from) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'socso_rates_contributions_check' AND conrelid = 'public.socso_rates'::regclass) THEN
        ALTER TABLE public.socso_rates ADD CONSTRAINT socso_rates_contributions_check CHECK (first_cat_employee >= 0 AND first_cat_employer >= 0 AND second_cat_employer >= 0) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'socso_rates_effective_dates_check' AND conrelid = 'public.socso_rates'::regclass) THEN
        ALTER TABLE public.socso_rates ADD CONSTRAINT socso_rates_effective_dates_check CHECK (effective_to IS NULL OR effective_to >= effective_from) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'pcb_brackets_range_check' AND conrelid = 'public.pcb_brackets'::regclass) THEN
        ALTER TABLE public.pcb_brackets ADD CONSTRAINT pcb_brackets_range_check CHECK (chargeable_income_from >= 0 AND chargeable_income_to >= chargeable_income_from) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'pcb_brackets_rate_check' AND conrelid = 'public.pcb_brackets'::regclass) THEN
        ALTER TABLE public.pcb_brackets ADD CONSTRAINT pcb_brackets_rate_check CHECK (tax_rate_percent BETWEEN 0 AND 100 AND cumulative_tax >= 0) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'pcb_brackets_year_check' AND conrelid = 'public.pcb_brackets'::regclass) THEN
        ALTER TABLE public.pcb_brackets ADD CONSTRAINT pcb_brackets_year_check CHECK (effective_year BETWEEN 2000 AND 2200) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'pcb_reliefs_amount_year_check' AND conrelid = 'public.pcb_reliefs'::regclass) THEN
        ALTER TABLE public.pcb_reliefs ADD CONSTRAINT pcb_reliefs_amount_year_check CHECK (amount >= 0 AND effective_year BETWEEN 2000 AND 2200) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'leave_requests_dates_check' AND conrelid = 'public.leave_requests'::regclass) THEN
        ALTER TABLE public.leave_requests ADD CONSTRAINT leave_requests_dates_check CHECK (start_date <= end_date) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'leave_requests_days_check' AND conrelid = 'public.leave_requests'::regclass) THEN
        ALTER TABLE public.leave_requests ADD CONSTRAINT leave_requests_days_check CHECK (days > 0) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'payroll_groups_days_check' AND conrelid = 'public.payroll_groups'::regclass) THEN
        ALTER TABLE public.payroll_groups ADD CONSTRAINT payroll_groups_days_check CHECK (cutoff_day BETWEEN 1 AND 31 AND payment_day BETWEEN 1 AND 31) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'payroll_runs_period_check' AND conrelid = 'public.payroll_runs'::regclass) THEN
        ALTER TABLE public.payroll_runs ADD CONSTRAINT payroll_runs_period_check CHECK (period_month BETWEEN 1 AND 12 AND period_start <= period_end) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'payroll_runs_version_count_check' AND conrelid = 'public.payroll_runs'::regclass) THEN
        ALTER TABLE public.payroll_runs ADD CONSTRAINT payroll_runs_version_count_check CHECK (version > 0 AND employee_count >= 0) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'users_deleted_by_fkey' AND conrelid = 'public.users'::regclass) THEN
        ALTER TABLE public.users ADD CONSTRAINT users_deleted_by_fkey FOREIGN KEY (deleted_by) REFERENCES public.users(id) ON DELETE SET NULL NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'bulk_import_sessions_user_id_fkey' AND conrelid = 'public.bulk_import_sessions'::regclass) THEN
        ALTER TABLE public.bulk_import_sessions ADD CONSTRAINT bulk_import_sessions_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE NOT VALID;
    END IF;
END
$constraints$;

-- Critical tenant-owned relationships use composite foreign keys. Existing
-- populated installations receive them NOT VALID (new writes are still
-- enforced); an empty/fresh database validates them immediately.
DO $tenant_constraints$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'users_employee_company_required_check' AND conrelid = 'public.users'::regclass) THEN
        ALTER TABLE public.users ADD CONSTRAINT users_employee_company_required_check
            CHECK (employee_id IS NULL OR company_id IS NOT NULL) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'employees_payroll_group_tenant_fkey' AND conrelid = 'public.employees'::regclass) THEN
        ALTER TABLE public.employees ADD CONSTRAINT employees_payroll_group_tenant_fkey
            FOREIGN KEY (payroll_group_id, company_id)
            REFERENCES public.payroll_groups(id, company_id) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'attendance_records_employee_tenant_fkey' AND conrelid = 'public.attendance_records'::regclass) THEN
        ALTER TABLE public.attendance_records ADD CONSTRAINT attendance_records_employee_tenant_fkey
            FOREIGN KEY (employee_id, company_id)
            REFERENCES public.employees(id, company_id) ON DELETE CASCADE NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'attendance_records_qr_token_tenant_fkey' AND conrelid = 'public.attendance_records'::regclass) THEN
        ALTER TABLE public.attendance_records ADD CONSTRAINT attendance_records_qr_token_tenant_fkey
            FOREIGN KEY (qr_token_id, company_id)
            REFERENCES public.attendance_qr_tokens(id, company_id) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'payroll_runs_group_tenant_fkey' AND conrelid = 'public.payroll_runs'::regclass) THEN
        ALTER TABLE public.payroll_runs ADD CONSTRAINT payroll_runs_group_tenant_fkey
            FOREIGN KEY (payroll_group_id, company_id)
            REFERENCES public.payroll_groups(id, company_id) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'payroll_entries_employee_tenant_fkey' AND conrelid = 'public.payroll_entries'::regclass) THEN
        ALTER TABLE public.payroll_entries ADD CONSTRAINT payroll_entries_employee_tenant_fkey
            FOREIGN KEY (employee_id, company_id)
            REFERENCES public.employees(id, company_id) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'payroll_entries_run_tenant_fkey' AND conrelid = 'public.payroll_entries'::regclass) THEN
        ALTER TABLE public.payroll_entries ADD CONSTRAINT payroll_entries_run_tenant_fkey
            FOREIGN KEY (payroll_run_id, company_id)
            REFERENCES public.payroll_runs(id, company_id) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'users_employee_tenant_fkey' AND conrelid = 'public.users'::regclass) THEN
        ALTER TABLE public.users ADD CONSTRAINT users_employee_tenant_fkey
            FOREIGN KEY (employee_id, company_id)
            REFERENCES public.employees(id, company_id)
            ON DELETE SET NULL (employee_id) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'claims_employee_tenant_fkey' AND conrelid = 'public.claims'::regclass) THEN
        ALTER TABLE public.claims ADD CONSTRAINT claims_employee_tenant_fkey
            FOREIGN KEY (employee_id, company_id)
            REFERENCES public.employees(id, company_id) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'leave_requests_employee_tenant_fkey' AND conrelid = 'public.leave_requests'::regclass) THEN
        ALTER TABLE public.leave_requests ADD CONSTRAINT leave_requests_employee_tenant_fkey
            FOREIGN KEY (employee_id, company_id)
            REFERENCES public.employees(id, company_id) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'leave_requests_type_tenant_fkey' AND conrelid = 'public.leave_requests'::regclass) THEN
        ALTER TABLE public.leave_requests ADD CONSTRAINT leave_requests_type_tenant_fkey
            FOREIGN KEY (leave_type_id, company_id)
            REFERENCES public.leave_types(id, company_id) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'overtime_employee_tenant_fkey' AND conrelid = 'public.overtime_applications'::regclass) THEN
        ALTER TABLE public.overtime_applications ADD CONSTRAINT overtime_employee_tenant_fkey
            FOREIGN KEY (employee_id, company_id)
            REFERENCES public.employees(id, company_id) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'documents_employee_tenant_fkey' AND conrelid = 'public.documents'::regclass) THEN
        ALTER TABLE public.documents ADD CONSTRAINT documents_employee_tenant_fkey
            FOREIGN KEY (employee_id, company_id)
            REFERENCES public.employees(id, company_id) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'documents_category_tenant_fkey' AND conrelid = 'public.documents'::regclass) THEN
        ALTER TABLE public.documents ADD CONSTRAINT documents_category_tenant_fkey
            FOREIGN KEY (category_id, company_id)
            REFERENCES public.document_categories(id, company_id) NOT VALID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'employee_work_schedules_employee_tenant_fkey' AND conrelid = 'public.employee_work_schedules'::regclass) THEN
        ALTER TABLE public.employee_work_schedules ADD CONSTRAINT employee_work_schedules_employee_tenant_fkey
            FOREIGN KEY (employee_id, company_id)
            REFERENCES public.employees(id, company_id) ON DELETE CASCADE NOT VALID;
    END IF;

    IF NOT EXISTS (SELECT 1 FROM public.companies) THEN
        ALTER TABLE public.users VALIDATE CONSTRAINT users_employee_company_required_check;
        ALTER TABLE public.employees VALIDATE CONSTRAINT employees_payroll_group_tenant_fkey;
        ALTER TABLE public.attendance_records VALIDATE CONSTRAINT attendance_records_employee_tenant_fkey;
        ALTER TABLE public.attendance_records VALIDATE CONSTRAINT attendance_records_qr_token_tenant_fkey;
        ALTER TABLE public.payroll_runs VALIDATE CONSTRAINT payroll_runs_group_tenant_fkey;
        ALTER TABLE public.payroll_entries VALIDATE CONSTRAINT payroll_entries_employee_tenant_fkey;
        ALTER TABLE public.payroll_entries VALIDATE CONSTRAINT payroll_entries_run_tenant_fkey;
        ALTER TABLE public.users VALIDATE CONSTRAINT users_employee_tenant_fkey;
        ALTER TABLE public.claims VALIDATE CONSTRAINT claims_employee_tenant_fkey;
        ALTER TABLE public.leave_requests VALIDATE CONSTRAINT leave_requests_employee_tenant_fkey;
        ALTER TABLE public.leave_requests VALIDATE CONSTRAINT leave_requests_type_tenant_fkey;
        ALTER TABLE public.overtime_applications VALIDATE CONSTRAINT overtime_employee_tenant_fkey;
        ALTER TABLE public.documents VALIDATE CONSTRAINT documents_employee_tenant_fkey;
        ALTER TABLE public.documents VALIDATE CONSTRAINT documents_category_tenant_fkey;
        ALTER TABLE public.employee_work_schedules VALIDATE CONSTRAINT employee_work_schedules_employee_tenant_fkey;
    END IF;
END
$tenant_constraints$;

-- The company-qualified relationships above subsume these scalar foreign keys
-- (including their delete actions) and avoid duplicate checks on every write.
ALTER TABLE public.attendance_records DROP CONSTRAINT IF EXISTS attendance_records_employee_id_fkey;
ALTER TABLE public.attendance_records DROP CONSTRAINT IF EXISTS attendance_records_qr_token_id_fkey;
ALTER TABLE public.payroll_entries DROP CONSTRAINT IF EXISTS payroll_entries_employee_id_fkey;
ALTER TABLE public.payroll_entries DROP CONSTRAINT IF EXISTS payroll_entries_payroll_run_id_fkey;
ALTER TABLE public.payroll_runs DROP CONSTRAINT IF EXISTS payroll_runs_payroll_group_id_fkey;
ALTER TABLE public.users DROP CONSTRAINT IF EXISTS users_employee_id_fkey;
ALTER TABLE public.claims DROP CONSTRAINT IF EXISTS claims_employee_id_fkey;
ALTER TABLE public.leave_requests DROP CONSTRAINT IF EXISTS leave_requests_employee_id_fkey;
ALTER TABLE public.leave_requests DROP CONSTRAINT IF EXISTS leave_requests_leave_type_id_fkey;
ALTER TABLE public.overtime_applications DROP CONSTRAINT IF EXISTS overtime_applications_employee_id_fkey;
ALTER TABLE public.documents DROP CONSTRAINT IF EXISTS documents_employee_id_fkey;
ALTER TABLE public.documents DROP CONSTRAINT IF EXISTS documents_category_id_fkey;
ALTER TABLE public.employee_work_schedules DROP CONSTRAINT IF EXISTS employee_work_schedules_employee_id_fkey;
ALTER TABLE public.employees DROP CONSTRAINT IF EXISTS fk_employees_payroll_group;

CREATE STATISTICS IF NOT EXISTS public.employees_tenant_state_stats (dependencies, mcv)
    ON company_id, is_active, deleted_at, payroll_group_id
    FROM public.employees;
CREATE STATISTICS IF NOT EXISTS public.attendance_tenant_state_stats (dependencies, mcv)
    ON company_id, status, check_in_at
    FROM public.attendance_records;
CREATE STATISTICS IF NOT EXISTS public.payroll_runs_tenant_period_stats (dependencies, mcv)
    ON company_id, payroll_group_id, period_year, period_month, status
    FROM public.payroll_runs;
