BEGIN;
CREATE TABLE pending_project_cleanup (project_id INTEGER PRIMARY KEY);
CREATE TABLE project_id_sequence (last_id INTEGER NOT NULL);
INSERT INTO project_id_sequence SELECT COALESCE(MAX(id), 0) FROM projects;
PRAGMA user_version = 5;
COMMIT;
