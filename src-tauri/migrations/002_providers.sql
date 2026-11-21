BEGIN;
CREATE TABLE IF NOT EXISTS provider_profiles (
    id TEXT PRIMARY KEY,
    label TEXT NOT NULL,
    base_url TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS project_models (
    project_id INTEGER PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
    profile_id TEXT NOT NULL REFERENCES provider_profiles(id) ON DELETE CASCADE,
    model_id TEXT NOT NULL
);
PRAGMA user_version = 2;
COMMIT;
