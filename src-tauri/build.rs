fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "list_projects",
            "create_project",
            "open_project",
            "save_idea",
        ]),
    ))
    .expect("failed to build Tauri configuration");
}
