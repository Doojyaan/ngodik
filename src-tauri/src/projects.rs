use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

#[cfg(test)]
mod tests;

const MAX_IDEA_BYTES: usize = 16 * 1024;

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectError {
    InvalidTitle,
    IdeaTooLong,
    NotFound,
    ChangedElsewhere,
    StorageUnavailable,
}

impl From<rusqlite::Error> for ProjectError {
    fn from(_: rusqlite::Error) -> Self {
        Self::StorageUnavailable
    }
}

type Result<T> = std::result::Result<T, ProjectError>;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    id: i64,
    title: String,
    updated_at: i64,
    draft_status: Option<String>,
    completed_stages: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Checkpoint {
    revision: i64,
    saved_at: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    id: i64,
    title: String,
    pub(super) idea: String,
    pub(super) revision: i64,
    history: Vec<Checkpoint>,
}

pub struct ProjectStore {
    pub(super) connection: Connection,
}

impl ProjectStore {
    pub fn open(path: &Path) -> Result<Self> {
        let connection = Connection::open(path)?;
        connection.busy_timeout(std::time::Duration::from_secs(3))?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        let schema: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if schema == 0 {
            connection.execute_batch(include_str!("../migrations/001_projects.sql"))?;
        }
        match schema {
            0 | 1 => connection.execute_batch(include_str!("../migrations/002_providers.sql"))?,
            2 | 3 => {}
            _ => return Err(ProjectError::StorageUnavailable),
        }
        if schema <= 2 {
            connection.execute_batch(include_str!("../migrations/003_drafts.sql"))?;
        }
        Ok(Self { connection })
    }

    pub fn list(&self) -> Result<Vec<ProjectSummary>> {
        let mut query = self.connection.prepare(
            "SELECT p.id, p.title, p.updated_at, run.status,
                 (SELECT COUNT(*) FROM draft_stages WHERE run_id=run.id AND status='completed')
             FROM projects p
             LEFT JOIN draft_runs run ON run.id=(SELECT id FROM draft_runs WHERE project_id=p.id ORDER BY id DESC LIMIT 1)
             ORDER BY p.updated_at DESC,p.id DESC",
        )?;
        let projects = query.query_map([], |row| {
            Ok(ProjectSummary {
                id: row.get(0)?,
                title: row.get(1)?,
                updated_at: row.get(2)?,
                draft_status: row.get(3)?,
                completed_stages: row.get(4)?,
            })
        })?;
        Ok(projects.collect::<rusqlite::Result<_>>()?)
    }

    pub fn create(&mut self, title: &str, idea: &str) -> Result<Project> {
        let title = title.trim();
        if title.is_empty() || title.chars().count() > 100 {
            return Err(ProjectError::InvalidTitle);
        }
        if idea.len() > MAX_IDEA_BYTES {
            return Err(ProjectError::IdeaTooLong);
        }

        let timestamp = now();
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO projects (title, idea, updated_at) VALUES (?1, ?2, ?3)",
            params![title, idea, timestamp],
        )?;
        let id = transaction.last_insert_rowid();
        transaction.execute(
            "INSERT INTO idea_checkpoints VALUES (?1, 0, ?2, ?3)",
            params![id, idea, timestamp],
        )?;
        let project = Self::read(&transaction, id)?;
        transaction.commit()?;
        Ok(project)
    }

    pub fn get(&self, id: i64) -> Result<Project> {
        Self::read(&self.connection, id)
    }

    pub fn save(&mut self, id: i64, revision: i64, idea: &str) -> Result<Project> {
        if idea.len() > MAX_IDEA_BYTES {
            return Err(ProjectError::IdeaTooLong);
        }

        let timestamp = now();
        let transaction = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let current = Self::read(&transaction, id)?;
        if current.revision != revision {
            return Err(ProjectError::ChangedElsewhere);
        }
        if current.idea == idea {
            return Ok(current);
        }

        let next = revision + 1;
        transaction.execute(
            "UPDATE projects SET idea = ?1, revision = ?2, updated_at = ?3 WHERE id = ?4",
            params![idea, next, timestamp, id],
        )?;
        transaction.execute(
            "INSERT INTO idea_checkpoints VALUES (?1, ?2, ?3, ?4)",
            params![id, next, idea, timestamp],
        )?;
        transaction.execute(
            "DELETE FROM idea_checkpoints WHERE project_id = ?1 AND revision <= ?2",
            params![id, next - 10],
        )?;
        let project = Self::read(&transaction, id)?;
        transaction.commit()?;
        Ok(project)
    }

    fn read(connection: &Connection, id: i64) -> Result<Project> {
        let mut project = connection
            .query_row(
                "SELECT id, title, idea, revision FROM projects WHERE id = ?1",
                [id],
                |row| {
                    Ok(Project {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        idea: row.get(2)?,
                        revision: row.get(3)?,
                        history: Vec::new(),
                    })
                },
            )
            .optional()?
            .ok_or(ProjectError::NotFound)?;

        let mut query = connection.prepare(
            "SELECT revision, saved_at FROM idea_checkpoints
             WHERE project_id = ?1 ORDER BY revision DESC",
        )?;
        let history = query.query_map([id], |row| {
            Ok(Checkpoint {
                revision: row.get(0)?,
                saved_at: row.get(1)?,
            })
        })?;
        project.history = history.collect::<rusqlite::Result<_>>()?;
        Ok(project)
    }
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
