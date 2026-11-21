#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod drafts;
mod projects;
mod providers;

use std::sync::{Mutex, MutexGuard};

use projects::{Project, ProjectError, ProjectStore, ProjectSummary};
use tauri::{Manager, State};

type Store = Mutex<ProjectStore>;

fn store<'a>(state: &'a State<'_, Store>) -> Result<MutexGuard<'a, ProjectStore>, ProjectError> {
    state
        .inner()
        .lock()
        .map_err(|_| ProjectError::StorageUnavailable)
}

#[tauri::command]
fn list_projects(state: State<'_, Store>) -> Result<Vec<ProjectSummary>, ProjectError> {
    store(&state)?.list()
}

#[tauri::command]
fn create_project(
    state: State<'_, Store>,
    title: String,
    idea: Option<String>,
) -> Result<Project, ProjectError> {
    store(&state)?.create(&title, idea.as_deref().unwrap_or_default())
}

#[tauri::command]
fn open_project(state: State<'_, Store>, id: i64) -> Result<Project, ProjectError> {
    store(&state)?.get(id)
}

#[tauri::command]
fn rename_project(state: State<'_, Store>, id: i64, title: String) -> Result<(), ProjectError> {
    store(&state)?.rename(id, &title)
}

#[tauri::command]
fn delete_project(
    app: tauri::AppHandle,
    state: State<'_, Store>,
    id: i64,
) -> Result<(), ProjectError> {
    let root = app
        .path()
        .app_local_data_dir()
        .map_err(|_| ProjectError::StorageUnavailable)?;
    let mut store = store(&state)?;
    store.delete(id)?;
    let _ = store.clean_deleted_projects(&root);
    Ok(())
}

#[tauri::command]
fn save_idea(
    state: State<'_, Store>,
    id: i64,
    revision: i64,
    idea: String,
) -> Result<Project, ProjectError> {
    store(&state)?.save(id, revision, &idea)
}

fn main() {
    let context = tauri::generate_context!();
    let Some(_instance) =
        Instance::acquire(&context.config().identifier).expect("Kunci aplikasi tidak dapat dibuat")
    else {
        return;
    };
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let directory = app.path().app_local_data_dir()?;
            std::fs::create_dir_all(&directory)?;
            let mut projects = ProjectStore::open(&directory.join("ngodik.sqlite3"))
                .map_err(|_| std::io::Error::other("Penyimpanan proyek tidak dapat dibuka"))?;
            let _ = projects.clean_deleted_projects(&directory);
            app.manage(
                drafts::Drafts::new(directory, &mut projects)
                    .map_err(|_| std::io::Error::other("Checkpoint draf tidak dapat dibuka"))?,
            );
            app.manage(Mutex::new(projects));
            app.manage(
                providers::http_client()
                    .map_err(|_| std::io::Error::other("Koneksi model tidak dapat disiapkan"))?,
            );
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_projects,
            create_project,
            open_project,
            rename_project,
            delete_project,
            save_idea,
            providers::list_profiles,
            providers::save_profile,
            providers::set_profile_key,
            providers::delete_profile,
            providers::sync_models,
            providers::test_connection,
            providers::project_model,
            providers::set_project_model,
            drafts::draft_status,
            drafts::start_draft,
            drafts::resume_draft,
            drafts::cancel_draft,
            drafts::revisions::revise_draft,
            drafts::revisions::decide_proposal,
            drafts::revisions::restore_blueprint,
            drafts::export::export_preview,
            drafts::export::save_blueprint,
            drafts::export::copy_handoff,
        ])
        .run(context)
        .expect("ngodik gagal dibuka");
}

struct Instance(windows_sys::Win32::Foundation::HANDLE);

impl Instance {
    fn acquire(identifier: &str) -> std::io::Result<Option<Self>> {
        use windows_sys::Win32::{
            Foundation::{ERROR_ALREADY_EXISTS, GetLastError},
            System::Threading::CreateMutexW,
        };
        let name: Vec<_> = format!("Local\\{identifier}-instance")
            .encode_utf16()
            .chain([0])
            .collect();
        // An open handle reserves this application instance for the desktop session.
        unsafe {
            let handle = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
            if handle.is_null() {
                return Err(std::io::Error::last_os_error());
            }
            let exists = GetLastError() == ERROR_ALREADY_EXISTS;
            Ok((!exists).then_some(Self(handle)))
        }
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        // This handle was returned by CreateMutexW.
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}
