CREATE TABLE projects (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Seed a default project
INSERT INTO projects (id, name) VALUES ('00000000-0000-0000-0000-000000000001', 'Default');

-- Migrate api_keys from user_id to project_id
ALTER TABLE api_keys ADD COLUMN project_id UUID REFERENCES projects(id) ON DELETE CASCADE;
UPDATE api_keys SET project_id = '00000000-0000-0000-0000-000000000001';
ALTER TABLE api_keys ALTER COLUMN project_id SET NOT NULL;
ALTER TABLE api_keys DROP COLUMN user_id;

CREATE INDEX idx_api_keys_project ON api_keys(project_id);
DROP INDEX IF EXISTS idx_api_keys_user;
