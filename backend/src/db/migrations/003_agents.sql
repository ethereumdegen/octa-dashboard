CREATE TABLE agents (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    url TEXT NOT NULL,
    manifest JSONB NOT NULL DEFAULT '{}',
    status TEXT NOT NULL DEFAULT 'unknown',
    last_health_check TIMESTAMPTZ,
    registered_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
