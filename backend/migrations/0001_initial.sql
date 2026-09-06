CREATE TABLE users (
    id uuid PRIMARY KEY,
    email text NOT NULL UNIQUE CHECK (length(email) <= 254),
    password_hash text NOT NULL,
    timezone text NOT NULL DEFAULT 'Asia/Shanghai',
    is_admin boolean NOT NULL DEFAULT false,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE sessions (
    token_hash text PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at timestamptz NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX sessions_user ON sessions(user_id);
CREATE INDEX sessions_expiry ON sessions(expires_at);
CREATE TABLE rate_limits (
    key text NOT NULL,
    bucket bigint NOT NULL,
    hits integer NOT NULL DEFAULT 1,
    PRIMARY KEY(key, bucket)
);
CREATE TABLE cards (
    id uuid PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES users(id),
    data jsonb NOT NULL CHECK (jsonb_typeof(data) = 'object'),
    active boolean NOT NULL DEFAULT true,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE(id, user_id)
);
CREATE INDEX cards_owner ON cards(user_id);
CREATE TABLE card_cycles (
    id uuid PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES users(id),
    card_id uuid NOT NULL,
    cycle_month date NOT NULL CHECK (extract(day FROM cycle_month) = 1),
    statement_date date NOT NULL,
    due_date date NOT NULL,
    statement_overridden boolean NOT NULL DEFAULT false,
    due_overridden boolean NOT NULL DEFAULT false,
    amount numeric(18,2) CHECK (amount >= 0),
    minimum_payment numeric(18,2) CHECK (minimum_payment >= 0),
    paid_at timestamptz,
    note text NOT NULL DEFAULT '',
    revision integer NOT NULL DEFAULT 0,
    updated_at timestamptz NOT NULL DEFAULT now(),
    FOREIGN KEY(card_id, user_id) REFERENCES cards(id, user_id),
    UNIQUE(card_id, cycle_month),
    UNIQUE(id, user_id),
    CHECK (due_date >= statement_date)
);
CREATE INDEX cycles_owner_due ON card_cycles(user_id, due_date);
CREATE TABLE card_milestones (
    id uuid PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES users(id),
    card_id uuid NOT NULL,
    data jsonb NOT NULL CHECK (jsonb_typeof(data) = 'object'),
    updated_at timestamptz NOT NULL DEFAULT now(),
    FOREIGN KEY(card_id, user_id) REFERENCES cards(id, user_id),
    UNIQUE(id, user_id)
);
CREATE TABLE calendar_events (
    id uuid PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES users(id),
    card_id uuid NOT NULL,
    cycle_id uuid,
    kind text NOT NULL,
    title text NOT NULL,
    event_date date NOT NULL,
    paid boolean NOT NULL DEFAULT false,
    active boolean NOT NULL DEFAULT true,
    revision integer NOT NULL DEFAULT 0,
    updated_at timestamptz NOT NULL DEFAULT now(),
    FOREIGN KEY(card_id, user_id) REFERENCES cards(id, user_id),
    FOREIGN KEY(cycle_id, user_id) REFERENCES card_cycles(id, user_id),
    UNIQUE(id, user_id)
);
CREATE INDEX events_owner_date ON calendar_events(user_id, event_date);
CREATE TABLE calendar_feeds (
    id uuid PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES users(id),
    data jsonb NOT NULL CHECK (jsonb_typeof(data) = 'object'),
    token_version integer NOT NULL DEFAULT 1 CHECK (token_version > 0),
    updated_at timestamptz NOT NULL DEFAULT now(),
    last_accessed_at timestamptz
);
CREATE INDEX feeds_owner ON calendar_feeds(user_id);
CREATE TABLE notification_connections (
    id uuid PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES users(id),
    name text NOT NULL,
    kind text NOT NULL CHECK (kind IN ('bark','email','telegram','webpush','webhook')),
    encrypted_config text NOT NULL,
    enabled boolean NOT NULL DEFAULT true,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE(id, user_id)
);
CREATE TABLE notification_templates (
    id uuid PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES users(id),
    data jsonb NOT NULL CHECK (jsonb_typeof(data) = 'object'),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE(id, user_id)
);
CREATE TABLE notification_rules (
    id uuid PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES users(id),
    data jsonb NOT NULL CHECK (jsonb_typeof(data) = 'object'),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE(id, user_id)
);
CREATE TABLE notification_jobs (
    id uuid PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES users(id),
    rule_id uuid,
    event_id uuid,
    connection_id uuid NOT NULL,
    offset_days integer NOT NULL DEFAULT 0,
    scheduled_at timestamptz NOT NULL,
    next_attempt_at timestamptz NOT NULL,
    expires_at timestamptz NOT NULL,
    status text NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','processing','retry','sent','cancelled','dead','expired')),
    attempt_count integer NOT NULL DEFAULT 0,
    lease_owner uuid,
    lease_until timestamptz,
    template_snapshot jsonb NOT NULL,
    context jsonb NOT NULL,
    plan_token uuid,
    last_error text,
    created_at timestamptz NOT NULL DEFAULT now(),
    sent_at timestamptz,
    FOREIGN KEY(rule_id, user_id) REFERENCES notification_rules(id, user_id),
    FOREIGN KEY(event_id, user_id) REFERENCES calendar_events(id, user_id),
    FOREIGN KEY(connection_id, user_id) REFERENCES notification_connections(id, user_id),
    UNIQUE(rule_id, event_id, connection_id, offset_days)
);
CREATE INDEX jobs_due ON notification_jobs(next_attempt_at) WHERE status IN ('pending','retry');
CREATE INDEX jobs_owner ON notification_jobs(user_id, created_at DESC);
CREATE INDEX jobs_lease ON notification_jobs(lease_until) WHERE status = 'processing';
CREATE TABLE delivery_attempts (
    id bigserial PRIMARY KEY,
    job_id uuid NOT NULL REFERENCES notification_jobs(id),
    started_at timestamptz NOT NULL DEFAULT now(),
    finished_at timestamptz NOT NULL DEFAULT now(),
    success boolean NOT NULL,
    response_status integer,
    error_code text,
    rendered jsonb NOT NULL
);
CREATE INDEX attempts_job ON delivery_attempts(job_id);
CREATE TABLE audit_logs (
    id bigserial PRIMARY KEY,
    user_id uuid REFERENCES users(id),
    action text NOT NULL,
    entity_id uuid,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX audits_owner_time ON audit_logs(user_id, created_at DESC);
