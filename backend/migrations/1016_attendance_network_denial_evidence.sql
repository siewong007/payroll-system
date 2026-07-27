-- Denial evidence and dismissal expiry for attendance network binding.
--
-- Follow-on to `1010_attendance_networks.sql`, which is already deployed and
-- therefore immutable — these columns cannot be folded back into it without
-- changing its checksum and wedging `sqlx::migrate!` at startup.
--
-- What this control is, stated plainly so nobody mistakes it for more: an
-- egress prefix proves network *path*, not physical presence. An SSH tunnel or
-- a VPN exit node left running on any office machine produces a genuinely
-- office-originated packet, a full-tunnel corporate VPN certifies every remote
-- worker as present, and a shared building uplink admits the café downstairs.
-- This is an HR control that raises casual remote check-in from free to
-- "maintain a persistent office foothold", and it produces good evidence for
-- the existing exception-triage workflow. It is not an authorization boundary.

-- ─── Denial evidence ───
--
-- A denied check-in is the *only* evidence that exists when the office egress
-- address changes. Learning normally runs after a successful check-in, so on
-- the morning the ISP hands out a new address every employee is refused, the
-- success path never runs, and the new prefix is recorded nowhere — leaving a
-- company-wide lockout with nothing to recover from except an administrator
-- discovering the address by other means.
--
-- Counted separately from `observation_count` because a denial is emphatically
-- not corroboration. It can only ever feed the "your office address may have
-- changed" prompt, never the anchored thresholds that make a block proposable.
ALTER TABLE public.attendance_network_observations
    ADD COLUMN IF NOT EXISTS denied_count integer DEFAULT 0 NOT NULL;

ALTER TABLE public.attendance_network_observations
    DROP CONSTRAINT IF EXISTS attendance_network_observations_counts_check;

ALTER TABLE public.attendance_network_observations
    ADD CONSTRAINT attendance_network_observations_counts_check
    CHECK (observation_count >= 0
           AND anchored_count >= 0
           AND denied_count >= 0
           AND anchored_count <= observation_count);

-- ─── Dismissal expiry ───
--
-- A dismissal means "not right now", not "never again". An office that moves,
-- or a PPPoE lease that later returns to a block someone once declined, must
-- become proposable again rather than stay suppressed forever by a click made
-- months earlier. The daily cleanup purges lapsed rows.
ALTER TABLE public.attendance_network_dismissals
    ADD COLUMN IF NOT EXISTS expires_at timestamp with time zone
        DEFAULT (now() + interval '180 days') NOT NULL;

CREATE INDEX IF NOT EXISTS idx_attendance_network_dismissals_expiry
    ON public.attendance_network_dismissals USING btree (expires_at);
