use tauri::{AppHandle, State};

use super::{
    DraftError, Drafts, Result, contains_key, key_for, lock, qa, spawn, storage, target, versions,
};
use crate::Store;

#[tauri::command]
pub fn revise_draft(
    app: AppHandle,
    store: State<'_, Store>,
    drafts: State<'_, Drafts>,
    project_id: i64,
    version_id: i64,
    profile_id: String,
    model_id: String,
    scope: String,
    instruction: String,
) -> Result<()> {
    let mut store = lock(&store)?;
    storage::require_no_proposal(&store, project_id)?;
    if versions::head(&store.connection, project_id)? != Some(version_id) {
        return Err(DraftError::Changed);
    }
    let base = versions::read(&store, &drafts.root, project_id, version_id)?;
    let instruction = instruction.trim();
    let scope_available = match scope.as_str() {
        "all" | "product" | "architect" => true,
        "analyst" => base.draft.documents()[0]["applicability"]["systemAnalysis"] == true,
        "ux" => base.draft.documents()[0]["applicability"]["uiUx"] == true,
        "review" => base.draft.qa.as_ref().is_some_and(|report| {
            matches!(report.semantic_status, qa::SemanticStatus::NotReviewed)
        }),
        _ => false,
    };
    if !scope_available || instruction.len() > 4096 || (scope != "review" && instruction.is_empty())
    {
        return Err(DraftError::InvalidRevision);
    }
    let target = target(&store, project_id)?.ok_or(DraftError::NoModel)?;
    if target.profile_id != profile_id || target.model_id != model_id {
        return Err(DraftError::Changed);
    }
    let (url, key) = key_for(&app, &target)?;
    if contains_key(instruction, &key)
        || contains_key(&base.idea, &key)
        || contains_key(&target.model_id, &key)
    {
        return Err(DraftError::ContainsKey);
    }
    let id = storage::create(
        &mut store,
        project_id,
        base.draft.idea_revision,
        &base.idea,
        &target,
        Some(version_id),
        Some((&scope, instruction)),
    )?;
    spawn(app, &drafts, storage::run(&store, id)?, url, key)
}

fn proposal(
    store: &crate::projects::ProjectStore,
    project_id: i64,
    run_id: i64,
) -> Result<storage::Run> {
    if storage::is_running(&store.connection, project_id)? {
        return Err(DraftError::Busy);
    }
    let run = storage::latest(store, project_id, false)?.ok_or(DraftError::NotFound)?;
    if run.id != run_id || !storage::is_proposal(&run) {
        return Err(DraftError::Changed);
    }
    Ok(run)
}

#[tauri::command]
pub fn decide_proposal(
    store: State<'_, Store>,
    drafts: State<'_, Drafts>,
    project_id: i64,
    run_id: i64,
    apply: bool,
) -> Result<()> {
    let mut store = lock(&store)?;
    let run = proposal(&store, project_id, run_id)?;
    if !apply {
        store.connection.execute(
            "UPDATE draft_runs SET review_decision='dismissed' WHERE id=?1",
            [run.id],
        )?;
        return Ok(());
    }
    let head = versions::head(&store.connection, project_id)?;
    if head != run.base_version {
        return Err(DraftError::Changed);
    }
    let note = match run.revision_scope.as_deref() {
        Some("review") => "Tinjauan draf lama",
        Some(_) => "Revisi disetujui",
        None if head.is_none() => "Draf pertama",
        None => "Draf baru disetujui",
    };
    let idea = run.idea.clone();
    let draft = storage::show(&store, &drafts.root, run)?;
    if !qa::deterministic(&draft.documents()).is_empty() {
        return Err(DraftError::InvalidDraft);
    }
    versions::publish(
        &mut store,
        &drafts.root,
        project_id,
        head,
        draft,
        idea,
        note,
        None,
    )
}

#[tauri::command]
pub fn restore_blueprint(
    store: State<'_, Store>,
    drafts: State<'_, Drafts>,
    project_id: i64,
    version_id: i64,
    expected_head: i64,
) -> Result<()> {
    let mut store = lock(&store)?;
    storage::require_no_proposal(&store, project_id)?;
    if storage::is_running(&store.connection, project_id)? {
        return Err(DraftError::Busy);
    }
    if versions::head(&store.connection, project_id)? != Some(expected_head)
        || version_id == expected_head
    {
        return Err(DraftError::Changed);
    }
    let backup = versions::read(&store, &drafts.root, project_id, version_id)?;
    let summary = format!("Dipulihkan dari versi {}", backup.version.number);
    versions::publish(
        &mut store,
        &drafts.root,
        project_id,
        Some(expected_head),
        backup.draft,
        backup.idea,
        &summary,
        Some(version_id),
    )
}
