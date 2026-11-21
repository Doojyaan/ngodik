mod package;
mod render;
#[cfg(test)]
mod tests;

use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, State, Window};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::DialogExt;

use crate::{Store, providers};

use super::{Drafts, contains_key, value_contains_key, versions};

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportError {
    MissingVersion,
    VersionChanged,
    InvalidBlueprint,
    InvalidPackage,
    ContainsCredential,
    CredentialCheck,
    InvalidLocation,
    WriteFailed,
    ClipboardUnavailable,
}

type Result<T> = std::result::Result<T, ExportError>;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedExport {
    path: String,
    document_count: usize,
    issue_count: usize,
}

fn prepare(
    app: &AppHandle,
    store: &Store,
    drafts: &Drafts,
    project_id: i64,
    expected: Option<i64>,
) -> Result<(i64, package::Package)> {
    let (version_id, backup, profiles) = {
        let store = store.lock().map_err(|_| ExportError::InvalidBlueprint)?;
        let version_id = versions::head(&store.connection, project_id)
            .map_err(|_| ExportError::InvalidBlueprint)?
            .ok_or(ExportError::MissingVersion)?;
        if expected.is_some_and(|id| id != version_id) {
            return Err(ExportError::VersionChanged);
        }
        let backup = versions::read(&store, &drafts.root, project_id, version_id)
            .map_err(|_| ExportError::InvalidBlueprint)?;
        let mut query = store
            .connection
            .prepare("SELECT id FROM provider_profiles")
            .map_err(|_| ExportError::CredentialCheck)?;
        let profiles = query
            .query_map([], |row| row.get(0))
            .map_err(|_| ExportError::CredentialCheck)?
            .collect::<rusqlite::Result<Vec<String>>>()
            .map_err(|_| ExportError::CredentialCheck)?;
        (version_id, backup, profiles)
    };
    let pack = package::build(&backup)?;
    let docs = backup.draft.documents();
    let review =
        serde_json::to_value(&backup.draft.qa).map_err(|_| ExportError::InvalidBlueprint)?;
    for id in profiles {
        let target = providers::key_target(app, &id);
        let Some(key) =
            providers::credentials::read(&target).map_err(|_| ExportError::CredentialCheck)?
        else {
            continue;
        };
        check_secret(&pack, &docs, &review, &key)?;
    }
    Ok((version_id, pack))
}

fn check_secret(
    pack: &package::Package,
    docs: &[serde_json::Value; 4],
    review: &serde_json::Value,
    key: &[u8],
) -> Result<()> {
    if value_contains_key(review, key)
        || docs.iter().any(|doc| value_contains_key(doc, key))
        || pack
            .files
            .iter()
            .any(|(name, body)| contains_key(name, key) || contains_key(body, key))
        || contains_key(&pack.name, key)
    {
        return Err(ExportError::ContainsCredential);
    }
    Ok(())
}

#[tauri::command]
pub fn export_preview(
    app: AppHandle,
    store: State<'_, Store>,
    drafts: State<'_, Drafts>,
    project_id: i64,
) -> Result<package::Preview> {
    let (id, pack) = prepare(&app, store.inner(), &drafts, project_id, None)?;
    Ok(pack.preview(id))
}

#[tauri::command]
pub async fn save_blueprint(
    window: Window,
    app: AppHandle,
    store: State<'_, Store>,
    drafts: State<'_, Drafts>,
    project_id: i64,
    version_id: i64,
) -> Result<Option<SavedExport>> {
    let (_, pack) = prepare(&app, store.inner(), &drafts, project_id, Some(version_id))?;
    let (send, receive) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_parent(&window)
        .set_title("Simpan blueprint")
        .add_filter("Blueprint ZIP", &["zip"])
        .set_file_name(&pack.name)
        .save_file(move |chosen| {
            let _ = send.send(chosen);
        });
    let chosen = receive.await.map_err(|_| ExportError::InvalidLocation)?;
    tauri::async_runtime::spawn_blocking(move || {
        let Some(chosen) = chosen else {
            return Ok(None);
        };
        let mut path: PathBuf = chosen
            .into_path()
            .map_err(|_| ExportError::InvalidLocation)?;
        if path.extension().is_none() {
            path.set_extension("zip");
            if path.exists() {
                return Err(ExportError::InvalidLocation);
            }
        }
        if !path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"))
        {
            return Err(ExportError::InvalidLocation);
        }
        pack.write(&path)?;
        Ok(Some(SavedExport {
            path: path.to_string_lossy().into_owned(),
            document_count: pack.files.len(),
            issue_count: pack.issues,
        }))
    })
    .await
    .map_err(|_| ExportError::WriteFailed)?
}

#[tauri::command]
pub fn copy_handoff(app: AppHandle) -> Result<()> {
    app.clipboard()
        .write_text(package::HANDOFF)
        .map_err(|_| ExportError::ClipboardUnavailable)
}
