-- Persist per-feed publication state. A feed only emits events it includes now,
-- or cancellation tombstones for events it actually published previously.
CREATE TABLE calendar_publications (
    feed_id uuid NOT NULL REFERENCES calendar_feeds(id) ON DELETE CASCADE,
    event_id uuid NOT NULL REFERENCES calendar_events(id),
    representation_hash text NOT NULL,
    sequence integer NOT NULL DEFAULT 0 CHECK (sequence >= 0),
    last_date date NOT NULL,
    cancelled boolean NOT NULL DEFAULT false,
    modified_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY(feed_id, event_id)
);
CREATE INDEX calendar_publications_date ON calendar_publications(feed_id, last_date);
CREATE TABLE worker_heartbeats (
    role text PRIMARY KEY,
    last_success_at timestamptz NOT NULL DEFAULT now()
);
