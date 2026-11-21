use std::{
    collections::{BTreeMap, HashSet},
    fmt::Write as _,
    io::Write as _,
    path::Path,
};

use pulldown_cmark::{Event, Parser, Tag};
use serde::Serialize;
use serde_json::Value;
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

use super::{ExportError, Result, render};
use crate::drafts::{Stage, qa, versions::Backup};

pub const HANDOFF: &str = "Gunakan folder hasil ekstrak ZIP ngodik sebagai acuan proyek. Baca README.md, AGENTS.md, dan PROJECT_SPECIFICATION.json, lalu baca dokumen yang dirujuk. Periksa lingkungan dan skill yang benar-benar tersedia. Tunjukkan rencana bertahap, asumsi, risiko, kriteria penerimaan, dan cara verifikasi. Selaraskan konflik atau keputusan besar bersama saya. Minta persetujuan sebelum menulis kode dan setelah melaporkan hasil setiap tahap.";

const AGENTS: &str = "# Panduan untuk coding agent\n\n1. Baca [README](README.md), [spesifikasi kanonis](PROJECT_SPECIFICATION.json), dan dokumen yang berlaku. Periksa workspace dan skill yang tersedia di akun ini.\n2. Susun rencana implementasi bertahap, asumsi, risiko, serta cara memverifikasi kriteria penerimaan. Tunjukkan rencana dan minta persetujuan sebelum coding.\n3. Ikuti keputusan yang sudah dicatat. Jika ada konflik atau keputusan besar yang belum jelas, tunjukkan sumber dan tanyakan kepada pengguna sebelum mengerjakan bagian yang terdampak.\n4. Kerjakan satu tahap. Laporkan hasil, tes, dan batas yang tersisa, lalu minta persetujuan pengguna sebelum lanjut.\n";

const READING_ORDER: [(&str, &str); 7] = [
    (render::PRODUCT, "Kebutuhan produk"),
    (
        "ASSUMPTIONS_AND_OPEN_QUESTIONS.md",
        "Asumsi dan pertanyaan terbuka",
    ),
    ("SYSTEM_ANALYSIS.md", "Analisis sistem"),
    ("UX_SPECIFICATION.md", "Spesifikasi UI/UX"),
    ("SOLUTION_ARCHITECTURE.md", "Arsitektur solusi"),
    ("IMPLEMENTATION_PLAN.md", "Rencana implementasi"),
    (render::QA, "Validasi dan QA"),
];

pub struct Package {
    pub files: BTreeMap<String, String>,
    pub name: String,
    pub number: i64,
    pub issues: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    version_id: i64,
    version_number: i64,
    suggested_name: String,
    documents: Vec<String>,
    issue_count: usize,
}

impl Package {
    pub fn preview(&self, version_id: i64) -> Preview {
        Preview {
            version_id,
            version_number: self.number,
            suggested_name: self.name.clone(),
            documents: self.files.keys().cloned().collect(),
            issue_count: self.issues,
        }
    }

    pub fn write(&self, destination: &Path) -> Result<()> {
        let parent = destination.parent().ok_or(ExportError::InvalidLocation)?;
        let mut temporary =
            tempfile::NamedTempFile::new_in(parent).map_err(|_| ExportError::WriteFailed)?;
        {
            let mut zip = ZipWriter::new(temporary.as_file_mut());
            let options =
                SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
            for (name, contents) in &self.files {
                zip.start_file(name, options)
                    .map_err(|_| ExportError::WriteFailed)?;
                zip.write_all(contents.as_bytes())
                    .map_err(|_| ExportError::WriteFailed)?;
            }
            zip.finish()
                .map_err(|_| ExportError::WriteFailed)?
                .sync_all()
                .map_err(|_| ExportError::WriteFailed)?;
        }
        temporary
            .persist(destination)
            .map_err(|_| ExportError::WriteFailed)?;
        Ok(())
    }
}

