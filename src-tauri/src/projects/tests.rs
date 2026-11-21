use super::*;

#[test]
fn saves_reopens_and_keeps_projects_separate() {
    let path = std::env::temp_dir().join(format!("ngodik-test-{}.sqlite3", std::process::id()));
    let mut store = ProjectStore::open(&path).unwrap();
    let first = store.create(" Inventaris ", "").unwrap();
    let second = store.create("Reservasi", "Jadwalkan pemesanan.").unwrap();
    store
        .save(first.id, first.revision, "Catat stok 📦")
        .unwrap();
    drop(store);
    let store = ProjectStore::open(&path).unwrap();
    assert_eq!(store.list().unwrap().len(), 2);
    let project = store.get(first.id).unwrap();
    assert_eq!(project.title, "Inventaris");
    assert_eq!(project.idea, "Catat stok 📦");
    assert_eq!(project.history.len(), 2);
    assert_eq!(store.get(second.id).unwrap().idea, "Jadwalkan pemesanan.");
    drop(store);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn failed_or_stale_write_preserves_current_version_and_history_is_bounded() {
    let mut store = ProjectStore::open(std::path::Path::new(":memory:")).unwrap();
    assert!(store.create("  ", "").is_err());
    let mut project = store.create("Uji", "").unwrap();
    for step in 0..12 {
        project = store
            .save(project.id, project.revision, &format!("Ide {step}"))
            .unwrap();
    }
    assert!(store.save(project.id, 0, "Perubahan lama").is_err());
    assert!(
        store
            .save(project.id, project.revision, &"a".repeat(16385))
            .is_err()
    );
    store
        .connection
        .execute_batch(
            "CREATE TRIGGER fail_checkpoint BEFORE INSERT ON idea_checkpoints
         BEGIN SELECT RAISE(ABORT, 'checkpoint failed'); END;",
        )
        .unwrap();
    assert!(
        store
            .save(project.id, project.revision, "Harus batal")
            .is_err()
    );
    let restored = store.get(project.id).unwrap();
    assert_eq!(restored.idea, "Ide 11");
    assert_eq!(restored.revision, project.revision);
    assert_eq!(restored.history.len(), 10);
}

#[test]
fn rename_and_delete_preserve_other_projects_and_reserve_deleted_ids() {
    let root = tempfile::tempdir().unwrap();
    let database = root.path().join("projects.sqlite3");
    let mut store = ProjectStore::open(&database).unwrap();
    let kept = store.create("Tetap ada", "Ide kedua").unwrap();
    let project = store.create("Nama awal", "Ide awal").unwrap();
    let project = store
        .save(project.id, project.revision, "Ide tersimpan")
        .unwrap();
    assert!(matches!(
        store.rename(project.id, "  "),
        Err(ProjectError::InvalidTitle)
    ));
    store.rename(project.id, " Nama baru ").unwrap();
    let renamed = store.get(project.id).unwrap();
    assert_eq!(renamed.title, "Nama baru");
    assert_eq!(renamed.revision, project.revision);
    assert_eq!(renamed.history.len(), 2);

    let connection = &store.connection;
    connection.execute(
        "INSERT INTO draft_runs(id,project_id,idea_revision,idea,target,status) VALUES (10,?1,0,'','','running')",
        [project.id],
    ).unwrap();
    assert!(matches!(
        store.delete(project.id),
        Err(ProjectError::DraftRunning)
    ));
    assert!(store.get(project.id).is_ok());
    store
        .connection
        .execute("UPDATE draft_runs SET status='completed'", [])
        .unwrap();
    store.connection.execute(
        "INSERT INTO blueprint_versions(id,project_id,run_id,number,parent_id,restored_from,created_at,summary)
         VALUES (20,?1,10,1,NULL,NULL,0,''),(21,?1,10,2,20,20,0,'')",
        [project.id],
    ).unwrap();
    let directory = root
        .path()
        .join("projects")
        .join(project.id.to_string())
        .join("drafts");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("checkpoint.json"), "{}").unwrap();
    store.delete(project.id).unwrap();
    assert!(matches!(store.get(project.id), Err(ProjectError::NotFound)));
    assert_eq!(store.get(kept.id).unwrap().idea, "Ide kedua");

    drop(store);
    let mut store = ProjectStore::open(&database).unwrap();
    assert_eq!(store.list().unwrap().len(), 1);
    store.clean_deleted_projects(root.path()).unwrap();
    assert!(!directory.exists());
    let next = store.create("Proyek baru", "").unwrap();
    assert!(next.id > project.id);
}
