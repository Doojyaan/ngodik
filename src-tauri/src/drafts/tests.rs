use super::*;

#[test]
fn downstream_document_requires_real_requirement_references() {
    let data = serde_json::json!({"sections":[{"title":"Alur","body":"Buka aplikasi","requirementRefs":["R-9"]}]});
    let refs = ["R-1"];
    assert!(contract::document(data.clone(), &refs, true).is_err());
    assert!(contract::document(data, &["R-9"], true).is_ok());
    assert!(contract::document(serde_json::json!({"sections":[]}), &refs, false).is_ok());
    assert!(contract::document(serde_json::json!({"sections":[]}), &refs, true).is_err());
}

#[test]
fn restart_preserves_referenced_files_and_discards_interrupted_orphans() {
    let root = std::env::temp_dir().join(format!("ngodik-draft-{}", uuid::Uuid::new_v4()));
    let mut store = ProjectStore::open(std::path::Path::new(":memory:")).unwrap();
    store.create("Uji").unwrap();
    let target = Target {
        profile_id: "fixture".into(),
        base_url: "https://example.test/v1/".into(),
        model_id: "fixture".into(),
    };
    let run_id = storage::create(&mut store, 1, 0, "Ide", &target).unwrap();
    assert!(matches!(
        storage::create(&mut store, 1, 0, "Ide", &target),
        Err(DraftError::Busy)
    ));
    let run = storage::run(&store, run_id).unwrap();
    let product = serde_json::json!({"title":"Draf"});
    storage::begin_attempt(&store, run_id, Stage::Product).unwrap();
    storage::checkpoint(&store, &root, &run, Stage::Product, &product).unwrap();
    let dir = root.join("projects/1/drafts").join(run_id.to_string());
    let orphan = dir.join(format!("{}.json", uuid::Uuid::new_v4()));
    std::fs::write(&orphan, "{}").unwrap();
    storage::begin_attempt(&store, run_id, Stage::Ux).unwrap();
    storage::interrupt(&store, &root).unwrap();
    assert!(!orphan.exists());
    let run = storage::run(&store, run_id).unwrap();
    assert_eq!(run.status, "interrupted");
    storage::resume(&mut store, run_id, 1).unwrap();
    assert_eq!(
        storage::cached(&store, &root, &run, Stage::Product).unwrap(),
        Some(product)
    );
    let view = storage::show(&store, &root, storage::run(&store, run_id).unwrap()).unwrap();
    assert_eq!(view.stages[0].status, "completed");
    assert_eq!(view.stages[2].status, "waiting");

    let entries: Vec<_> = std::fs::read_dir(&dir).unwrap().collect();
    assert_eq!(entries.len(), 1);
    std::fs::remove_file(entries.into_iter().next().unwrap().unwrap().path()).unwrap();
    let view = storage::show(&store, &root, storage::run(&store, run_id).unwrap()).unwrap();
    assert_eq!(
        view.stages[0].error.as_deref(),
        Some("checkpoint_unreadable")
    );
    for directory in dir.ancestors().take(5) {
        std::fs::remove_dir(directory).unwrap();
    }
}

#[test]
fn decoded_sensitive_output_is_recognized_before_checkpoint() {
    let data: Value = serde_json::from_str(r#"{"nested":[{"text":"fixture\u002dkey"}]}"#).unwrap();
    assert!(value_contains_key(&data, b"fixture-key"));
    assert!(!value_contains_key(&data, b"another-key"));
}