pub fn build(backup: &Backup) -> Result<Package> {
    let docs = backup.draft.documents();
    if !qa::deterministic(&docs).is_empty() {
        return Err(ExportError::InvalidBlueprint);
    }
    let spec = &docs[Stage::Product as usize];
    let title = spec["title"]
        .as_str()
        .ok_or(ExportError::InvalidBlueprint)?;
    let report = backup.draft.qa.as_ref();
    let mut flags = Vec::new();
    let semantic_complete =
        report.is_some_and(|report| matches!(report.semantic_status, qa::SemanticStatus::Complete));
    if !semantic_complete {
        flags.push("Tinjauan semantik tidak tersedia untuk versi ini.".into());
    }
    for finding in report.into_iter().flat_map(|report| &report.semantic) {
        if matches!(finding.severity, qa::Severity::Important) {
            flags.push(finding.message.clone());
        }
    }
    let needs_api = spec["applicability"]["httpApi"] == true;
    let analyst = render::assets(&docs[1]).map_err(|_| ExportError::InvalidBlueprint)?;
    let ux = render::assets(&docs[2]).map_err(|_| ExportError::InvalidBlueprint)?;
    let architecture = render::assets(&docs[3]).map_err(|_| ExportError::InvalidBlueprint)?;
    if needs_api && architecture.api_operations.is_none() {
        flags.push("Rute dan operasi HTTP belum ditentukan. Tinjau kontrak OpenAPI sementara sebelum implementasi.".into());
    }

    let mut files = BTreeMap::new();
    files.insert("AGENTS.md".into(), AGENTS.into());
    files.insert(
        render::CANONICAL.into(),
        serde_json::to_string_pretty(spec).map_err(|_| ExportError::InvalidBlueprint)? + "\n",
    );
    files.insert(render::PRODUCT.into(), render::product(spec));
    if let Some(assumptions) = render::assumptions(spec) {
        files.insert("ASSUMPTIONS_AND_OPEN_QUESTIONS.md".into(), assumptions);
    }
    for (stage, name) in [
        (Stage::Analyst, "SYSTEM_ANALYSIS.md"),
        (Stage::Ux, "UX_SPECIFICATION.md"),
        (Stage::Architect, "SOLUTION_ARCHITECTURE.md"),
    ] {
        let value = &docs[stage as usize];
        if value["sections"]
            .as_array()
            .is_some_and(|sections| !sections.is_empty())
        {
            files.insert(name.into(), render::document(render::label(stage), value));
        }
    }
    if let Some(flow) = analyst.flow {
        add_asset(
            &mut files,
            "SYSTEM_ANALYSIS.md",
            "diagrams/system-flow.mmd",
            "Diagram analisis sistem",
            render::diagram(&flow),
        )?;
    }
    if let Some(flow) = architecture.flow {
        add_asset(
            &mut files,
            "SOLUTION_ARCHITECTURE.md",
            "diagrams/solution-flow.mmd",
            "Diagram arsitektur",
            render::diagram(&flow),
        )?;
    }
    if let Some(wireframes) = ux.wireframes {
        for (index, view) in wireframes.iter().enumerate() {
            add_asset(
                &mut files,
                "UX_SPECIFICATION.md",
                &format!("wireframes/view-{:02}.svg", index + 1),
                &render::line(&view.title),
                render::wireframe(view),
            )?;
        }
    }
    if let Some(steps) = architecture.implementation_plan {
        files.insert("IMPLEMENTATION_PLAN.md".into(), render::plan(&steps));
    }
    if needs_api {
        let content = render::openapi(
            spec,
            backup.version.number,
            architecture.api_operations.as_deref(),
        )
        .map_err(|_| ExportError::InvalidPackage)?;
        add_asset(
            &mut files,
            "SOLUTION_ARCHITECTURE.md",
            "api/OPENAPI.yaml",
            "Kontrak API HTTP",
            content,
        )?;
        if architecture.api_operations.is_none() {
            note_in(
                &mut files,
                "SOLUTION_ARCHITECTURE.md",
                "Rute dan operasi HTTP belum ditentukan.",
            )?;
        }
    }
    for finding in report.into_iter().flat_map(|report| &report.semantic) {
        if !matches!(finding.severity, qa::Severity::Important) {
            continue;
        }
        for stage in &finding.documents {
            let name = match stage {
                Stage::Product => render::PRODUCT,
                Stage::Analyst => "SYSTEM_ANALYSIS.md",
                Stage::Ux => "UX_SPECIFICATION.md",
                Stage::Architect => "SOLUTION_ARCHITECTURE.md",
            };
            if files.contains_key(name) {
                note_in(&mut files, name, &finding.message)?;
            }
        }
    }
    files.insert(render::QA.into(), render::checks(spec, report, &flags));
    files.insert(
        "README.md".into(),
        readme(title, spec, backup.version.number, &files, &flags),
    );
    validate(&files, spec)?;
    Ok(Package {
        files,
        number: backup.version.number,
        name: suggested_name(title, backup.version.number),
        issues: flags.len(),
    })
}

fn add_asset(
    files: &mut BTreeMap<String, String>,
    parent: &str,
    name: &str,
    label: &str,
    content: String,
) -> Result<()> {
    let parent = files.get_mut(parent).ok_or(ExportError::InvalidPackage)?;
    let _ = writeln!(parent, "\nLihat [{label}]({name}).");
    files.insert(name.into(), content);
    Ok(())
}

fn note_in(files: &mut BTreeMap<String, String>, name: &str, note: &str) -> Result<()> {
    let text = files.get_mut(name).ok_or(ExportError::InvalidPackage)?;
    let _ = writeln!(
        text,
        "\n> **Perlu ditinjau.** {} Lihat [Validasi dan QA]({}).",
        render::line(note),
        render::QA
    );
    Ok(())
}

