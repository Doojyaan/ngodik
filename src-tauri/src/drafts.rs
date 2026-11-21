mod changes;
mod contract;
pub mod export;
mod extras;
mod qa;
pub mod revisions;
mod storage;
#[cfg(test)]
mod tests;
mod versions;

use std::{collections::HashMap, path::PathBuf, sync::Mutex, time::Duration};

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::{AppHandle, Manager, State};
use tokio_util::sync::CancellationToken;
use url::Url;
use zeroize::Zeroizing;

use crate::{
    Store,
    projects::ProjectStore,
    providers::{self, ProviderError},
};
use storage::{Run, RunView, Target};

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DraftError {
    NotFound,
    StorageUnavailable,
    Busy,
    NoModel,
    EmptyIdea,
    Changed,
    InvalidDraft,
    Canceled,
    ContextTooLarge,
    ContainsKey,
    InvalidRevision,
    PendingReview,
    RecoveryUnavailable,
    #[serde(untagged)]
    Provider(ProviderError),
}

impl From<rusqlite::Error> for DraftError {
    fn from(_: rusqlite::Error) -> Self {
        Self::StorageUnavailable
    }
}
impl From<std::io::Error> for DraftError {
    fn from(_: std::io::Error) -> Self {
        Self::StorageUnavailable
    }
}
impl From<ProviderError> for DraftError {
    fn from(error: ProviderError) -> Self {
        Self::Provider(error)
    }
}

impl DraftError {
    fn code(&self) -> String {
        serde_json::to_value(self)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_else(|| "storage_unavailable".into())
    }
}

type Result<T> = std::result::Result<T, DraftError>;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Product = 0,
    Analyst = 1,
    Ux = 2,
    Architect = 3,
}

impl Stage {
    const ALL: [Self; 4] = [Self::Product, Self::Analyst, Self::Ux, Self::Architect];
}

pub struct Drafts {
    root: PathBuf,
    active: Mutex<HashMap<i64, CancellationToken>>,
}

