-- The Watcher: tracks recent GitHub commits across linked repos and (phase 2)
-- generates LLM summaries for each commit via a background worker.

CREATE TABLE IF NOT EXISTS watcher_repos (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner           TEXT NOT NULL,
    repo            TEXT NOT NULL,
    default_branch  TEXT NOT NULL DEFAULT 'main',
    enabled         BOOLEAN NOT NULL DEFAULT true,
    last_checked_at TIMESTAMPTZ,
    last_error      TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (owner, repo)
);

CREATE TABLE IF NOT EXISTS watcher_commits (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    repo_id        UUID NOT NULL REFERENCES watcher_repos(id) ON DELETE CASCADE,
    sha            TEXT NOT NULL,
    author         TEXT,
    author_email   TEXT,
    message        TEXT,
    url            TEXT,
    committed_at   TIMESTAMPTZ,
    additions      INTEGER,
    deletions      INTEGER,
    files_changed  JSONB,
    summary        TEXT,
    -- pending → done | error : drives the phase-2 summary worker
    summary_status TEXT NOT NULL DEFAULT 'pending',
    raw_data       JSONB,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (repo_id, sha)
);

CREATE INDEX IF NOT EXISTS idx_watcher_commits_committed_at
    ON watcher_commits (committed_at DESC NULLS LAST);

CREATE INDEX IF NOT EXISTS idx_watcher_commits_summary_status
    ON watcher_commits (summary_status)
    WHERE summary_status = 'pending';
