mod contract;
mod storage;
#[cfg(test)]
mod tests;

use std::{collections::HashMap, path::PathBuf, sync::Mutex, time::Duration};

use reqwest::Client;
use serde::Serialize;
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

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
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
    pub fn new(root: PathBuf, store: &ProjectStore) -> Result<Self> {
        storage::interrupt(store, &root)?;
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
    previous: Option<RunView>,
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
    let current = storage::latest(&store, project_id, false)?
        .map(|run| storage::show(&store, &drafts.root, run))
        .transpose()?;
    let previous = if current
        .as_ref()
        .is_some_and(|run| run.status != "completed")
    {
        storage::latest(&store, project_id, true)?
            .map(|run| storage::show(&store, &drafts.root, run))
            .transpose()?
    } else {
        None
    };
    Ok(Snapshot {
        target: target(&store, project_id)?,
        current,
        previous,
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
    let id = storage::create(&mut store, project_id, revision, &project.idea, &target)?;
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
    if contains_key(&run.idea, &key) || contains_key(&run.target.model_id, &key) {
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
            let _ = storage::finish(&mut store, run.id, status, error.as_deref());
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
        let product = self
            .stage(Stage::Product, &json!({"idea":self.run.idea}), None)
            .await?;
        let spec = contract::canonical(product.clone())?;
        let context = json!({"projectSpecification":product});
        let (analyst, ux) = tokio::join!(
            self.stage(Stage::Analyst, &context, Some(&spec)),
            self.stage(Stage::Ux, &context, Some(&spec)),
        );
        let architecture =
            json!({"projectSpecification": product, "systemAnalysis": analyst?, "uiUx": ux?});
        self.stage(Stage::Architect, &architecture, Some(&spec))
            .await?;
        Ok(())
    }

    async fn stage(
        &self,
        stage: Stage,
        context: &Value,
        spec: Option<&contract::Canonical>,
    ) -> Result<Value> {
        self.check_canceled()?;
        if let Some(saved) = storage::cached(&*self.store()?, self.root, self.run, stage)? {
            return Ok(saved);
        }
        let result = self.generate(stage, context, spec).await;
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
        let instruction = contract::instruction(stage);
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
