use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{
    DraftError, Result, qa,
    storage::{self, RunView},
};
use crate::projects::ProjectStore;

const RECOVERY_LIMIT: i64 = 10;

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    pub id: i64,
    pub number: i64,
    pub parent_id: Option<i64>,
    pub restored_from: Option<i64>,
    pub created_at: i64,
    pub summary: String,
    pub recoverable: bool,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    schema_version: u32,
    project_id: i64,
    pub version: Version,
    pub idea: String,
    pub draft: RunView,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Published {
    pub version: Version,
    pub draft: RunView,
}

fn directory(root: &Path, project_id: i64) -> PathBuf {
    root.join("projects")
        .join(project_id.to_string())
        .join("recovery")
}

fn metadata(row: &rusqlite::Row<'_>) -> rusqlite::Result<Version> {
    Ok(Version {
        id: row.get(0)?,
        number: row.get(1)?,
        parent_id: row.get(2)?,
        restored_from: row.get(3)?,
        created_at: row.get(4)?,
        summary: row.get(5)?,
        recoverable: row.get(6)?,
    })
}

pub fn list(store: &ProjectStore, project_id: i64) -> Result<Vec<Version>> {
    let mut query = store.connection.prepare(
        "SELECT id,number,parent_id,restored_from,created_at,summary,recovery_file IS NOT NULL
         FROM blueprint_versions WHERE project_id=?1 ORDER BY number DESC",
    )?;
    Ok(query
        .query_map([project_id], metadata)?
        .collect::<rusqlite::Result<_>>()?)
}

pub fn head(connection: &Connection, project_id: i64) -> Result<Option<i64>> {
    Ok(connection
        .query_row(
            "SELECT id FROM blueprint_versions WHERE project_id=?1 ORDER BY number DESC LIMIT 1",
            [project_id],
            |row| row.get(0),
        )
        .optional()?)
}

pub fn read(store: &ProjectStore, root: &Path, project_id: i64, id: i64) -> Result<Backup> {
    let stored: Option<(Option<String>, Option<Vec<u8>>)> = store.connection.query_row(
        "SELECT recovery_file,recovery_hash FROM blueprint_versions WHERE id=?1 AND project_id=?2",
        params![id, project_id], |row| Ok((row.get(0)?, row.get(1)?)),
    ).optional()?;
    let (Some(file), Some(hash)) = stored.ok_or(DraftError::RecoveryUnavailable)? else {
        return Err(DraftError::RecoveryUnavailable);
    };
    let file = Uuid::parse_str(&file).map_err(|_| DraftError::RecoveryUnavailable)?;
    let bytes = fs::read(directory(root, project_id).join(format!("{file}.json")))
        .map_err(|_| DraftError::RecoveryUnavailable)?;
    if Sha256::digest(&bytes).as_slice() != hash {
        return Err(DraftError::RecoveryUnavailable);
    }
    let backup: Backup =
        serde_json::from_slice(&bytes).map_err(|_| DraftError::RecoveryUnavailable)?;
    if backup.schema_version != 1 || backup.project_id != project_id || backup.version.id != id {
        return Err(DraftError::RecoveryUnavailable);
    }
    Ok(backup)
}

