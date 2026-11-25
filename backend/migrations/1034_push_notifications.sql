-- Web Push subscriptions and durable push delivery outbox.

CREATE TABLE public.push_subscriptions (
    id uuid DEFAULT uuidv7() NOT NULL PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES public.users(id) ON DELETE CASCADE,
    company_id uuid REFERENCES public.companies(id) ON DELETE SET NULL,
    session_id uuid REFERENCES public.user_sessions(id) ON DELETE CASCADE,
    endpoint text NOT NULL,
    p256dh text NOT NULL,
    auth text NOT NULL,
    user_agent text,
    failure_count integer NOT NULL DEFAULT 0,
    last_error text,
    last_seen_at timestamp with time zone NOT NULL DEFAULT now(),
    revoked_at timestamp with time zone,
    created_at timestamp with time zone NOT NULL DEFAULT now(),
    updated_at timestamp with time zone NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_push_subscriptions_endpoint
    ON public.push_subscriptions (endpoint);

CREATE INDEX IF NOT EXISTS idx_push_subscriptions_user_active
    ON public.push_subscriptions (user_id)
    WHERE revoked_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_push_subscriptions_session_active
    ON public.push_subscriptions (session_id)
    WHERE revoked_at IS NULL AND session_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_push_subscriptions_revoked_at
    ON public.push_subscriptions (revoked_at)
    WHERE revoked_at IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_push_subscriptions_last_seen_at
    ON public.push_subscriptions (last_seen_at);

CREATE TABLE public.push_outbox (
    id uuid DEFAULT uuidv7() NOT NULL PRIMARY KEY,
    subscription_id uuid NOT NULL REFERENCES public.push_subscriptions(id) ON DELETE CASCADE,
    user_id uuid NOT NULL REFERENCES public.users(id) ON DELETE CASCADE,
    company_id uuid REFERENCES public.companies(id) ON DELETE CASCADE,
    notification_id uuid REFERENCES public.notifications(id) ON DELETE SET NULL,
    title character varying(255) NOT NULL,
    body text NOT NULL,
    url character varying(255) NOT NULL DEFAULT '/portal/notifications',
    tag character varying(100),
    status character varying(20) NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'sent', 'failed', 'gone')),
    attempts integer NOT NULL DEFAULT 0,
    max_attempts integer NOT NULL DEFAULT 4,
    next_retry_at timestamp with time zone NOT NULL DEFAULT now(),
    last_error text,
    sent_at timestamp with time zone,
    created_at timestamp with time zone NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_push_outbox_pending
    ON public.push_outbox (next_retry_at, created_at)
    WHERE status = 'pending';

CREATE INDEX IF NOT EXISTS idx_push_outbox_created_at
    ON public.push_outbox (created_at);

CREATE INDEX IF NOT EXISTS idx_push_outbox_failed_created
    ON public.push_outbox (created_at)
    WHERE status = 'failed';
