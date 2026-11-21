BEGIN;
CREATE TABLE IF NOT EXISTS draft_runs (
    id INTEGER PRIMARY KEY,
    project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    idea_revision INTEGER NOT NULL,
    idea TEXT NOT NULL,
    target TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('running','completed','failed','canceled','interrupted')),
    error TEXT
);
CREATE INDEX IF NOT EXISTS draft_runs_project ON draft_runs(project_id,id DESC);
CREATE UNIQUE INDEX IF NOT EXISTS draft_one_running ON draft_runs(project_id) WHERE status='running';
CREATE TABLE IF NOT EXISTS draft_stages (
    run_id INTEGER NOT NULL REFERENCES draft_runs(id) ON DELETE CASCADE,
    stage INTEGER NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('waiting','running','completed','failed','canceled','interrupted')),
    attempts INTEGER NOT NULL DEFAULT 0,
    artifact TEXT,
    error TEXT,
    PRIMARY KEY (run_id,stage)
);
PRAGMA user_version=3;
COMMIT;