fn readme(
    title: &str,
    spec: &Value,
    number: i64,
    files: &BTreeMap<String, String>,
    flags: &[String],
) -> String {
    let summary = render::escape(spec["summary"].as_str().unwrap_or_default());
    let mut result = format!(
        "# {}\n\n{summary}\n\nBlueprint versi {number}. Format paket ngodik v1 dan skema kanonis v1.\n",
        render::line(title)
    );
    if !flags.is_empty() {
        let _ = writeln!(
            result,
            "\n## Perlu ditinjau\n\n{} hal perlu ditinjau. Baca [Validasi dan QA]({}).",
            flags.len(),
            render::QA
        );
        for flag in flags {
            let _ = writeln!(result, "- {}", render::line(flag));
        }
    }
    result.push_str("\n## Urutan baca\n\n1. [Panduan coding agent](AGENTS.md)\n2. [Spesifikasi kanonis](PROJECT_SPECIFICATION.json)\n");
    let mut index = 3;
    for (name, label) in READING_ORDER {
        if files.contains_key(name) {
            let _ = writeln!(result, "{index}. [{label}]({name})");
            index += 1;
        }
    }
    if files.keys().any(|name| name.contains('/')) {
        result.push_str("\n## Lampiran\n\n");
        for name in files.keys().filter(|name| name.contains('/')) {
            let _ = writeln!(result, "- [{name}]({name})");
        }
    }
    result
}

fn suggested_name(title: &str, number: i64) -> String {
    let slug = title
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    let compact = slug
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let compact = &compact[..compact.len().min(48)];
    let name = if compact.is_empty() {
        "proyek"
    } else {
        compact
    };
    format!("blueprint-{name}-v{number}.zip")
}

pub fn valid_name(name: &str) -> bool {
    fn safe(segment: &str) -> bool {
        if segment.is_empty()
            || segment.len() > 80
            || segment.starts_with('.')
            || segment.ends_with('.')
            || !segment
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"_-.\x20".contains(&byte))
            || segment.trim() != segment
        {
            return false;
        }
        let stem = segment
            .split('.')
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        !matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            && !["COM", "LPT"].iter().any(|prefix| {
                stem.strip_prefix(prefix).is_some_and(|suffix| {
                    suffix.len() == 1 && matches!(suffix.as_bytes()[0], b'1'..=b'9')
                })
            })
    }
    !name.is_empty() && name.rsplit('/').all(safe)
}

fn validate(files: &BTreeMap<String, String>, spec: &Value) -> Result<()> {
    let mut seen = HashSet::new();
    for (name, content) in files {
        if !valid_name(name) || !seen.insert(name.to_ascii_lowercase()) || content.trim().is_empty()
        {
            return Err(ExportError::InvalidPackage);
        }
        if name.ends_with(".md") {
            links(files, name, content)?;
        }
    }
    for mandatory in [
        "README.md",
        "AGENTS.md",
        render::CANONICAL,
        render::PRODUCT,
        "SOLUTION_ARCHITECTURE.md",
        render::QA,
    ] {
        if !files.contains_key(mandatory) {
            return Err(ExportError::InvalidPackage);
        }
    }
    for (field, name) in [
        ("systemAnalysis", "SYSTEM_ANALYSIS.md"),
        ("uiUx", "UX_SPECIFICATION.md"),
        ("httpApi", "api/OPENAPI.yaml"),
    ] {
        if (spec["applicability"][field] == true) != files.contains_key(name) {
            return Err(ExportError::InvalidPackage);
        }
    }
    let canonical = serde_json::from_str::<Value>(&files[render::CANONICAL])
        .map_err(|_| ExportError::InvalidPackage)?;
    if &canonical != spec {
        return Err(ExportError::InvalidPackage);
    }
    if let Some(openapi) = files.get("api/OPENAPI.yaml") {
        let api: Value =
            serde_yaml_ng::from_str(openapi).map_err(|_| ExportError::InvalidPackage)?;
        if api["openapi"] != "3.1.1" || !api["paths"].is_object() || !api["info"].is_object() {
            return Err(ExportError::InvalidPackage);
        }
    }
    Ok(())
}

pub fn links(files: &BTreeMap<String, String>, source: &str, content: &str) -> Result<()> {
    for element in Parser::new(content) {
        let Event::Start(Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. }) = element else {
            continue;
        };
        let (path, anchor) = dest_url
            .split_once('#')
            .map_or((dest_url.as_ref(), None), |(path, anchor)| {
                (path, Some(anchor))
            });
        let target = if path.is_empty() {
            source.to_owned()
        } else {
            if !valid_name(path) {
                return Err(ExportError::InvalidPackage);
            }
            source
                .rsplit_once('/')
                .map_or_else(|| path.to_owned(), |(parent, _)| format!("{parent}/{path}"))
        };
        let destination = files.get(&target).ok_or(ExportError::InvalidPackage)?;
        if let Some(anchor) = anchor {
            if anchor.is_empty()
                || !destination.lines().any(|line| {
                    line.strip_prefix("### ")
                        .is_some_and(|heading| heading.eq_ignore_ascii_case(anchor))
                })
            {
                return Err(ExportError::InvalidPackage);
            }
        }
    }
    Ok(())
}
