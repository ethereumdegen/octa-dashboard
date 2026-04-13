CREATE TABLE IF NOT EXISTS microservices (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    icon TEXT NOT NULL DEFAULT 'Box',
    nav_path TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT false,
    installed_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Seed the knowledgebase as a built-in microservice (disabled by default)
INSERT INTO microservices (id, name, description, icon, nav_path, enabled)
VALUES ('knowledgebase', 'Knowledgebase', 'Internal wiki and document management', 'Book', '/knowledgebase', false)
ON CONFLICT (id) DO NOTHING;
