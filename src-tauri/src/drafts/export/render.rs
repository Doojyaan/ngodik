use std::fmt::Write;

use serde_json::{Map, Value, json};

use crate::drafts::{
    Stage,
    extras::{self, Artifacts, Flow, Operation, Step, Wireframe},
    qa,
};

pub const PRODUCT: &str = "PRODUCT_REQUIREMENTS.md";
pub const QA: &str = "VALIDATION_AND_QA.md";
pub const CANONICAL: &str = "PROJECT_SPECIFICATION.json";

pub fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '\n' => escaped.push('\n'),
            '\r' => {}
            character if !character.is_control() => {
                if "\\`*_[]#!~|".contains(character) {
                    escaped.push('\\');
                }
                escaped.push(character);
            }
            _ => {}
        }
    }
    escaped
}

pub fn line(text: &str) -> String {
    escape(&text.split_whitespace().collect::<Vec<_>>().join(" "))
}

fn text<'a>(object: &'a Value, field: &str) -> &'a str {
    object[field].as_str().unwrap_or_default()
}
fn list<'a>(object: &'a Value, field: &str) -> &'a [Value] {
    object[field].as_array().map_or(&[], Vec::as_slice)
}

fn bullets(destination: &mut String, title: &str, items: &[Value]) {
    if items.is_empty() {
        return;
    }
    let _ = writeln!(destination, "\n## {title}\n");
    for item in items.iter().filter_map(Value::as_str) {
        let _ = writeln!(destination, "- {}", line(item));
    }
}

