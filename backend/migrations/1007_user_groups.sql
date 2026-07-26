-- User groups: a named, company-scoped bundle of permissions that can be
-- granted to users on top of their roles.
--
-- Roles stay fixed and code-defined (`core::permission`) because payroll
-- separation of duties depends on them being reviewable in one place. Groups
-- are the escape hatch for the cases roles cannot express: "the two people who
-- also handle the calendar", "contractors who may read attendance but nothing
-- else". Effective permissions are the union of role grants and group grants,
-- so a group can only ever *add* — there is no negative grant, and no way to
-- use a group to strip a permission a role confers. That keeps the answer to
-- "why can this person do X?" additive and therefore searchable.

CREATE TABLE public.user_groups (
    id uuid DEFAULT uuidv7() NOT NULL,
    company_id uuid NOT NULL,
    name character varying(100) NOT NULL,
    description text,
    is_active boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    created_by uuid,
    updated_by uuid,
    CONSTRAINT user_groups_pkey PRIMARY KEY (id),
    CONSTRAINT user_groups_company_id_fkey
        FOREIGN KEY (company_id) REFERENCES public.companies(id) ON DELETE CASCADE,
    CONSTRAINT user_groups_created_by_fkey
        FOREIGN KEY (created_by) REFERENCES public.users(id) ON DELETE SET NULL,
    CONSTRAINT user_groups_updated_by_fkey
        FOREIGN KEY (updated_by) REFERENCES public.users(id) ON DELETE SET NULL,
    CONSTRAINT user_groups_company_name_key UNIQUE (company_id, name),
    CONSTRAINT user_groups_name_not_blank CHECK (btrim(name) <> '')
);

-- Permission keys are validated against `Permission::as_str()` in the service
-- layer rather than by a CHECK constraint: the enum is the source of truth and
-- a constraint here would be a second copy that has to be migrated in lockstep
-- every time a capability is added. `user_group_permissions_known_keys` in the
-- schema invariant tests asserts every stored key still resolves.
CREATE TABLE public.user_group_permissions (
    group_id uuid NOT NULL,
    permission character varying(64) NOT NULL,
    granted_at timestamp with time zone DEFAULT now() NOT NULL,
    granted_by uuid,
    CONSTRAINT user_group_permissions_pkey PRIMARY KEY (group_id, permission),
    CONSTRAINT user_group_permissions_group_id_fkey
        FOREIGN KEY (group_id) REFERENCES public.user_groups(id) ON DELETE CASCADE,
    CONSTRAINT user_group_permissions_granted_by_fkey
        FOREIGN KEY (granted_by) REFERENCES public.users(id) ON DELETE SET NULL
);

CREATE TABLE public.user_group_members (
    group_id uuid NOT NULL,
    user_id uuid NOT NULL,
    added_at timestamp with time zone DEFAULT now() NOT NULL,
    added_by uuid,
    CONSTRAINT user_group_members_pkey PRIMARY KEY (group_id, user_id),
    CONSTRAINT user_group_members_group_id_fkey
        FOREIGN KEY (group_id) REFERENCES public.user_groups(id) ON DELETE CASCADE,
    CONSTRAINT user_group_members_user_id_fkey
        FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE,
    CONSTRAINT user_group_members_added_by_fkey
        FOREIGN KEY (added_by) REFERENCES public.users(id) ON DELETE SET NULL
);

-- The permission resolver runs on every authenticated request, keyed by
-- (user_id, company_id) — see `user_groups::effective_permissions`. This index
-- is what keeps that from becoming a sequential scan of every membership.
CREATE INDEX idx_user_group_members_user ON public.user_group_members USING btree (user_id);
CREATE INDEX idx_user_groups_company_active
    ON public.user_groups USING btree (company_id)
    WHERE (is_active = true);

-- A group grants permissions inside one company, so its members must actually
-- belong to that company. Without this, adding a user from company B to
-- company A's group would grant them company A's capabilities the moment they
-- switched context. Mirrors `enforce_team_member_company` for `team_members`.
CREATE OR REPLACE FUNCTION public.enforce_user_group_member_company()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = public, pg_temp
AS $user_group_member_company$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM user_groups grp
        JOIN user_companies uc
          ON uc.user_id = NEW.user_id
         AND uc.company_id = grp.company_id
        WHERE grp.id = NEW.group_id
    ) THEN
        RAISE EXCEPTION 'User must belong to the group''s company'
            USING ERRCODE = '23514', CONSTRAINT = 'user_group_members_same_company_check';
    END IF;
    RETURN NEW;
END
$user_group_member_company$;

DROP TRIGGER IF EXISTS user_group_members_same_company_trigger ON public.user_group_members;
CREATE TRIGGER user_group_members_same_company_trigger
    BEFORE INSERT OR UPDATE ON public.user_group_members
    FOR EACH ROW EXECUTE FUNCTION public.enforce_user_group_member_company();

-- Tenant ownership of a group is fixed at creation, like every other
-- company-scoped table.
DROP TRIGGER IF EXISTS user_groups_company_immutable_trigger ON public.user_groups;
CREATE TRIGGER user_groups_company_immutable_trigger
    BEFORE UPDATE OF company_id ON public.user_groups
    FOR EACH ROW EXECUTE FUNCTION public.enforce_immutable_company_id();
