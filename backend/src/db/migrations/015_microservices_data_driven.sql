-- Make microservices fully data-driven from agent discovery.
-- Going forward, microservice rows are auto-created when agents are discovered,
-- not hand-seeded in migration files.

-- Add slug (used for /app/{slug} routing), source_url, and agent_id
ALTER TABLE microservices ADD COLUMN IF NOT EXISTS slug TEXT;
ALTER TABLE microservices ADD COLUMN IF NOT EXISTS source_url TEXT;

-- Backfill slug from existing nav_path (strip leading /)
UPDATE microservices SET slug = LTRIM(nav_path, '/') WHERE slug IS NULL;

-- Make slug NOT NULL with a unique constraint going forward
ALTER TABLE microservices ALTER COLUMN slug SET NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_microservices_slug ON microservices(slug);

-- Update nav_path to use /app/{slug} pattern for all existing rows
UPDATE microservices SET nav_path = '/app/' || slug;
