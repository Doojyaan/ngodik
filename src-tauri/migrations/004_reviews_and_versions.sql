BEGIN;
ALTER TABLE draft_runs ADD COLUMN base_version INTEGER;
ALTER TABLE draft_runs ADD COLUMN revision_scope TEXT;
ALTER TABLE draft_runs ADD COLUMN revision_note TEXT;
ALTER TABLE draft_runs ADD COLUMN review_decision TEXT;
CREATE TABLE draft_reviews (
    run_id INTEGER PRIMARY KEY REFERENCES draft_runs(id) ON DELETE CASCADE,
    semantic_started INTEGER NOT NULL DEFAULT 0,
    report TEXT
);
CREATE TABLE blueprint_versions (
    id INTEGER PRIMARY KEY,
    project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    run_id INTEGER NOT NULL REFERENCES draft_runs(id),
    number INTEGER NOT NULL,
    parent_id INTEGER REFERENCES blueprint_versions(id),
    restored_from INTEGER REFERENCES blueprint_versions(id),
    created_at INTEGER NOT NULL,
    summary TEXT NOT NULL,
    recovery_file TEXT,
    recovery_hash BLOB,
    UNIQUE (project_id, number)
);
CREATE INDEX blueprint_versions_project ON blueprint_versions(project_id, number DESC);
PRAGMA user_version=4;
COMMIT;
