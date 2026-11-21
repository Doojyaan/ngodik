#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

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
fn create_project(state: State<'_, Store>, title: String) -> Result<Project, ProjectError> {
    store(&state)?.create(&title)
}

#[tauri::command]
fn open_project(state: State<'_, Store>, id: i64) -> Result<Project, ProjectError> {
    store(&state)?.get(id)
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
    tauri::Builder::default()
        .setup(|app| {
            let directory = app.path().app_local_data_dir()?;
            std::fs::create_dir_all(&directory)?;
            let projects = ProjectStore::open(&directory.join("ngodik.sqlite3"))
                .map_err(|_| std::io::Error::other("Penyimpanan proyek tidak dapat dibuka"))?;
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
            save_idea,
            providers::list_profiles,
            providers::save_profile,
            providers::set_profile_key,
            providers::delete_profile,
            providers::sync_models,
            providers::test_connection,
            providers::project_model,
            providers::set_project_model,
        ])
        .run(tauri::generate_context!())
        .expect("ngodik gagal dibuka");
}
