CREATE TABLE IF NOT EXISTS platform_secrets (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_by UUID REFERENCES users(id),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
