--
-- PostgreSQL database dump
--


-- Dumped from database version 18.4 (Homebrew)
-- Dumped by pg_dump version 18.4 (Homebrew)

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET transaction_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

--
-- Name: public; Type: SCHEMA; Schema: -; Owner: -
--

-- *not* creating schema, since initdb creates it


--
-- Name: uuid-ossp; Type: EXTENSION; Schema: -; Owner: -
--

CREATE EXTENSION IF NOT EXISTS "uuid-ossp" WITH SCHEMA public;


--
-- Name: EXTENSION "uuid-ossp"; Type: COMMENT; Schema: -; Owner: -
--

COMMENT ON EXTENSION "uuid-ossp" IS 'generate universally unique identifiers (UUIDs)';


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


SET default_tablespace = '';

SET default_table_access_method = heap;

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
    id uuid NOT NULL,
    company_id uuid NOT NULL,
    user_id uuid NOT NULL,
    file_name text NOT NULL,
    row_count integer NOT NULL,
    valid_count integer NOT NULL,
    validated_data jsonb NOT NULL,
    status text DEFAULT 'pending'::text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    confirmed_at timestamp with time zone,
    expires_at timestamp with time zone DEFAULT (now() + '01:00:00'::interval) NOT NULL
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
    CONSTRAINT companies_geofence_mode_check CHECK (((geofence_mode)::text = ANY (ARRAY[('none'::character varying)::text, ('warn'::character varying)::text, ('enforce'::character varying)::text])))
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
    updated_at timestamp with time zone DEFAULT now() NOT NULL
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
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: email_logs; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.email_logs (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
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
    id uuid DEFAULT gen_random_uuid() NOT NULL,
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
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: holidays; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.holidays (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
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
    id uuid DEFAULT gen_random_uuid() NOT NULL,
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
    id uuid DEFAULT gen_random_uuid() NOT NULL,
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
    updated_by uuid
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
    updated_by uuid
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
    created_at timestamp with time zone DEFAULT now() NOT NULL
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
    created_at timestamp with time zone DEFAULT now() NOT NULL
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
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: system_settings; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.system_settings (
    id uuid DEFAULT uuidv7() NOT NULL,
    setting_key character varying(100) NOT NULL,
    setting_value character varying(500) NOT NULL,
    description character varying(255),
    effective_from date NOT NULL,
    effective_to date,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: team_members; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.team_members (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
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
    id uuid DEFAULT gen_random_uuid() NOT NULL,
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
    role character varying(50) DEFAULT 'employee'::character varying NOT NULL,
    company_id uuid,
    employee_id uuid,
    is_active boolean DEFAULT true,
    last_login timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    must_change_password boolean DEFAULT false NOT NULL,
    roles character varying(50)[] DEFAULT ARRAY['employee'::character varying(50)] NOT NULL,
    CONSTRAINT users_role_check CHECK (((role)::text = ANY (ARRAY[('super_admin'::character varying)::text, ('admin'::character varying)::text, ('payroll_admin'::character varying)::text, ('hr_manager'::character varying)::text, ('finance'::character varying)::text, ('exec'::character varying)::text, ('employee'::character varying)::text]))),
    CONSTRAINT users_roles_valid CHECK (((cardinality(roles) >= 1) AND (roles <@ ARRAY['super_admin'::character varying(50), 'admin'::character varying(50), 'payroll_admin'::character varying(50), 'hr_manager'::character varying(50), 'finance'::character varying(50), 'exec'::character varying(50), 'employee'::character varying(50)])))
);


--
-- Name: working_day_config; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.working_day_config (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
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
-- Name: system_settings system_settings_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.system_settings
    ADD CONSTRAINT system_settings_pkey PRIMARY KEY (id);


--
-- Name: system_settings system_settings_setting_key_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.system_settings
    ADD CONSTRAINT system_settings_setting_key_key UNIQUE (setting_key);


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
-- Name: users users_email_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_email_key UNIQUE (email);


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
-- Name: employees_company_employee_number_active; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX employees_company_employee_number_active ON public.employees USING btree (company_id, employee_number) WHERE (deleted_at IS NULL);


--
-- Name: idx_attendance_company_date; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_attendance_company_date ON public.attendance_records USING btree (company_id, check_in_at DESC);


--
-- Name: idx_attendance_employee; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_attendance_employee ON public.attendance_records USING btree (employee_id);


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
-- Name: idx_audit_logs_created; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_audit_logs_created ON public.audit_logs USING btree (created_at);


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
-- Name: idx_company_settings_category; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_company_settings_category ON public.company_settings USING btree (company_id, category);


--
-- Name: idx_company_settings_company; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_company_settings_company ON public.company_settings USING btree (company_id);


--
-- Name: idx_document_categories_company; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_document_categories_company ON public.document_categories USING btree (company_id);


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
-- Name: idx_eis_rates_lookup; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_eis_rates_lookup ON public.eis_rates USING btree (effective_from, wage_from, wage_to);


--
-- Name: idx_email_logs_company; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_email_logs_company ON public.email_logs USING btree (company_id);


--
-- Name: idx_email_logs_employee; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_email_logs_employee ON public.email_logs USING btree (company_id, employee_id);


--
-- Name: idx_email_logs_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_email_logs_status ON public.email_logs USING btree (status);


--
-- Name: idx_email_templates_company; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_email_templates_company ON public.email_templates USING btree (company_id);


--
-- Name: idx_email_templates_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_email_templates_type ON public.email_templates USING btree (company_id, letter_type);


--
-- Name: idx_employee_allowances; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_employee_allowances ON public.employee_allowances USING btree (employee_id, is_active);


--
-- Name: idx_employee_work_schedules_employee; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_employee_work_schedules_employee ON public.employee_work_schedules USING btree (employee_id);


--
-- Name: idx_employees_active; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_employees_active ON public.employees USING btree (is_active) WHERE (is_active = true);


--
-- Name: idx_employees_company; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_employees_company ON public.employees USING btree (company_id);


--
-- Name: idx_employees_payroll_group; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_employees_payroll_group ON public.employees USING btree (payroll_group_id);


--
-- Name: idx_epf_rates_lookup; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_epf_rates_lookup ON public.epf_rates USING btree (category, effective_from, wage_from, wage_to);


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
-- Name: idx_notifications_user; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_notifications_user ON public.notifications USING btree (user_id, is_read);


--
-- Name: idx_oauth2_accounts_provider; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_oauth2_accounts_provider ON public.oauth2_accounts USING btree (provider, provider_user_id);


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
-- Name: idx_payroll_item_details; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_payroll_item_details ON public.payroll_item_details USING btree (payroll_item_id);


--
-- Name: idx_payroll_items_employee; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_payroll_items_employee ON public.payroll_items USING btree (employee_id);


--
-- Name: idx_payroll_items_run; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_payroll_items_run ON public.payroll_items USING btree (payroll_run_id);


--
-- Name: idx_payroll_runs_company; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_payroll_runs_company ON public.payroll_runs USING btree (company_id);


--
-- Name: idx_payroll_runs_period; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_payroll_runs_period ON public.payroll_runs USING btree (period_year, period_month);


--
-- Name: idx_pcb_brackets_lookup; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_pcb_brackets_lookup ON public.pcb_brackets USING btree (effective_year, chargeable_income_from);


--
-- Name: idx_qr_tokens_company_expires; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_qr_tokens_company_expires ON public.attendance_qr_tokens USING btree (company_id, expires_at);


--
-- Name: idx_qr_tokens_token; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_qr_tokens_token ON public.attendance_qr_tokens USING btree (token);


--
-- Name: idx_refresh_tokens_hash; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_refresh_tokens_hash ON public.refresh_tokens USING btree (token_hash);


--
-- Name: idx_refresh_tokens_user; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_refresh_tokens_user ON public.refresh_tokens USING btree (user_id);


--
-- Name: idx_salary_history_employee; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_salary_history_employee ON public.salary_history USING btree (employee_id);


--
-- Name: idx_socso_rates_lookup; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_socso_rates_lookup ON public.socso_rates USING btree (effective_from, wage_from, wage_to);


--
-- Name: idx_team_members_employee; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_team_members_employee ON public.team_members USING btree (employee_id);


--
-- Name: idx_team_members_team; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_team_members_team ON public.team_members USING btree (team_id);


--
-- Name: idx_teams_company; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_teams_company ON public.teams USING btree (company_id);


--
-- Name: idx_user_companies_company; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_user_companies_company ON public.user_companies USING btree (company_id);


--
-- Name: idx_user_companies_user; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_user_companies_user ON public.user_companies USING btree (user_id);


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
-- Name: working_day_config working_day_config_company_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.working_day_config
    ADD CONSTRAINT working_day_config_company_id_fkey FOREIGN KEY (company_id) REFERENCES public.companies(id);


--
-- PostgreSQL database dump complete
--


