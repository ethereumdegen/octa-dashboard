-- Knowledgebase RAG microservice (modeled on solarabase, global/shared scope, no login).
-- Solarabase table names kept verbatim (no collision with existing teller tables) so the
-- ported sqlx query layer needs no SQL edits. Multi-tenant FKs (workspaces/users) dropped;
-- user-id columns kept as nullable UUID (no FK) populated from the wrapper's x-user-id header.

-- ── Knowledgebases ───────────────────────────────────────────────────────────
CREATE TABLE knowledgebases (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name            TEXT NOT NULL,
    slug            TEXT NOT NULL UNIQUE,
    description     TEXT NOT NULL DEFAULT '',
    system_prompt   TEXT NOT NULL DEFAULT '',
    model           TEXT NOT NULL DEFAULT 'gpt-4o',
    accent_color    TEXT NOT NULL DEFAULT '#111827',
    logo_url        TEXT,
    created_by      UUID,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- ── Folders ──────────────────────────────────────────────────────────────────
CREATE TABLE doc_folders (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    kb_id       UUID NOT NULL REFERENCES knowledgebases(id) ON DELETE CASCADE,
    parent_id   UUID REFERENCES doc_folders(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    category    TEXT,
    created_by  UUID,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX uq_doc_folders_root_name ON doc_folders (kb_id, name) WHERE parent_id IS NULL;
CREATE UNIQUE INDEX uq_doc_folders_nested_name ON doc_folders (kb_id, parent_id, name) WHERE parent_id IS NOT NULL;
CREATE INDEX idx_doc_folders_kb ON doc_folders (kb_id);
CREATE INDEX idx_doc_folders_parent ON doc_folders (parent_id);

-- ── Documents + PageIndex ────────────────────────────────────────────────────
CREATE TYPE doc_status AS ENUM ('uploaded', 'processing', 'indexed', 'failed');

CREATE TABLE documents (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    kb_id       UUID NOT NULL REFERENCES knowledgebases(id) ON DELETE CASCADE,
    folder_id   UUID REFERENCES doc_folders(id) ON DELETE SET NULL,
    filename    TEXT NOT NULL,
    mime_type   TEXT NOT NULL,
    s3_key      TEXT NOT NULL UNIQUE,
    size_bytes  BIGINT NOT NULL,
    status      doc_status NOT NULL DEFAULT 'uploaded',
    page_count  INT,
    error_msg   TEXT,
    uploaded_by UUID,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_documents_status ON documents(status);
CREATE INDEX idx_documents_kb ON documents(kb_id);
CREATE INDEX idx_documents_folder ON documents(folder_id);

CREATE TABLE page_indexes (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    page_num    INT NOT NULL,
    content     TEXT NOT NULL,
    tree_index  JSONB NOT NULL,
    content_tsv tsvector GENERATED ALWAYS AS (to_tsvector('english', content)) STORED,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(document_id, page_num)
);
CREATE INDEX idx_page_indexes_doc ON page_indexes(document_id);
CREATE INDEX idx_page_content_fts ON page_indexes USING GIN(content_tsv);

CREATE TABLE document_indexes (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE UNIQUE,
    root_index  JSONB NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- ── Wiki (auto-generated) ────────────────────────────────────────────────────
CREATE TABLE wiki_pages (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    kb_id          UUID NOT NULL REFERENCES knowledgebases(id) ON DELETE CASCADE,
    document_id    UUID REFERENCES documents(id) ON DELETE SET NULL,
    slug           TEXT NOT NULL,
    title          TEXT NOT NULL,
    summary        TEXT,
    content_s3_key TEXT NOT NULL,
    page_type      TEXT NOT NULL DEFAULT 'concept',
    sources        JSONB NOT NULL DEFAULT '[]',
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(kb_id, slug)
);
CREATE INDEX idx_wiki_pages_kb ON wiki_pages(kb_id);

-- ── Chat sessions / messages / job queue ─────────────────────────────────────
CREATE TABLE chat_sessions (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    kb_id       UUID NOT NULL REFERENCES knowledgebases(id) ON DELETE CASCADE,
    user_id     UUID,
    title       TEXT NOT NULL DEFAULT 'New Chat',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_chat_sessions_kb ON chat_sessions(kb_id);
CREATE INDEX idx_chat_sessions_user ON chat_sessions(user_id);

CREATE TYPE chat_role AS ENUM ('user', 'assistant');

CREATE TABLE chat_messages (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id  UUID NOT NULL REFERENCES chat_sessions(id) ON DELETE CASCADE,
    role        chat_role NOT NULL,
    content     TEXT NOT NULL,
    metadata    JSONB,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_chat_messages_session ON chat_messages(session_id);

CREATE TABLE chat_jobs (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id   UUID NOT NULL REFERENCES chat_sessions(id) ON DELETE CASCADE,
    kb_id        UUID NOT NULL REFERENCES knowledgebases(id) ON DELETE CASCADE,
    owner_id     UUID,
    status       TEXT NOT NULL DEFAULT 'ready',
    worker_id    TEXT,
    content      TEXT NOT NULL,
    error        TEXT,
    claimed_at   TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_chat_jobs_status ON chat_jobs(status);
CREATE INDEX idx_chat_jobs_session ON chat_jobs(session_id);
