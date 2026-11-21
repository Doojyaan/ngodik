use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::{DraftError, Result, Stage, qa};
use crate::projects::ProjectStore;

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Target {
    pub profile_id: String,
    pub base_url: String,
    pub model_id: String,
}

pub struct Run {
    pub id: i64,
    pub project_id: i64,
    pub idea_revision: i64,
    pub idea: String,
    pub target: Target,
    pub status: String,
    pub error: Option<String>,
    pub base_version: Option<i64>,
    pub revision_scope: Option<String>,
    pub revision_note: Option<String>,
    pub review_decision: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StageView {
    pub stage: Stage,
    pub status: String,
    pub attempts: u32,
    pub error: Option<String>,
    pub output: Option<Value>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunView {
    pub id: i64,
    pub idea_revision: i64,
    pub target: Target,
    pub status: String,
    pub error: Option<String>,
    pub stages: Vec<StageView>,
    pub qa: Option<qa::Report>,
}

impl RunView {
    pub fn documents(&self) -> [Value; 4] {
        std::array::from_fn(|index| {
            self.stages
                .get(index)
                .and_then(|stage| stage.output.clone())
                .unwrap_or(Value::Null)
        })
    }
}

pub fn interrupt(store: &ProjectStore, root: &Path) -> Result<()> {
    let connection = &store.connection;
    let ids = {
        let mut query = connection.prepare("SELECT id FROM draft_runs WHERE status='running'")?;
        query
            .query_map([], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<i64>>>()?
    };
    for id in ids {
        let run = run(store, id)?;
        let mut query = connection.prepare(
            "SELECT artifact FROM draft_stages WHERE run_id=?1 AND artifact IS NOT NULL",
        )?;
        let referenced = query
            .query_map([id], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<String>>>()?;
        let Ok(entries) = fs::read_dir(directory(root, &run)) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            if Uuid::parse_str(stem).is_ok()
                && entry.file_type().is_ok_and(|kind| kind.is_file())
                && (path.extension().is_some_and(|ext| ext == "tmp")
                    || (path.extension().is_some_and(|ext| ext == "json")
                        && !referenced.iter().any(|name| name == stem)))
            {
                let _ = fs::remove_file(path);
            }
        }
    }
    connection.execute_batch(
        "BEGIN;
         UPDATE draft_runs SET status='interrupted' WHERE status='running';
         UPDATE draft_stages SET status='interrupted' WHERE status='running';
         COMMIT;",
    )?;
    Ok(())
}

pub fn create(
    store: &mut ProjectStore,
    project_id: i64,
    revision: i64,
    idea: &str,
    target: &Target,
    base_version: Option<i64>,
    revision_info: Option<(&str, &str)>,
) -> Result<i64> {
    let transaction = store.connection.transaction()?;
    if is_running(&transaction, project_id)? {
        return Err(DraftError::Busy);
    }
    let settings = serde_json::to_string(target).map_err(|_| DraftError::StorageUnavailable)?;
    transaction.execute(
        "INSERT INTO draft_runs (project_id,idea_revision,idea,target,status,base_version,revision_scope,revision_note) VALUES (?1,?2,?3,?4,'running',?5,?6,?7)",
        params![project_id,revision,idea,settings,base_version,revision_info.map(|it| it.0),revision_info.map(|it| it.1)],
    )?;
    let id = transaction.last_insert_rowid();
    for stage in Stage::ALL {
        transaction.execute(
            "INSERT INTO draft_stages (run_id,stage,status) VALUES (?1,?2,'waiting')",
            params![id, stage as i64],
        )?;
    }
    transaction.commit()?;
    Ok(id)
}

pub fn resume(store: &mut ProjectStore, id: i64, project_id: i64) -> Result<()> {
    let transaction = store.connection.transaction()?;
    if is_running(&transaction, project_id)? {
        return Err(DraftError::Busy);
    }
    if transaction.execute(
        "UPDATE draft_runs SET status='running', error=NULL
         WHERE id=?1 AND project_id=?2 AND status IN ('failed','interrupted','canceled')",
        params![id, project_id],
    )? != 1
    {
        return Err(DraftError::NotFound);
    }
    transaction.execute(
        "UPDATE draft_stages SET status='waiting', error=NULL WHERE run_id=?1 AND status!='completed'", [id],
    )?;
    transaction.commit()?;
    Ok(())
}

pub fn is_running(connection: &Connection, project_id: i64) -> Result<bool> {
    Ok(connection.query_row(
        "SELECT EXISTS (SELECT 1 FROM draft_runs WHERE project_id=?1 AND status='running')",
        [project_id],
        |row| row.get(0),
    )?)
}

pub fn latest(store: &ProjectStore, project_id: i64, completed: bool) -> Result<Option<Run>> {
    let id = store.connection.query_row(
        "SELECT id FROM draft_runs WHERE project_id=?1 AND (?2=0 OR status='completed') ORDER BY id DESC LIMIT 1",
        params![project_id, completed], |row| row.get(0),
    ).optional()?;
    id.map(|id| run(store, id)).transpose()
}

pub fn run(store: &ProjectStore, id: i64) -> Result<Run> {
    let (project_id, idea_revision, idea, settings, status, error, base_version, revision_scope, revision_note, review_decision) = store
        .connection
        .query_row(
            "SELECT project_id,idea_revision,idea,target,status,error,base_version,revision_scope,revision_note,review_decision FROM draft_runs WHERE id=?1",
            [id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get::<_, String>(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                ))
            },
        )
        .optional()?
        .ok_or(DraftError::NotFound)?;
    let target = serde_json::from_str(&settings).map_err(|_| DraftError::StorageUnavailable)?;
    Ok(Run {
        id,
        project_id,
        idea_revision,
        idea,
        target,
        status,
        error,
        base_version,
        revision_scope,
        revision_note,
        review_decision,
    })
}

pub fn show(store: &ProjectStore, root: &Path, run: Run) -> Result<RunView> {
    let mut stages = Vec::new();
    for stage in Stage::ALL {
        let (status, attempts, error, artifact) = store.connection.query_row(
            "SELECT status,attempts,error,artifact FROM draft_stages WHERE run_id=?1 AND stage=?2",
            params![run.id, stage as i64],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            },
        )?;
        let (status, error, output) = match artifact
            .map(|name| read_artifact(root, &run, &name))
            .transpose()
        {
            Ok(output) => (status, error, output),
            Err(_) => ("failed".into(), Some("checkpoint_unreadable".into()), None),
        };
        stages.push(StageView {
            stage,
            status,
            attempts,
            error,
            output,
        });
    }
    Ok(RunView {
        id: run.id,
        idea_revision: run.idea_revision,
        target: run.target,
        status: run.status,
        error: run.error,
        stages,
        qa: qa::saved(&store.connection, run.id)?,
    })
}

pub fn cached(store: &ProjectStore, root: &Path, run: &Run, stage: Stage) -> Result<Option<Value>> {
    let id: Option<String> = store.connection.query_row(
        "SELECT artifact FROM draft_stages WHERE run_id=?1 AND stage=?2",
        params![run.id, stage as i64],
        |row| row.get(0),
    )?;
    id.map(|name| read_artifact(root, run, &name)).transpose()
}

fn directory(root: &Path, run: &Run) -> PathBuf {
    root.join("projects")
        .join(run.project_id.to_string())
        .join("drafts")
        .join(run.id.to_string())
}

fn read_artifact(root: &Path, run: &Run, name: &str) -> Result<Value> {
    let name = Uuid::parse_str(name).map_err(|_| DraftError::StorageUnavailable)?;
    let bytes = fs::read(directory(root, run).join(format!("{name}.json")))?;
    serde_json::from_slice(&bytes).map_err(|_| DraftError::StorageUnavailable)
}

pub fn begin_attempt(store: &ProjectStore, run: i64, stage: Stage) -> Result<()> {
    if store.connection.execute(
        "UPDATE draft_stages SET status='running', attempts=attempts+1, error=NULL
         WHERE run_id=?1 AND stage=?2 AND EXISTS(SELECT 1 FROM draft_runs WHERE id=?1 AND status='running')",
        params![run,stage as i64],
    )? != 1 { return Err(DraftError::Canceled); }
    Ok(())
}

fn write_json(path: &Path, id: Uuid, output: &impl Serialize) -> Result<PathBuf> {
    let bytes = serde_json::to_vec(output).map_err(|_| DraftError::StorageUnavailable)?;
    write_blob(path, id, &bytes)
}

pub fn write_blob(path: &Path, id: Uuid, bytes: &[u8]) -> Result<PathBuf> {
    fs::create_dir_all(path)?;
    let temporary = path.join(format!("{id}.tmp"));
    let destination = path.join(format!("{id}.json"));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.flush()?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, &destination)?;
        Ok(destination)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

pub fn checkpoint(
    store: &ProjectStore,
    root: &Path,
    run: &Run,
    stage: Stage,
    output: &Value,
) -> Result<()> {
    let id = Uuid::new_v4();
    let path = write_json(&directory(root, run), id, output)?;
    let result = store.connection.execute(
        "UPDATE draft_stages SET status='completed', artifact=?3 WHERE run_id=?1 AND stage=?2
         AND EXISTS(SELECT 1 FROM draft_runs WHERE id=?1 AND status='running')",
        params![run.id, stage as i64, id.to_string()],
    );
    if matches!(result, Ok(1)) {
        return Ok(());
    }
    let _ = fs::remove_file(path);
    match result {
        Ok(_) => Err(DraftError::Canceled),
        Err(error) => Err(error.into()),
    }
}

pub fn fail_stage(store: &ProjectStore, run: i64, stage: Stage, error: &str) -> Result<()> {
    store.connection.execute(
        "UPDATE draft_stages SET status='failed', error=?3 WHERE run_id=?1 AND stage=?2 AND status IN ('waiting','running') AND EXISTS(SELECT 1 FROM draft_runs WHERE id=?1 AND status='running')",
        params![run,stage as i64,error],
    )?;
    Ok(())
}

pub fn finish(store: &mut ProjectStore, run: i64, status: &str, error: Option<&str>) -> Result<()> {
    let transaction = store.connection.transaction()?;
    if transaction.execute(
        "UPDATE draft_runs SET status=?2,error=?3 WHERE id=?1 AND status='running'",
        params![run, status, error],
    )? == 1
    {
        transaction.execute(
            "UPDATE draft_stages SET status=?2,error=?3 WHERE run_id=?1 AND status='running'",
            params![run, status, error],
        )?;
    }
    transaction.commit()?;
    Ok(())
}

pub fn is_proposal(run: &Run) -> bool {
    run.status == "completed" && run.review_decision.is_none()
}

pub fn require_no_proposal(store: &ProjectStore, project_id: i64) -> Result<()> {
    if latest(store, project_id, false)?
        .as_ref()
        .is_some_and(is_proposal)
    {
        return Err(DraftError::PendingReview);
    }
    Ok(())
}
