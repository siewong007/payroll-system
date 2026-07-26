-- Bind attendance to the office network, and learn which network that is.
--
-- The control answers "was this employee actually on the company's network when
-- they checked in?" A browser cannot read a WiFi SSID or BSSID — no such API
-- exists — so any network name a client sends is self-declared and forgeable
-- with one line in devtools. The only network signal the server can trust is
-- the one it observes itself: the client address resolved by `core::client_ip`
-- (right-most X-Forwarded-For entry, and only when TRUST_PROXY_HEADERS says a
-- trusted proxy is the sole path in). Everything below is built on that address
-- and nothing else. The human-readable "HQ WiFi" name lives in `label` on the
-- approved row, set by an administrator — never taken from the client.
--
-- Two tables, because approving a network and observing one are different
-- trust levels and must not share a row:
--
--   company_networks                  — the allow-list. Only an administrator
--                                       writes here, and only ever explicitly.
--   attendance_network_observations   — what has been seen. Evidence for a
--                                       proposal; never itself authorizes.
--
-- Nothing is ever promoted from the second to the first automatically. An
-- employee who checks in from home during rollout would otherwise teach the
-- system that their living room is the office.

-- ─── Mode ───

-- 'none'    — not in use; nothing observed, nothing checked.
-- 'learn'   — observe and record only. Never blocks, never flags. This is the
--             rollout phase: run it for a fortnight, then approve what it found.
-- 'warn'    — check, allow anyway, flag the record for review.
-- 'enforce' — check, and refuse a check-in from an unapproved network.
--
-- Deliberately mirrors companies.geofence_mode, with 'learn' added ahead of
-- 'warn' because a network allow-list — unlike a geofence — cannot be
-- configured from a map before it is switched on.
ALTER TABLE public.companies
    ADD COLUMN IF NOT EXISTS attendance_network_mode character varying(20)
        DEFAULT 'none' NOT NULL;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'companies_attendance_network_mode_check'
          AND conrelid = 'public.companies'::regclass
    ) THEN
        ALTER TABLE public.companies
            ADD CONSTRAINT companies_attendance_network_mode_check
            CHECK (attendance_network_mode IN ('none', 'learn', 'warn', 'enforce'));
    END IF;
END
$$;

-- ─── The allow-list ───

CREATE TABLE IF NOT EXISTS public.company_networks (
    id uuid DEFAULT uuidv7() NOT NULL,
    company_id uuid NOT NULL,
    -- What a human calls it. This is where "HQ WiFi" lives: an administrator
    -- types it against a prefix they approved, so the friendly name is bound to
    -- something verifiable instead of being asserted by the device checking in.
    label character varying(150) NOT NULL,
    -- Canonical network address with host bits already cleared, e.g.
    -- '203.0.113.0'. Stored as text with a separate prefix length rather than
    -- as inet/cidr: matching happens in Rust (core::ip_prefix), which keeps the
    -- policy — carrier-NAT rejection, minimum prefix width — under unit test
    -- instead of inside a SQL operator, and avoids adding an sqlx type feature.
    network text NOT NULL,
    prefix_len smallint NOT NULL,
    is_active boolean DEFAULT true NOT NULL,
    -- Provenance. `approved_by` is the administrator who accepted it; NULL only
    -- for rows a future import creates. `learned_from_observation` records that
    -- this began as a proposal rather than a typed-in value, which is worth
    -- knowing when auditing why a network is trusted.
    approved_by uuid,
    approved_at timestamp with time zone DEFAULT now() NOT NULL,
    learned_from_observation boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT company_networks_pkey PRIMARY KEY (id),
    CONSTRAINT company_networks_company_id_fkey
        FOREIGN KEY (company_id) REFERENCES public.companies(id) ON DELETE CASCADE,
    CONSTRAINT company_networks_approved_by_fkey
        FOREIGN KEY (approved_by) REFERENCES public.users(id) ON DELETE SET NULL,
    -- A width floor in the database as well as in Rust. The application refuses
    -- anything broader (/20 v4, /48 v6), but a stray migration or a psql session
    -- must not be able to insert 0.0.0.0/0 and silently disable the control
    -- while the UI still reads "enforced".
    CONSTRAINT company_networks_prefix_len_check
        CHECK (prefix_len BETWEEN 16 AND 128)
);

-- One row per network per company: approving the same block twice would mean
-- deactivating one copy did nothing.
CREATE UNIQUE INDEX IF NOT EXISTS idx_company_networks_unique
    ON public.company_networks USING btree (company_id, network, prefix_len);

