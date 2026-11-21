fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "list_projects",
            "create_project",
            "open_project",
            "save_idea",
            "list_profiles",
            "save_profile",
            "set_profile_key",
            "delete_profile",
            "sync_models",
            "test_connection",
            "project_model",
            "set_project_model",
            "draft_status",
            "start_draft",
            "resume_draft",
            "cancel_draft",
        ]),
    ))
    .expect("failed to build Tauri configuration");
}