impl Drafts {
    pub fn new(root: PathBuf, store: &mut ProjectStore) -> Result<Self> {
        storage::interrupt(store, &root)?;
        versions::initialize(store, &root)?;
        Ok(Self {
            root,
            active: Mutex::new(HashMap::new()),
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    target: Option<Target>,
    current: Option<RunView>,
    published: Option<versions::Published>,
    proposal: Option<Proposal>,
    versions: Vec<versions::Version>,
    recovery_warning: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Proposal {
    run_id: i64,
    changes: Vec<changes::Change>,
}

fn lock<'a>(store: &'a State<'_, Store>) -> Result<std::sync::MutexGuard<'a, ProjectStore>> {
    store.lock().map_err(|_| DraftError::StorageUnavailable)
}

fn target(store: &ProjectStore, project_id: i64) -> Result<Option<Target>> {
    Ok(providers::read_selection(store, project_id)?
        .map(|selection| {
            providers::get_profile(store, &selection.profile_id).map(|profile| Target {
                profile_id: selection.profile_id,
                base_url: profile.base_url,
                model_id: selection.model_id,
            })
        })
        .transpose()?)
}

#[tauri::command]
pub fn draft_status(
    store: State<'_, Store>,
    drafts: State<'_, Drafts>,
    project_id: i64,
) -> Result<Snapshot> {
    let store = lock(&store)?;
    let latest = storage::latest(&store, project_id, false)?;
    let pending = latest.as_ref().is_some_and(storage::is_proposal);
    let current = latest
        .filter(|run| run.review_decision.as_deref() != Some("dismissed"))
        .map(|run| storage::show(&store, &drafts.root, run))
        .transpose()?;
    let versions = versions::list(&store, project_id)?;
    let mut recovery_warning = false;
    let published = versions.first().and_then(|version| {
        match versions::read(&store, &drafts.root, project_id, version.id) {
            Ok(backup) => Some(versions::Published {
                version: backup.version,
                draft: backup.draft,
            }),
            Err(_) => {
                recovery_warning = true;
                None
            }
        }
    });
    let proposal = current.as_ref().filter(|_| pending).map(|draft| Proposal {
        run_id: draft.id,
        changes: published
            .as_ref()
            .map(|active| changes::compare(&active.draft.documents(), &draft.documents()))
            .unwrap_or_default(),
    });
    Ok(Snapshot {
        target: target(&store, project_id)?,
        current,
        published,
        proposal,
        versions,
        recovery_warning,
    })
}

#[tauri::command]
pub fn start_draft(
    app: AppHandle,
    store: State<'_, Store>,
    drafts: State<'_, Drafts>,
    project_id: i64,
    revision: i64,
    profile_id: String,
    model_id: String,
) -> Result<()> {
    let mut store = lock(&store)?;
    let project = store.get(project_id).map_err(|error| match error {
        crate::projects::ProjectError::NotFound => DraftError::NotFound,
        _ => DraftError::StorageUnavailable,
    })?;
    if project.idea.trim().is_empty() {
        return Err(DraftError::EmptyIdea);
    }
    let target = target(&store, project_id)?.ok_or(DraftError::NoModel)?;
    if project.revision != revision
        || target.profile_id != profile_id
        || target.model_id != model_id
    {
        return Err(DraftError::Changed);
    }
    let (base, key) = key_for(&app, &target)?;
    if contains_key(&project.idea, &key) || contains_key(&target.model_id, &key) {
        return Err(DraftError::ContainsKey);
    }
    storage::require_no_proposal(&store, project_id)?;
    let head = versions::head(&store.connection, project_id)?;
    let id = storage::create(
        &mut store,
        project_id,
        revision,
        &project.idea,
        &target,
        head,
        None,
    )?;
    spawn(app, &drafts, storage::run(&store, id)?, base, key)
}

#[tauri::command]
pub fn resume_draft(
    app: AppHandle,
    store: State<'_, Store>,
    drafts: State<'_, Drafts>,
    project_id: i64,
    run_id: i64,
) -> Result<()> {
    let mut store = lock(&store)?;
    let run = storage::run(&store, run_id)?;
    if run.project_id != project_id
        || storage::latest(&store, project_id, false)?.map(|run| run.id) != Some(run_id)
    {
        return Err(DraftError::NotFound);
    }
    let profile = providers::get_profile(&store, &run.target.profile_id)?;
    if profile.base_url != run.target.base_url {
        return Err(DraftError::Changed);
    }
    let (base, key) = key_for(&app, &run.target)?;
    if contains_key(&run.idea, &key)
        || contains_key(&run.target.model_id, &key)
        || run
            .revision_note
            .as_deref()
            .is_some_and(|note| contains_key(note, &key))
    {
        return Err(DraftError::ContainsKey);
    }
    storage::resume(&mut store, run_id, project_id)?;
    spawn(app, &drafts, run, base, key)
}

#[tauri::command]
pub fn cancel_draft(
    store: State<'_, Store>,
    drafts: State<'_, Drafts>,
    project_id: i64,
    run_id: i64,
) -> Result<()> {
    let store = lock(&store)?;
    let run = storage::run(&store, run_id)?;
    if run.project_id != project_id {
        return Err(DraftError::NotFound);
    }
    let active = drafts
        .active
        .lock()
        .map_err(|_| DraftError::StorageUnavailable)?;
    if let Some(token) = active.get(&run_id) {
        token.cancel();
    }
    Ok(())
}

fn key_for(app: &AppHandle, target: &Target) -> Result<(Url, Zeroizing<Vec<u8>>)> {
    let base = providers::endpoint::base_url(&target.base_url)?;
    let key = providers::credentials::read(&providers::key_target(app, &target.profile_id))?
        .ok_or(ProviderError::MissingKey)?;
    Ok((base, key))
}

fn spawn(
    app: AppHandle,
    drafts: &Drafts,
    run: Run,
    base: Url,
    key: Zeroizing<Vec<u8>>,
) -> Result<()> {
    let token = CancellationToken::new();
    drafts
        .active
        .lock()
        .map_err(|_| DraftError::StorageUnavailable)?
        .insert(run.id, token.clone());
    tauri::async_runtime::spawn(async move {
        let store = app.state::<Store>();
        let drafts = app.state::<Drafts>();
        let client = app.state::<Client>();
        let worker = Worker {
            store: store.inner(),
            root: &drafts.root,
            client: &client,
            run: &run,
            base: &base,
            key: &key,
            cancel: &token,
        };
        let result = worker.execute().await;
        if let Ok(mut store) = store.lock() {
            let (status, error) = match result {
                _ if token.is_cancelled() => ("canceled", None),
                Ok(()) => ("completed", None),
                Err(error) => ("failed", Some(error.code())),
            };
            if storage::finish(&mut store, run.id, status, error.as_deref()).is_ok()
                && status == "completed"
            {
                let _ = versions::first(&mut store, &drafts.root, run.id);
            }
            if let Ok(mut active) = drafts.active.lock() {
                active.remove(&run.id);
            }
        }
    });
    Ok(())
}

struct Worker<'a> {
    store: &'a Store,
    root: &'a std::path::Path,
    client: &'a Client,
    run: &'a Run,
    base: &'a Url,
    key: &'a [u8],
    cancel: &'a CancellationToken,
}

impl Worker<'_> {
    fn store(&self) -> Result<std::sync::MutexGuard<'_, ProjectStore>> {
        self.store
            .lock()
            .map_err(|_| DraftError::StorageUnavailable)
    }

    async fn execute(&self) -> Result<()> {
        let scope = self.run.revision_scope.as_deref();
        let previous = if let (Some(_), Some(base)) = (scope, self.run.base_version) {
            let store = self.store()?;
            Some(
                versions::read(&store, self.root, self.run.project_id, base)?
                    .draft
                    .documents(),
            )
        } else {
            None
        };
        let reuse = |stage: Stage| {
            previous.as_ref().and_then(|docs| {
                let unchanged = match scope {
                    Some("review") => true,
                    Some("analyst") => matches!(stage, Stage::Product | Stage::Ux),
                    Some("ux") => matches!(stage, Stage::Product | Stage::Analyst),
                    Some("architect") => stage != Stage::Architect,
                    _ => false,
                };
                unchanged.then(|| &docs[stage as usize])
            })
        };
        let mut product_context = json!({"idea":self.run.idea});
        if matches!(scope, Some("all" | "product")) {
            if let Some(docs) = &previous {
                product_context["revision"] =
                    json!({"instruction":self.run.revision_note,"previousSpecification":docs[0]});
            }
        }
        let product = self
            .stage(
                Stage::Product,
                &product_context,
                None,
                reuse(Stage::Product),
            )
            .await?;
        let spec = contract::canonical(product.clone())?;
        let context_for = |stage: Stage, mut context: Value| {
            let target = match stage {
                Stage::Analyst => "analyst",
                Stage::Ux => "ux",
                Stage::Architect => "architect",
                Stage::Product => "product",
            };
            if scope == Some(target) {
                if let Some(docs) = &previous {
                    context["revision"] = json!({"instruction":self.run.revision_note,"previousDocument":docs[stage as usize]});
                }
            }
            context
        };
        let analyst_context = context_for(Stage::Analyst, json!({"projectSpecification":product}));
        let ux_context = context_for(Stage::Ux, json!({"projectSpecification":product}));
        let (analyst, ux) = tokio::join!(
            self.stage(
                Stage::Analyst,
                &analyst_context,
                Some(&spec),
                reuse(Stage::Analyst)
            ),
            self.stage(Stage::Ux, &ux_context, Some(&spec), reuse(Stage::Ux)),
        );
        let (analyst, ux) = (analyst?, ux?);
        let architecture = context_for(
            Stage::Architect,
            json!({
                "projectSpecification":product, "systemAnalysis":analyst, "uiUx":ux
            }),
        );
        let architect = self
            .stage(
                Stage::Architect,
                &architecture,
                Some(&spec),
                reuse(Stage::Architect),
            )
            .await?;
        self.review([product, analyst, ux, architect]).await
    }

    async fn review(&self, docs: [Value; 4]) -> Result<()> {
        self.check_canceled()?;
        let mut report = qa::Report::local(qa::deterministic(&docs));
        let payload = qa::context(&docs);
        let eligible = report.deterministic.is_empty()
            && !value_contains_key(&payload, self.key)
            && payload.to_string().len() <= 132 * 1024;
        let send = {
            let store = self.store()?;
            if qa::saved(&store.connection, self.run.id)?.is_some() {
                return Ok(());
            }
            eligible && qa::begin(&store.connection, self.run.id)?
        };
        report.semantic_status = qa::SemanticStatus::Unavailable;
        if send {
            let payload = payload.to_string();
            let (response, _) = tokio::select! {
                biased;
                _ = self.cancel.cancelled() => return Err(DraftError::Canceled),
                response = providers::endpoint::generate(
                    self.client, self.base, self.key, &self.run.target.model_id, qa::INSTRUCTION, &payload
                ) => response,
            };
            if let Ok(response) = response {
                if !value_contains_key(&response, self.key) {
                    if let Ok(findings) = qa::semantic(response, &docs) {
                        report.semantic_status = qa::SemanticStatus::Complete;
                        report.semantic = findings;
                    }
                }
            }
        }
        let store = self.store()?;
        self.check_canceled()?;
        qa::save(&store.connection, self.run.id, &report)
    }

    async fn stage(
        &self,
        stage: Stage,
        context: &Value,
        spec: Option<&contract::Canonical>,
        reuse: Option<&Value>,
    ) -> Result<Value> {
        self.check_canceled()?;
        if let Some(saved) = storage::cached(&*self.store()?, self.root, self.run, stage)? {
            return Ok(saved);
        }
        let result = match reuse {
            Some(document) => {
                if value_contains_key(document, self.key) {
                    return Err(DraftError::ContainsKey);
                }
                contract::normalize(stage, document.clone(), spec)
            }
            None => self.generate(stage, context, spec).await,
        };
        match &result {
            Err(_) if self.cancel.is_cancelled() => return Err(DraftError::Canceled),
            Err(error) => storage::fail_stage(&*self.store()?, self.run.id, stage, &error.code())?,
            Ok(output) => {
                let store = self.store()?;
                self.check_canceled()?;
                storage::checkpoint(&store, self.root, self.run, stage, output)?;
            }
        }
        result
    }

    async fn generate(
        &self,
        stage: Stage,
        context: &Value,
        spec: Option<&contract::Canonical>,
    ) -> Result<Value> {
        let payload = context.to_string();
        if payload.len() > 64 * 1024 {
            return Err(DraftError::ContextTooLarge);
        }
        if value_contains_key(context, self.key) {
            return Err(DraftError::ContainsKey);
        }
        let mut instruction = contract::instruction(stage);
        if context.get("revision").is_some() {
            instruction.push_str(" Apply only the requested revision within your role. Keep unrelated requirements, IDs, assumptions and document sections stable. Mark guesses rather than introducing unsupported decisions.");
        }
        for attempt in 0..2 {
            self.check_canceled()?;
            storage::begin_attempt(&*self.store()?, self.run.id, stage)?;
            let (response, retry_after) = tokio::select! {
                biased;
                _ = self.cancel.cancelled() => return Err(DraftError::Canceled),
                response = providers::endpoint::generate(
                    self.client, self.base, self.key, &self.run.target.model_id, &instruction, &payload
                ) => response
            };
            match response {
                Ok(output) => {
                    if value_contains_key(&output, self.key) {
                        return Err(DraftError::ContainsKey);
                    }
                    return contract::normalize(stage, output, spec);
                }
                Err(error) if attempt == 0 && transient(&error) => {
                    let delay = retry_after.unwrap_or(Duration::from_millis(600));
                    // A long provider cooldown requires an explicit retry instead of retrying too soon.
                    if delay > Duration::from_secs(30) {
                        return Err(error.into());
                    }
                    tokio::select! {
                        _ = self.cancel.cancelled() => return Err(DraftError::Canceled),
                        _ = tokio::time::sleep(delay) => {}
                    }
                }
                Err(error) => return Err(error.into()),
            }
        }
        unreachable!("the retry loop always returns an outcome")
    }

    fn check_canceled(&self) -> Result<()> {
        if self.cancel.is_cancelled() {
            Err(DraftError::Canceled)
        } else {
            Ok(())
        }
    }
}

fn transient(error: &ProviderError) -> bool {
    matches!(
        error,
        ProviderError::Timeout
            | ProviderError::NetworkUnavailable
            | ProviderError::RateLimited
            | ProviderError::EndpointUnavailable
    )
}

fn contains_key(text: &str, key: &[u8]) -> bool {
    !key.is_empty() && text.as_bytes().windows(key.len()).any(|part| part == key)
}

fn value_contains_key(value: &Value, key: &[u8]) -> bool {
    match value {
        Value::String(text) => contains_key(text, key),
        Value::Array(items) => items.iter().any(|item| value_contains_key(item, key)),
        Value::Object(items) => items
            .iter()
            .any(|(name, item)| contains_key(name, key) || value_contains_key(item, key)),
        _ => false,
    }
}