pub fn publish(
    store: &mut ProjectStore,
    root: &Path,
    project_id: i64,
    expected_head: Option<i64>,
    draft: RunView,
    idea: String,
    summary: &str,
    restored_from: Option<i64>,
) -> Result<()> {
    let transaction = store.connection.transaction()?;
    if head(&transaction, project_id)? != expected_head {
        return Err(DraftError::Changed);
    }
    let number: i64 = transaction.query_row(
        "SELECT COALESCE(MAX(number),0)+1 FROM blueprint_versions WHERE project_id=?1",
        [project_id],
        |row| row.get(0),
    )?;
    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let file = Uuid::new_v4();
    transaction.execute(
        "INSERT INTO blueprint_versions(project_id,run_id,number,parent_id,restored_from,created_at,summary,recovery_file)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![project_id,draft.id,number,expected_head,restored_from,created_at,summary,file.to_string()],
    )?;
    let version = Version {
        id: transaction.last_insert_rowid(),
        number,
        parent_id: expected_head,
        restored_from,
        created_at,
        summary: summary.into(),
        recoverable: true,
    };
    let run_id = draft.id;
    let backup = Backup {
        schema_version: 1,
        project_id,
        version,
        idea,
        draft,
    };

    if restored_from.is_none() {
        transaction.execute(
            "UPDATE draft_runs SET review_decision='applied' WHERE id=?1",
            [run_id],
        )?;
    }
    transaction.execute(
        "UPDATE blueprint_versions SET recovery_file=NULL,recovery_hash=NULL
         WHERE project_id=?1 AND recovery_file IS NOT NULL AND number <= ?2",
        params![project_id, number - RECOVERY_LIMIT],
    )?;
    let bytes = serde_json::to_vec(&backup).map_err(|_| DraftError::StorageUnavailable)?;
    transaction.execute(
        "UPDATE blueprint_versions SET recovery_hash=?2 WHERE id=?1",
        params![backup.version.id, Sha256::digest(&bytes).as_slice()],
    )?;
    let path = storage::write_blob(&directory(root, project_id), file, &bytes)?;
    if let Err(error) = transaction.commit() {
        let _ = fs::remove_file(path);
        return Err(error.into());
    }
    let _ = clean(store, root, project_id);
    Ok(())
}

fn clean(store: &ProjectStore, root: &Path, project_id: i64) -> Result<()> {
    let mut query = store.connection.prepare("SELECT recovery_file FROM blueprint_versions WHERE project_id=?1 AND recovery_file IS NOT NULL")?;
    let referenced = query
        .query_map([project_id], |row| row.get(0))?
        .collect::<rusqlite::Result<HashSet<String>>>()?;
    let Ok(files) = fs::read_dir(directory(root, project_id)) else {
        return Ok(());
    };
    for file in files.flatten() {
        let path = file.path();
        let Some(stem) = path.file_stem().and_then(|name| name.to_str()) else {
            continue;
        };
        if Uuid::parse_str(stem).is_ok()
            && file.file_type().is_ok_and(|kind| kind.is_file())
            && (path.extension().is_some_and(|ext| ext == "tmp")
                || (path.extension().is_some_and(|ext| ext == "json")
                    && !referenced.contains(stem)))
        {
            let _ = fs::remove_file(path);
        }
    }
    Ok(())
}

pub fn initialize(store: &mut ProjectStore, root: &Path) -> Result<()> {
    let ids = {
        let mut query = store.connection.prepare("SELECT id FROM projects")?;
        query
            .query_map([], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<i64>>>()?
    };
    for project_id in ids {
        if head(&store.connection, project_id)?.is_none() {
            let previous = storage::latest(store, project_id, true)?;
            if let Some(run) = previous {
                // Imported Stage 3 drafts can be reviewed on demand without regenerating documents.
                let _ = first(store, root, run.id);
            }
        }
        let _ = clean(store, root, project_id);
    }
    Ok(())
}

pub fn first(store: &mut ProjectStore, root: &Path, run_id: i64) -> Result<()> {
    let run = storage::run(store, run_id)?;
    if run.status != "completed"
        || run.review_decision.as_deref() == Some("dismissed")
        || head(&store.connection, run.project_id)?.is_some()
    {
        return Ok(());
    }
    let project_id = run.project_id;
    let idea = run.idea.clone();
    let mut draft = storage::show(store, root, run)?;
    if draft.qa.is_none() {
        draft.qa = Some(qa::Report::local(qa::deterministic(&draft.documents())));
    }
    publish(
        store,
        root,
        project_id,
        None,
        draft,
        idea,
        "Draf pertama",
        None,
    )
}
