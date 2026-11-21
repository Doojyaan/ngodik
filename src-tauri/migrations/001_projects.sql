BEGIN;
CREATE TABLE IF NOT EXISTS projects (
    id INTEGER PRIMARY KEY,
    title TEXT NOT NULL,
    idea TEXT NOT NULL DEFAULT '',
    revision INTEGER NOT NULL DEFAULT 0,
    updated_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS idea_checkpoints (
    project_id INTEGER NOT NULL REFERENCES projects(id),
    revision INTEGER NOT NULL,
    idea TEXT NOT NULL,
    saved_at INTEGER NOT NULL,
    PRIMARY KEY (project_id, revision)
);
PRAGMA user_version = 1;
COMMIT;