fn trace(ids: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    ids.into_iter()
        .map(|id| {
            let id = id.as_ref();
            format!("[{id}]({PRODUCT}#{})", id.to_ascii_lowercase())
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn related(destination: &mut String, ids: &[Value]) {
    let links = trace(ids.iter().filter_map(Value::as_str));
    if !links.is_empty() {
        let _ = writeln!(destination, "\nKebutuhan terkait {links}.");
    }
}

pub fn product(specification: &Value) -> String {
    let mut result = format!(
        "# Kebutuhan produk\n\n## {}\n\n{}\n\n## Masalah\n\n{}\n\n## Pengguna\n\n{}\n",
        line(text(specification, "title")),
        escape(text(specification, "summary")),
        escape(text(specification, "problem")),
        escape(text(specification, "audience"))
    );
    bullets(&mut result, "Tujuan", list(specification, "goals"));
    bullets(&mut result, "Dalam cakupan", list(specification, "inScope"));
    bullets(
        &mut result,
        "Di luar cakupan",
        list(specification, "outOfScope"),
    );
    for (kind, title) in [
        ("functional", "Kebutuhan fungsional"),
        ("nonfunctional", "Kebutuhan nonfungsional"),
    ] {
        let mut group = list(specification, "requirements")
            .iter()
            .filter(|row| text(row, "kind") == kind)
            .peekable();
        if group.peek().is_none() {
            continue;
        }
        let _ = writeln!(result, "\n## {title}");
        for row in group {
            let _ = writeln!(
                result,
                "\n### {}\n\n{}\n\nSumber {}.\n\nKriteria penerimaan\n",
                text(row, "id"),
                escape(text(row, "statement")),
                line(text(row, "source"))
            );
            for criterion in list(row, "acceptanceCriteria")
                .iter()
                .filter_map(Value::as_str)
            {
                let _ = writeln!(result, "- {}", line(criterion));
            }
        }
    }
    let decisions = list(specification, "decisions");
    if !decisions.is_empty() {
        result.push_str("\n## Keputusan\n");
        findings(&mut result, decisions);
    }
    result
}

pub fn assumptions(specification: &Value) -> Option<String> {
    let mut result = String::from("# Asumsi dan pertanyaan terbuka\n");
    let mut has_content = false;
    for (field, title) in [
        ("assumptions", "Asumsi"),
        ("openQuestions", "Pertanyaan terbuka"),
    ] {
        let items = list(specification, field);
        if !items.is_empty() {
            has_content = true;
            let _ = writeln!(result, "\n## {title}");
            findings(&mut result, items);
        }
    }
    has_content.then_some(result)
}

fn findings(destination: &mut String, items: &[Value]) {
    destination.push('\n');
    for item in items {
        let _ = writeln!(
            destination,
            "- {} (Sumber {})",
            line(text(item, "text")),
            line(text(item, "source"))
        );
    }
}

pub fn document(title: &str, value: &Value) -> String {
    let mut result = format!("# {title}\n");
    for section in list(value, "sections") {
        let _ = writeln!(
            result,
            "\n## {}\n\n{}",
            line(text(section, "title")),
            escape(text(section, "body"))
        );
        related(&mut result, list(section, "requirementRefs"));
    }
    result
}

pub fn assets(document: &Value) -> Result<Artifacts, serde_json::Error> {
    document
        .get("artifacts")
        .map(|value| serde_json::from_value(value.clone()))
        .transpose()
        .map(Option::unwrap_or_default)
}

pub fn plan(steps: &[Step]) -> String {
    let mut result = String::from(
        "# Rencana implementasi\n\nKonfirmasi rencana sebelum menulis kode. Setelah setiap tahap, laporkan hasil dan verifikasi, lalu minta persetujuan sebelum lanjut.\n",
    );
    for step in steps {
        let _ = writeln!(
            result,
            "\n## {} · {}\n\n{}",
            step.id,
            line(&step.title),
            escape(&step.description)
        );
        if !step.depends_on.is_empty() {
            let _ = writeln!(result, "\nBergantung pada {}.", step.depends_on.join(", "));
        }
        result.push_str("\nVerifikasi\n\n");
        for criterion in &step.acceptance_criteria {
            let _ = writeln!(result, "- {}", line(criterion));
        }
    }
    result
}

pub fn diagram(flow: &Flow) -> String {
    let mut output = String::from("flowchart TD\n");
    for (i, node) in flow.nodes.iter().enumerate() {
        let label = node
            .chars()
            .map(|character| {
                if character.is_alphanumeric() || " .,_-/()".contains(character) {
                    character
                } else {
                    ' '
                }
            })
            .collect::<String>();
        let _ = writeln!(output, "    N{i}[\"{}\"]", label.trim());
    }
    for [from, to] in &flow.edges {
        let _ = writeln!(output, "    N{from} --> N{to}");
    }
    output
}

fn xml(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn wireframe(view: &Wireframe) -> String {
    let width = if view.mobile { 360 } else { 760 };
    let height = 104 + view.elements.len() * 47;
    let title = xml(&view.title);
    let mut output = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" role="img" aria-labelledby="name">
  <title id="name">{title}</title>
  <rect x="1" y="1" width="{}" height="{}" rx="10" fill="#f8fafc" stroke="#334155"/>
  <text x="18" y="34" font-family="sans-serif" font-size="17" font-weight="600">{title}</text>
  <line x1="0" y1="52" x2="{width}" y2="52" stroke="#94a3b8"/>
"##,
        width - 2,
        height - 2
    );
    for (i, label) in view.elements.iter().enumerate() {
        let y = 69 + i * 47;
        let limit = if view.mobile { 38 } else { 87 };
        let mut chars = label.chars();
        let mut label: String = chars.by_ref().take(limit).collect();
        if chars.next().is_some() {
            label.push('…');
        }
        let _ = writeln!(
            output,
            r##"  <rect x="18" y="{y}" width="{}" height="33" rx="4" fill="#e2e8f0" stroke="#94a3b8"/>
  <text x="29" y="{}" font-family="sans-serif" font-size="13">{}</text>"##,
            width - 36,
            y + 21,
            xml(&label)
        );
    }
    output.push_str("</svg>\n");
    output
}

pub fn openapi(
    specification: &Value,
    number: i64,
    operations: Option<&[Operation]>,
) -> serde_yaml_ng::Result<String> {
    let mut paths = Map::new();
    for operation in operations.into_iter().flatten() {
        let path = paths.entry(&operation.path).or_insert_with(|| json!({}));
        let mut output = json!({
            "summary":operation.summary,
            "responses":{(operation.response_status.to_string()):{"description":operation.response_description}},
            "x-ngodik-requirements":operation.requirement_refs,
        });
        let params = extras::path_parameters(&operation.path).unwrap_or_default();
        if !params.is_empty() {
            output["parameters"] = Value::Array(
                params
                    .iter()
                    .map(|name| {
                        json!({
                            "name":name,"in":"path","required":true,"schema":{"type":"string"},
                        })
                    })
                    .collect(),
            );
        }
        if let Some(description) = &operation.request_description {
            output["requestBody"] =
                json!({"description":description,"content":{"application/json":{}}});
        }
        path[operation.method.name()] = output;
    }
    let mut result = json!({
        "openapi":"3.1.1",
        "info":{"title":text(specification,"title"),"version":format!("draft.{number}")},
        "paths":paths,
    });
    if operations.is_none() {
        result["info"]["description"] = json!(
            "Perlu ditinjau. Blueprint ini memerlukan API HTTP, tetapi operasi dan rutenya belum ditentukan."
        );
        result["x-ngodik-status"] = json!("Perlu ditinjau");
        result["x-ngodik-requirements"] = Value::Array(list(specification,"requirements").iter()
            .map(|requirement| json!({"id":requirement["id"],"statement":requirement["statement"]})).collect());
    }
    serde_yaml_ng::to_string(&result)
}

pub fn label(stage: Stage) -> &'static str {
    match stage {
        Stage::Product => "Kebutuhan produk",
        Stage::Analyst => "Analisis sistem",
        Stage::Ux => "Pengalaman pengguna",
        Stage::Architect => "Arsitektur solusi",
    }
}

pub fn checks(specification: &Value, report: Option<&qa::Report>, flags: &[String]) -> String {
    let mut result = String::from(
        "# Validasi dan QA\n\n## Pemeriksaan lokal\n\nSkema kanonis v1, dokumen yang berlaku, nama berkas, tautan internal, dan referensi kebutuhan diperiksa saat membuat paket.\n",
    );
    if !flags.is_empty() {
        result.push_str("\n## Perlu ditinjau\n\n");
        for flag in flags {
            let _ = writeln!(result, "- {}", line(flag));
        }
    }
    result.push_str("\n## Tinjauan semantik\n\n");
    match report {
        Some(report) if matches!(report.semantic_status, qa::SemanticStatus::Complete) => {
            if report.semantic.is_empty() {
                result.push_str("Tinjauan tidak menandai temuan.\n");
            }
            for finding in &report.semantic {
                let label = match finding.severity {
                    qa::Severity::Important => "Perlu ditinjau",
                    qa::Severity::Note => "Catatan",
                };
                let documents = finding
                    .documents
                    .iter()
                    .map(|stage| self::label(*stage))
                    .collect::<Vec<_>>()
                    .join(", ");
                let _ = writeln!(
                    result,
                    "- **{label}** {} ({}).",
                    line(&finding.message),
                    documents
                );
                let links = trace(&finding.requirement_refs);
                if !links.is_empty() {
                    let _ = writeln!(result, "  Kebutuhan terkait {links}.");
                }
                if let Some(suggestion) = &finding.suggestion {
                    let _ = writeln!(result, "  Usulan {}", line(suggestion));
                }
            }
        }
        _ => result.push_str(
            "Tinjauan model tidak tersedia. Periksa dokumen secara manual sebelum implementasi.\n",
        ),
    }
    result.push_str("\n## Skenario penerimaan\n\n");
    for requirement in list(specification, "requirements") {
        let link = trace([text(requirement, "id")]);
        for criterion in list(requirement, "acceptanceCriteria")
            .iter()
            .filter_map(Value::as_str)
        {
            let _ = writeln!(result, "- {link} · {}", line(criterion));
        }
    }
    result
}
