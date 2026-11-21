pub(super) mod credentials;
pub(super) mod endpoint;
#[cfg(test)]
mod tests;

use reqwest::Client;
use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use tauri::State;
use url::Url;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{Store, projects::ProjectStore};

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderError {
    InvalidLabel,
    InvalidEndpoint,
    EndpointLocked,
    InvalidModel,
    NotFound,
    StorageUnavailable,
    CredentialUnavailable,
    InvalidKey,
    MissingKey,
    NetworkUnavailable,
    Timeout,
    Unauthorized,
    RateLimited,
    UnavailableRoute,
    EndpointUnavailable,
    RedirectBlocked,
    RequestRejected,
    InvalidResponse,
}

impl From<rusqlite::Error> for ProviderError {
    fn from(_: rusqlite::Error) -> Self {
        Self::StorageUnavailable
    }
}

type Result<T> = std::result::Result<T, ProviderError>;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    id: String,
    label: String,
    pub(super) base_url: String,
    has_key: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Selection {
    pub(super) profile_id: String,
    pub(super) model_id: String,
}

fn lock<'a>(store: &'a State<'_, Store>) -> Result<std::sync::MutexGuard<'a, ProjectStore>> {
    store.lock().map_err(|_| ProviderError::StorageUnavailable)
}

fn profile_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Profile> {
    Ok(Profile {
        id: row.get(0)?,
        label: row.get(1)?,
        base_url: row.get(2)?,
        has_key: false,
    })
}

pub(super) fn get_profile(store: &ProjectStore, id: &str) -> Result<Profile> {
    store
        .connection
        .query_row(
            "SELECT id, label, base_url FROM provider_profiles WHERE id=?1",
            [id],
            profile_row,
        )
        .optional()?
        .ok_or(ProviderError::NotFound)
}

fn write_profile(
    store: &ProjectStore,
    id: Option<&str>,
    label: &str,
    base_url: &str,
) -> Result<Profile> {
    let label = label.trim();
    if label.is_empty() || label.chars().count() > 80 || label.chars().any(char::is_control) {
        return Err(ProviderError::InvalidLabel);
    }
    let base = endpoint::base_url(base_url)?;
    if let Some(id) = id {
        let existing = get_profile(store, id)?;
        if existing.base_url != base.as_str() {
            return Err(ProviderError::EndpointLocked);
        }
        store.connection.execute(
            "UPDATE provider_profiles SET label=?1 WHERE id=?2",
            params![label, id],
        )?;
        return get_profile(store, id);
    }
    let id = Uuid::new_v4().to_string();
    store.connection.execute(
        "INSERT INTO provider_profiles VALUES (?1,?2,?3)",
        params![id, label, base.as_str()],
    )?;
    get_profile(store, &id)
}

fn require_project(store: &ProjectStore, project: i64) -> Result<()> {
    let exists: bool = store.connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM projects WHERE id=?1)",
        [project],
        |row| row.get(0),
    )?;
    if !exists {
        return Err(ProviderError::NotFound);
    }
    Ok(())
}

pub(super) fn read_selection(store: &ProjectStore, project: i64) -> Result<Option<Selection>> {
    require_project(store, project)?;
    Ok(store
        .connection
        .query_row(
            "SELECT profile_id, model_id FROM project_models WHERE project_id=?1",
            [project],
            |row| {
                Ok(Selection {
                    profile_id: row.get(0)?,
                    model_id: row.get(1)?,
                })
            },
        )
        .optional()?)
}

fn save_selection(store: &ProjectStore, project: i64, profile: &str, model: &str) -> Result<()> {
    require_project(store, project)?;
    get_profile(store, profile)?;
    if !valid_model(model) {
        return Err(ProviderError::InvalidModel);
    }
    store.connection.execute(
        "INSERT INTO project_models VALUES (?1,?2,?3)
         ON CONFLICT(project_id) DO UPDATE SET profile_id=excluded.profile_id, model_id=excluded.model_id",
        params![project, profile, model],
    )?;
    Ok(())
}

fn valid_model(model: &str) -> bool {
    !model.is_empty()
        && model.len() <= 256
        && !model
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace())
}

