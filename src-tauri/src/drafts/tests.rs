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
    store.create("Uji", "").unwrap();
    let target = Target {
        profile_id: "fixture".into(),
        base_url: "https://example.test/v1/".into(),
        model_id: "fixture".into(),
    };
    let run_id = storage::create(&mut store, 1, 0, "Ide", &target, None, None).unwrap();
    assert!(matches!(
        storage::create(&mut store, 1, 0, "Ide", &target, None, None),
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

fn example_docs(title: &str) -> [Value; 4] {
    [
        json!({
            "schemaVersion":1,"title":title,"summary":"Catat stok", "problem":"Catatan tersebar",
            "audience":"Pemilik toko","goals":["Pantau stok"],"inScope":["Pencatatan"],"outOfScope":[],
            "requirements":[{"id":"R-1","kind":"functional","statement":"Catat barang",
                "acceptanceCriteria":["Barang muncul"],"source":"Ide pengguna"}],
            "decisions":[],"assumptions":[],"openQuestions":[],
            "applicability":{"systemAnalysis":true,"uiUx":false,"httpApi":false}
        }),
        json!({"sections":[{"title":"Alur","body":"Catat barang","requirementRefs":["R-1"]}]}),
        json!({"sections":[]}),
        json!({"sections":[{"title":"Data","body":"Data lokal","requirementRefs":["R-1"]}]}),
    ]
}

#[test]
fn qa_separates_structure_from_semantic_proposals() {
    let original = example_docs("Toko");
    assert!(qa::deterministic(&original).is_empty());
    let findings = json!({"findings":[{"severity":"important","message":"Retensi belum jelas",
        "suggestion":"Tentukan masa retensi","requirementRefs":["R-1"],"documents":["analyst"]}]});
    let accepted = qa::semantic(findings.clone(), &original).unwrap();
    assert_eq!(accepted.len(), 1);
    assert_eq!(original[0]["title"], "Toko");
    let mut invalid = findings;
    invalid["findings"][0]["requirementRefs"][0] = json!("R-99");
    assert!(qa::semantic(invalid, &original).is_err());
    assert!(
        qa::semantic(
            json!({"findings":[],"product":{"title":"Diubah"}}),
            &original
        )
        .is_err()
    );

    let mut bad = original.clone();
    bad[0]["schemaVersion"] = json!(2);
    bad[1]["sections"][0]["requirementRefs"][0] = json!("R-99");
    assert_eq!(qa::deterministic(&bad).len(), 2);
}

#[test]
fn version_backups_are_bounded_distinct_and_integrity_checked() {
    let root = std::env::temp_dir().join(format!("ngodik-recovery-{}", uuid::Uuid::new_v4()));
    let mut store = ProjectStore::open(std::path::Path::new(":memory:")).unwrap();
    store.create("Toko", "Ide awal").unwrap();
    store.create("Proyek lain", "").unwrap();
    let target = Target {
        profile_id: "test".into(),
        base_url: "https://example.test/v1/".into(),
        model_id: "test".into(),
    };
    let run_id = storage::create(&mut store, 1, 0, "Ide awal", &target, None, None).unwrap();
    assert!(qa::begin(&store.connection, run_id).unwrap());
    assert!(!qa::begin(&store.connection, run_id).unwrap());
    let baseline = example_docs("Toko");
    let run = storage::run(&store, run_id).unwrap();
    for stage in Stage::ALL {
        storage::checkpoint(&store, &root, &run, stage, &baseline[stage as usize]).unwrap();
    }
    storage::finish(&mut store, run_id, "completed", None).unwrap();
    versions::initialize(&mut store, &root).unwrap();
    let first = versions::list(&store, 1).unwrap().remove(0);
    let first_backup = versions::read(&store, &root, 1, first.id).unwrap();
    assert!(matches!(
        first_backup.draft.qa.unwrap().semantic_status,
        qa::SemanticStatus::NotReviewed
    ));
    assert!(matches!(
        versions::read(&store, &root, 2, first.id),
        Err(DraftError::RecoveryUnavailable)
    ));

    for number in 2..=12 {
        let head = versions::head(&store.connection, 1).unwrap();
        let mut next = storage::show(&store, &root, storage::run(&store, run_id).unwrap()).unwrap();
        next.stages[0].output.as_mut().unwrap()["title"] = json!(format!("Toko {number}"));
        versions::publish(
            &mut store,
            &root,
            1,
            head,
            next,
            "Ide awal".into(),
            "Revisi",
            None,
        )
        .unwrap();
    }
    let list = versions::list(&store, 1).unwrap();
    assert_eq!(list.len(), 12);
    assert_eq!(
        list.iter().filter(|version| version.recoverable).count(),
        10
    );
    assert!(matches!(
        versions::read(&store, &root, 1, first.id),
        Err(DraftError::RecoveryUnavailable)
    ));
    let folder = root.join("projects/1/recovery");
    assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 10);
    let original = versions::read(&store, &root, 1, list[5].id).unwrap();
    let before = versions::read(&store, &root, 1, list[0].id).unwrap();
    assert_eq!(
        changes::compare(&before.draft.documents(), &original.draft.documents()).len(),
        1
    );
    versions::publish(
        &mut store,
        &root,
        1,
        Some(list[0].id),
        original.draft,
        original.idea,
        "Pemulihan",
        Some(original.version.id),
    )
    .unwrap();
    let head = versions::head(&store.connection, 1).unwrap().unwrap();
    assert_eq!(
        versions::read(&store, &root, 1, head)
            .unwrap()
            .draft
            .documents()[0]["title"],
        "Toko 7"
    );
    let filename: String = store
        .connection
        .query_row(
            "SELECT recovery_file FROM blueprint_versions WHERE id=?1",
            [head],
            |row| row.get(0),
        )
        .unwrap();
    let path = folder.join(format!("{filename}.json"));
    let mut bytes = std::fs::read(&path).unwrap();
    bytes.push(b' ');
    std::fs::write(path, bytes).unwrap();
    assert!(matches!(
        versions::read(&store, &root, 1, head),
        Err(DraftError::RecoveryUnavailable)
    ));
    // All files here were generated for this test in a unique temporary directory.
    std::fs::remove_dir_all(root).unwrap();
}
