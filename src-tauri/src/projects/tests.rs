use super::*;

#[test]
fn saves_reopens_and_keeps_projects_separate() {
    let path = std::env::temp_dir().join(format!("ngodik-test-{}.sqlite3", std::process::id()));
    let mut store = ProjectStore::open(&path).unwrap();
    let first = store.create(" Inventaris ").unwrap();
    let second = store.create("Reservasi").unwrap();
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
    assert!(store.get(second.id).unwrap().idea.is_empty());
    drop(store);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn failed_or_stale_write_preserves_current_version_and_history_is_bounded() {
    let mut store = ProjectStore::open(std::path::Path::new(":memory:")).unwrap();
    assert!(store.create("  ").is_err());
    let mut project = store.create("Uji").unwrap();
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
