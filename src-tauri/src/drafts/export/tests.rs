#[test]
fn zip_entry_names_reject_windows_collisions_and_traversal() {
    for invalid in [
        "../outside.md",
        "/absolute",
        "NUL.md",
        "readme.md.",
        "api\\x.md",
        "http:api.yaml",
        "a//b",
    ] {
        assert!(!super::package::valid_name(invalid), "{invalid}");
    }
    assert!(super::package::valid_name("api/OPENAPI.yaml"));
    assert!(super::package::valid_name("README.md"));
}

use std::io::Read;

use serde_json::{Value, json};

use super::{ExportError, check_secret, package};
use crate::{
    drafts::{Stage, qa, storage, versions},
    projects::ProjectStore,
};

fn example(ui: bool, http: bool, extras: bool) -> [Value; 4] {
    let product = json!({
        "schemaVersion":1,"title":"Inventaris Warung","summary":"Catat dan pantau stok.",
        "problem":"Catatan tersebar.","audience":"Pemilik toko.","goals":["Pantau stok"],
        "inScope":["Pencatatan"],"outOfScope":[],
        "requirements":[{"id":"R-1","kind":"functional","statement":"Catat barang dan pantau stok.",
            "acceptanceCriteria":["Barang tampil dalam daftar."],"source":"Ide pengguna"}],
        "decisions":[],"assumptions":[{"text":"Satu pengguna.","source":"Draf awal"}],"openQuestions":[],
        "applicability":{"systemAnalysis":true,"uiUx":ui,"httpApi":http}
    });
    let mut system = json!({"sections":[{"title":"Alur utama","body":"Pengguna membuka daftar.","requirementRefs":["R-1"]}]});
    let mut ux = if ui {
        json!({"sections":[{"title":"Daftar barang","body":"Layar stok.","requirementRefs":["R-1"]}]})
    } else {
        json!({"sections":[]})
    };
    let mut architecture = json!({"sections":[{"title":"Penyimpanan","body":"Data pada perangkat.","requirementRefs":["R-1"]}]});
    if extras {
        system["artifacts"] =
            json!({"flow":{"nodes":["Buka daftar","Pilih barang"],"edges":[[0,1]]}});
        if ui {
            ux["artifacts"] = json!({"wireframes":[{"title":"Daftar barang","mobile":false,"elements":["Cari barang","<script>"]}]});
        }
        architecture["artifacts"] = json!({"implementationPlan":[{"id":"T-1","title":"Pencatatan","description":"Simpan barang lokal.","dependsOn":[],"acceptanceCriteria":["Data bertahan."]}]});
        if http {
            architecture["artifacts"]["apiOperations"] = json!([{"method":"get","path":"/items/{id}",
            "summary":"Baca barang","requestDescription":null,"responseDescription":"Rincian barang","responseStatus":200,"requirementRefs":["R-1"]}]);
        }
    }
    [product, system, ux, architecture]
}

fn backup(docs: &[Value; 4], issue: bool) -> versions::Backup {
    let root = tempfile::tempdir().unwrap();
    let mut store = ProjectStore::open(std::path::Path::new(":memory:")).unwrap();
    store.create("Contoh", "Ide awal").unwrap();
    let target = storage::Target {
        profile_id: "local-fixture".into(),
        base_url: "https://example.test/v1/".into(),
        model_id: "fixture".into(),
    };
    let run_id = storage::create(&mut store, 1, 0, "Ide awal", &target, None, None).unwrap();
    let run = storage::run(&store, run_id).unwrap();
    for stage in Stage::ALL {
        storage::checkpoint(&store, root.path(), &run, stage, &docs[stage as usize]).unwrap();
    }
    let findings = if issue {
        qa::semantic(json!({"findings":[{"severity":"important","message":"Format CSV perlu ditetapkan.",
           "suggestion":"Pilih format kolom CSV.","requirementRefs":["R-1"],"documents":["product","architect"]}]}),docs).unwrap()
    } else {
        vec![]
    };
    qa::save(
        &store.connection,
        run_id,
        &qa::Report {
            deterministic: vec![],
            semantic: findings,
            semantic_status: qa::SemanticStatus::Complete,
        },
    )
    .unwrap();
    storage::finish(&mut store, run_id, "completed", None).unwrap();
    versions::first(&mut store, root.path(), run_id).unwrap();
    let version = versions::head(&store.connection, 1).unwrap().unwrap();
    versions::read(&store, root.path(), 1, version).unwrap()
}