-- The check-in hot path reads only the active rows for one company.
CREATE INDEX IF NOT EXISTS idx_company_networks_active
    ON public.company_networks USING btree (company_id)
    WHERE is_active = true;

-- ─── The evidence ───

-- One row per (company, employee, candidate network). Keeping the employee in
-- the key is what makes "how many *different* people have we seen here?"
-- answerable — the single threshold that stops one person's home broadband
-- from looking like an office.
--
-- PDPA: these rows are employees' home and mobile IP addresses. Three
-- consequences, all enforced elsewhere and noted here so the table is not
-- mistaken for a general-purpose log:
--   1. Rows are only written while the mode is 'learn', 'warn' or 'enforce'.
--   2. The daily cleanup task purges rows older than the learning window.
--   3. No endpoint exposes the employee_id → network mapping. Administrators
--      see aggregates (a network, a count of distinct employees, a date range),
--      never "Aisyah checked in from 203.0.113.5".
CREATE TABLE IF NOT EXISTS public.attendance_network_observations (
    id uuid DEFAULT uuidv7() NOT NULL,
    company_id uuid NOT NULL,
    employee_id uuid NOT NULL,
    network text NOT NULL,
    prefix_len smallint NOT NULL,
    observation_count integer DEFAULT 1 NOT NULL,
    -- Observations corroborated by a signal the employee does not control:
    -- the check-in was inside an active geofence, or its QR token was minted by
    -- a kiosk credential, which means a device physically in the office
    -- displayed the code. Only these count toward proposing a network, so a
    -- check-in from home cannot promote itself however often it is repeated.
    anchored_count integer DEFAULT 0 NOT NULL,
    first_seen_at timestamp with time zone DEFAULT now() NOT NULL,
    last_seen_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT attendance_network_observations_pkey PRIMARY KEY (id),
    CONSTRAINT attendance_network_observations_company_id_fkey
        FOREIGN KEY (company_id) REFERENCES public.companies(id) ON DELETE CASCADE,
    CONSTRAINT attendance_network_observations_employee_id_fkey
        FOREIGN KEY (employee_id) REFERENCES public.employees(id) ON DELETE CASCADE,
    CONSTRAINT attendance_network_observations_counts_check
        CHECK (observation_count > 0 AND anchored_count >= 0
               AND anchored_count <= observation_count)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_attendance_network_obs_unique
    ON public.attendance_network_observations
    USING btree (company_id, employee_id, network, prefix_len);

-- Candidate aggregation groups by network within a company.
CREATE INDEX IF NOT EXISTS idx_attendance_network_obs_candidates
    ON public.attendance_network_observations
    USING btree (company_id, network, prefix_len);

-- The retention purge scans by age.
CREATE INDEX IF NOT EXISTS idx_attendance_network_obs_last_seen
    ON public.attendance_network_observations USING btree (last_seen_at);

-- ─── Dismissed candidates ───

-- "Stop proposing this." Without it, an administrator who declines a candidate
-- is shown it again the next day, forever, and eventually approves it to make
-- the badge go away.
CREATE TABLE IF NOT EXISTS public.attendance_network_dismissals (
    id uuid DEFAULT uuidv7() NOT NULL,
    company_id uuid NOT NULL,
    network text NOT NULL,
    prefix_len smallint NOT NULL,
    dismissed_by uuid,
    dismissed_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT attendance_network_dismissals_pkey PRIMARY KEY (id),
    CONSTRAINT attendance_network_dismissals_company_id_fkey
        FOREIGN KEY (company_id) REFERENCES public.companies(id) ON DELETE CASCADE,
    CONSTRAINT attendance_network_dismissals_dismissed_by_fkey
        FOREIGN KEY (dismissed_by) REFERENCES public.users(id) ON DELETE SET NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_attendance_network_dismissals_unique
    ON public.attendance_network_dismissals
    USING btree (company_id, network, prefix_len);

-- ─── Flagging on the record ───

-- Parallel to is_outside_geofence: NULL means "not evaluated" (mode was 'none'
-- or 'learn' at the time), true means the address did not match any approved
-- network. A check-out from off-network ORs into it, exactly as the geofence
-- flag does, so an employee who leaves the building before checking out is
-- visible without being blocked.
ALTER TABLE public.attendance_records
    ADD COLUMN IF NOT EXISTS is_offsite_network boolean;

-- The off-network triage filter on the admin list, mirroring the geofence one.
CREATE INDEX IF NOT EXISTS idx_attendance_records_offsite_network
    ON public.attendance_records USING btree (company_id, check_in_at)
    WHERE is_offsite_network = true;