fn add_key_status(app: &tauri::AppHandle, mut profile: Profile) -> Result<Profile> {
    profile.has_key = credentials::read(&key_target(app, &profile.id))?.is_some();
    Ok(profile)
}

pub(super) fn key_target(app: &tauri::AppHandle, id: &str) -> String {
    credentials::target(&app.config().identifier, id)
}

#[tauri::command]
pub fn list_profiles(app: tauri::AppHandle, store: State<'_, Store>) -> Result<Vec<Profile>> {
    let profiles = {
        let store = lock(&store)?;
        let mut query = store.connection.prepare(
            "SELECT id, label, base_url FROM provider_profiles ORDER BY label COLLATE NOCASE, id",
        )?;
        query
            .query_map([], profile_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    profiles
        .into_iter()
        .map(|profile| add_key_status(&app, profile))
        .collect()
}

#[tauri::command]
pub fn save_profile(
    app: tauri::AppHandle,
    store: State<'_, Store>,
    id: Option<String>,
    label: String,
    base_url: String,
) -> Result<Profile> {
    let store = lock(&store)?;
    add_key_status(
        &app,
        write_profile(&store, id.as_deref(), &label, &base_url)?,
    )
}

#[tauri::command]
pub async fn set_profile_key(
    window: tauri::Window,
    app: tauri::AppHandle,
    store: State<'_, Store>,
    id: String,
) -> Result<bool> {
    let base = {
        let store = lock(&store)?;
        get_profile(&store, &id)?.base_url
    };
    let owner = window
        .hwnd()
        .map_err(|_| ProviderError::CredentialUnavailable)?
        .0 as usize;
    let Some(mut key) =
        tauri::async_runtime::spawn_blocking(move || credentials::prompt(owner, &base))
            .await
            .map_err(|_| ProviderError::CredentialUnavailable)??
    else {
        return Ok(false);
    };
    let store = lock(&store)?;
    get_profile(&store, &id)?;
    credentials::write(&key_target(&app, &id), &mut key)?;
    Ok(true)
}

#[tauri::command]
pub fn delete_profile(app: tauri::AppHandle, store: State<'_, Store>, id: String) -> Result<()> {
    let store = lock(&store)?;
    get_profile(&store, &id)?;
    credentials::delete(&key_target(&app, &id))?;
    store
        .connection
        .execute("DELETE FROM provider_profiles WHERE id=?1", [id])?;
    Ok(())
}

fn connection(
    app: &tauri::AppHandle,
    store: &State<'_, Store>,
    id: &str,
) -> Result<(Url, Zeroizing<Vec<u8>>)> {
    let store = lock(store)?;
    let profile = get_profile(&store, id)?;
    let base = endpoint::base_url(&profile.base_url)?;
    let key = credentials::read(&key_target(app, id))?.ok_or(ProviderError::MissingKey)?;
    Ok((base, key))
}

#[tauri::command]
pub async fn sync_models(
    app: tauri::AppHandle,
    store: State<'_, Store>,
    client: State<'_, Client>,
    id: String,
) -> Result<Vec<String>> {
    let (base, key) = connection(&app, &store, &id)?;
    endpoint::models(&client, &base, &key).await
}

#[tauri::command]
pub async fn test_connection(
    app: tauri::AppHandle,
    store: State<'_, Store>,
    client: State<'_, Client>,
    id: String,
    model_id: String,
) -> Result<()> {
    let model = model_id.trim();
    if !valid_model(model) {
        return Err(ProviderError::InvalidModel);
    }
    let (base, key) = connection(&app, &store, &id)?;
    endpoint::check(&client, &base, &key, model).await
}

#[tauri::command]
pub fn project_model(store: State<'_, Store>, project_id: i64) -> Result<Option<Selection>> {
    let store = lock(&store)?;
    read_selection(&store, project_id)
}

#[tauri::command]
pub fn set_project_model(
    store: State<'_, Store>,
    project_id: i64,
    profile_id: String,
    model_id: String,
) -> Result<()> {
    let store = lock(&store)?;
    save_selection(&store, project_id, &profile_id, model_id.trim())
}

pub fn http_client() -> Result<Client> {
    endpoint::client()
}