#[test]
fn adaptive_package_has_real_artifacts_and_flags_without_handoff() {
    let docs = example(true, true, true);
    let archive = package::build(&backup(&docs, true)).unwrap();
    assert_eq!(archive.issues, 1);
    assert!(archive.files["README.md"].contains("Perlu ditinjau"));
    assert!(archive.files["PRODUCT_REQUIREMENTS.md"].contains("Format CSV perlu ditetapkan"));
    assert!(archive.files["VALIDATION_AND_QA.md"].contains("#r-1"));
    assert!(archive.files["diagrams/system-flow.mmd"].contains("N0 --> N1"));
    assert!(archive.files["wireframes/view-01.svg"].contains("&lt;script&gt;"));
    assert!(!archive.files["wireframes/view-01.svg"].contains("<script>"));
    assert!(archive.files["IMPLEMENTATION_PLAN.md"].contains("Data bertahan"));
    let api: Value = serde_yaml_ng::from_str(&archive.files["api/OPENAPI.yaml"]).unwrap();
    assert_eq!(
        api["paths"]["/items/{id}"]["get"]["parameters"][0]["name"],
        "id"
    );
    assert_eq!(
        api["paths"]["/items/{id}"]["get"]["responses"]["200"]["description"],
        "Rincian barang"
    );
    assert!(
        archive
            .files
            .values()
            .all(|content| !content.contains(package::HANDOFF))
    );

    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("hasil.zip");
    archive.write(&path).unwrap();
    let mut opened = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    assert_eq!(opened.len(), archive.files.len());
    let mut actual = String::new();
    opened
        .by_name("README.md")
        .unwrap()
        .read_to_string(&mut actual)
        .unwrap();
    assert_eq!(actual, archive.files["README.md"]);
    let unavailable = root.path().join("missing/hasil.zip");
    assert!(matches!(
        archive.write(&unavailable),
        Err(ExportError::WriteFailed)
    ));
    assert!(!unavailable.exists());
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
}

#[test]
fn cli_omits_optional_docs_and_legacy_http_is_marked_as_provisional() {
    let cli = package::build(&backup(&example(false, false, false), false)).unwrap();
    for absent in [
        "UX_SPECIFICATION.md",
        "api/OPENAPI.yaml",
        "IMPLEMENTATION_PLAN.md",
        "diagrams/system-flow.mmd",
        "wireframes/view-01.svg",
    ] {
        assert!(!cli.files.contains_key(absent));
    }
    assert_eq!(cli.issues, 0);
    let old = package::build(&backup(&example(false, true, false), false)).unwrap();
    let api: Value = serde_yaml_ng::from_str(&old.files["api/OPENAPI.yaml"]).unwrap();
    assert_eq!(api["x-ngodik-status"], "Perlu ditinjau");
    assert_eq!(api["x-ngodik-requirements"][0]["id"], "R-1");
    assert_eq!(old.issues, 1);
}

#[test]
fn export_rejects_structural_errors_links_and_known_credentials() {
    let mut docs = example(false, false, false);
    let archive = package::build(&backup(&docs, false)).unwrap();
    let marker = b"fixture-key-no-export";
    let review = json!({"semantic":[{"message":"fixture-key-no-export"}]});
    assert!(matches!(
        check_secret(&archive, &docs, &review, marker),
        Err(ExportError::ContainsCredential)
    ));
    docs[0]["summary"] = serde_json::from_str(r#""fixture\u002dkey-no-export""#).unwrap();
    assert!(matches!(
        check_secret(&archive, &docs, &Value::Null, marker),
        Err(ExportError::ContainsCredential)
    ));
    assert!(
        package::links(
            &archive.files,
            "README.md",
            "[rusak](PRODUCT_REQUIREMENTS.md#r-999)"
        )
        .is_err()
    );
    assert!(package::links(&archive.files, "README.md", "[keluar](../README.md)").is_err());
    docs[1]["sections"][0]["requirementRefs"][0] = json!("R-999");
    assert!(matches!(
        package::build(&backup(&docs, false)),
        Err(ExportError::InvalidBlueprint)
    ));
}
