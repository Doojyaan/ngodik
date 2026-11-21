import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import { BlueprintHistory, ReviewPane, needsReview, reviewLabels, type Proposal, type Report, type Target, type Version } from "./draft-review";
type Requirement = {
  id: string; kind: "functional" | "nonfunctional"; statement: string; acceptanceCriteria: string[]; source: string;
};
type Finding = { text: string; source: string };
type Specification = {
  title: string; summary: string; problem: string; audience: string; goals: string[]; inScope: string[]; outOfScope: string[];
  requirements: Requirement[]; decisions: Finding[]; assumptions: Finding[]; openQuestions: Finding[];
  applicability: { systemAnalysis: boolean; uiUx: boolean; httpApi: boolean };
};
type Document = { sections: { title: string; body: string; requirementRefs: string[] }[] };
type Step = {
  stage: "product" | "analyst" | "ux" | "architect";
  status: "waiting" | "running" | "completed" | "failed" | "canceled" | "interrupted";
  attempts: number;
  error: string | null;
  output: Specification | Document | null;
};
type Run = { id: number; ideaRevision: number; target: Target; status: Step["status"]; error: string | null; stages: Step[]; qa: Report | null };
type Snapshot = {
  target: Target | null; current: Run | null; published: { version: Version; draft: Run } | null;
  proposal: Proposal | null; versions: Version[]; recoveryWarning: boolean;
};

const stepLabels = reviewLabels;
const filenames: Record<Step["stage"], string> = {
  product: "PROJECT_SPECIFICATION.json",
  analyst: "SYSTEM_ANALYSIS.json",
  ux: "UX_SPECIFICATION.json",
  architect: "SOLUTION_ARCHITECTURE.json",
};
export const stages = (Object.keys(stepLabels) as Step["stage"][]).map((name) => ({ name, label: stepLabels[name] }));
const statuses: Record<Step["status"], string> = {
  waiting: "Menunggu", running: "Sedang diproses", completed: "Selesai", failed: "Gagal",
  canceled: "Dibatalkan", interrupted: "Terhenti"
};
const errors: Record<string, string> = {
  busy: "Proyek ini sedang menyusun draf.",
  invalid_revision: "Periksa cakupan dan panjang instruksi revisi.",
  pending_review: "Terapkan atau buang usulan yang masih menunggu di tab Review.",
  recovery_unavailable: "Titik pemulihan ini tidak tersedia.",
  checkpoint_unreadable: "Hasil tahap yang tersimpan tidak dapat dibaca. Anda masih bisa membuat draf baru.",
  no_model: "Simpan satu model di panel Model proyek sebelum membuat draf.",
  missing_key: "Tambahkan API key pada profil melalui Pengaturan model.",
  empty_idea: "Tulis ide proyek terlebih dahulu.",
  changed: "Ide atau pilihan model berubah. Periksa tujuan pengiriman lalu coba lagi.",
  invalid_response: "Respons model tidak sesuai format. Periksa kompatibilitas atau coba model lain.",
  invalid_draft: "Hasil tahap ini tidak memenuhi struktur draf atau merujuk ke kebutuhan yang tidak ada.",
  context_too_large: "Konteks tahap ini melebihi batas lokal. Ringkas ide atau buat draf baru.",
  contains_key: "Teks yang cocok dengan API key terdeteksi. Periksa ide dan ID model sebelum mencoba lagi.",
  timeout: "Endpoint tidak merespons tepat waktu.",
  unauthorized: "Endpoint menolak API key. Periksa key dan izin model.",
  rate_limited: "Batas permintaan provider tercapai. Coba lagi setelah beberapa saat.",
  endpoint_unavailable: "Endpoint sedang bermasalah.",
  network_unavailable: "Koneksi ke endpoint gagal.",
  redirect_blocked: "Endpoint mengalihkan permintaan. Gunakan alamat tujuan yang benar.",
  request_rejected: "Provider menolak permintaan. Periksa model dan batas konteksnya.",
  not_found: "Draf atau profil yang dipakai sudah tidak tersedia.",
};
const explain = (error: unknown) => errors[String(error)] ?? "Terjadi masalah saat membaca atau menyimpan draf.";

function Results({ run }: { run: Run }) {
  return <div className="draft-results">{run.stages.map(({ stage, output }) => {
    if (!output) return null;
    if ("sections" in output) {
      return output.sections.length ? <details key={stage}>
        <summary>{stepLabels[stage]}</summary>
        {output.sections.map((section, index) => <div key={index}>
          <h4>{section.title}</h4>
          <p className="draft-text">{section.body}</p>
          {section.requirementRefs.length > 0 && <p>Kebutuhan terkait {section.requirementRefs.join(", ")}</p>}
        </div>)}
      </details> : null;
    }
    return <article key={stage} className="draft-overview">
      <h3>{output.title}</h3>
      <p>{output.summary}</p>
      {([["Keputusan", output.decisions], ["Asumsi", output.assumptions],
        ["Pertanyaan terbuka", output.openQuestions]] as const).map(([label, findings]) => findings.length ? (
        <section key={label}>
          <h4>{label}</h4>
          <ul>{findings.map((item, index) => <li key={index}>{item.text} <small>Sumber {item.source}</small></li>)}</ul>
        </section>
      ) : null)}
      <details>
        <summary>Spesifikasi proyek</summary>
        <p>{output.problem}</p>
        <p><strong>Pengguna</strong> {output.audience}</p>
        {([["Tujuan", output.goals], ["Dalam cakupan", output.inScope], ["Di luar cakupan", output.outOfScope]] as const)
          .map(([label, items]) => items.length > 0 && <section key={label}>
            <h4>{label}</h4><ul>{items.map((item, index) => <li key={index}>{item}</li>)}</ul>
          </section>)}
        <h4>Kebutuhan dan kriteria penerimaan</h4>
        <ul>{output.requirements.map((item) => <li key={item.id}>
          <strong>{item.id}</strong> {item.statement}
          <ul>{item.acceptanceCriteria.map((criterion, index) => <li key={index}>{criterion}</li>)}</ul>
          <small>{item.kind === "functional" ? "Fungsional" : "Nonfungsional"}. Sumber {item.source}</small>
        </li>)}</ul>
      </details>
    </article>;
  })}</div>;
}

function ArtifactReader({ run }: { run: Run | null | undefined }) {
  const [selected, setSelected] = useState<Step["stage"]>("product");
  const documents = run?.stages.filter(({ output }) => output && (!("sections" in output) || output.sections.length > 0)) ?? [];
  const active = documents.find(({ stage }) => stage === selected) ?? documents[0];
  const text = active ? JSON.stringify(active.output, null, 2) : "";
  const lines = text.split("\n");
  return <section className="artifact-view" aria-labelledby="artifact-title">
    <header className="section-heading">
      <h2 id="artifact-title">Artefak</h2>
      <p className="meta">{documents.length} dokumen tersedia{run ? ` · Draf ${run.id}` : ""}</p>
    </header>
    {active ? <div className="artifact-layout">
      <nav className="editor artifact-list" aria-label="Dokumen draf">
        {documents.map(({ stage, output }) => <button key={stage} aria-current={active.stage === stage ? "true" : undefined}
          onClick={() => setSelected(stage)}>
          <span className="artifact-dot" aria-hidden="true" />
          <span>{filenames[stage]}</span>
          <span className="meta">{(new TextEncoder().encode(JSON.stringify(output)).length / 1024).toFixed(1)} KiB</span>
        </button>)}
      </nav>
      <article className="editor artifact-reader" aria-label={filenames[active.stage]}>
        <header className="reader-bar"><strong>{filenames[active.stage]}</strong><span className="badge">JSON</span></header>
        <pre tabIndex={0} aria-label="Isi dokumen JSON"><code>
          {lines.map((line, index) => <span className="code-line" key={index}>
            <span className="line-no" aria-hidden="true">{index + 1}</span>
            <span>{line.split(/("(?:[^"\\]|\\.)*")/g).map((part, token) => token % 2
              ? <span className="code-string" key={token}>{part}</span> : part)}{"\n"}</span>
          </span>)}
        </code></pre>
      </article>
    </div> : <div className="editor"><p>Dokumen muncul di sini setelah tahap penyusunan menghasilkan draf.</p></div>}
  </section>;
}

export function DraftPanel({ projectId, hasIdea, prepare, configurationVersion, view, openModel, openIdea, openArtifacts, openDraft, openReview }: {
  projectId: number; hasIdea: boolean; prepare: () => Promise<number | undefined>; configurationVersion: number;
  view: "draft" | "artifacts" | "review" | "versions"; openModel: () => void; openIdea: () => void; openArtifacts: () => void;
  openDraft: () => void; openReview: () => void;
}) {
  const [snapshot, setSnapshot] = useState<Snapshot>();
  const [problem, setProblem] = useState("");
  const [action, setAction] = useState("");
  const current = snapshot?.current;
  const target = snapshot?.target;
  const active = snapshot?.published;
  const proposal = snapshot?.proposal;
  const versions = snapshot?.versions ?? [];
  const [artifactChoice, setArtifactChoice] = useState<number | null>(null);
  const running = current?.status === "running";
  const canResume = Boolean(current && ["failed", "canceled", "interrupted"].includes(current.status));
  const display = active?.draft ?? current;
  const progress = running || canResume || !active ? current : active.draft;

  const refresh = useCallback(async () => {
    setSnapshot(await invoke<Snapshot>("draft_status", { projectId }));
  }, [projectId]);

  useEffect(() => { void refresh().catch((error) => setProblem(explain(error))); }, [refresh, configurationVersion]);
  useEffect(() => {
    if (!running) return;
    let disposed = false;
    let next: number;
    async function poll() {
      try { if (!disposed) await refresh(); }
      catch (error) { if (!disposed) setProblem(explain(error)); }
      if (!disposed) next = window.setTimeout(poll, 600);
    }
    next = window.setTimeout(poll, 600);
    return () => { disposed = true; window.clearTimeout(next); };
  }, [refresh, running]);

  async function run(label: string, operation: () => Promise<void>) {
    setProblem("");
    setAction(label);
    try { await operation(); await refresh(); return true; }
    catch (error) { setProblem(explain(error)); await refresh().catch(() => {}); return false; }
    finally { setAction(""); }
  }

  if (view === "artifacts") {
    const candidateSelected = Boolean(proposal && artifactChoice === proposal.runId);
    const artifactDraft = candidateSelected ? current : display;
    return <>
      {proposal && active && <div className="settings-actions artifact-switch" aria-label="Jenis artefak">
        <button aria-pressed={!candidateSelected} onClick={() => setArtifactChoice(null)}>Draf aktif</button>
        <button aria-pressed={candidateSelected} onClick={() => setArtifactChoice(proposal.runId)}>Usulan revisi</button>
      </div>}
      <ArtifactReader key={artifactDraft?.id} run={artifactDraft} />
    </>;
  }

  if (view === "review") {
    const report = proposal ? current?.qa : active?.draft.qa ?? current?.qa;
    const specification = active?.draft.stages.find(({ stage }) => stage === "product")?.output;
    const scopes: Step["stage"][] = ["product"];
    if (specification && "applicability" in specification) {
      if (specification.applicability.systemAnalysis) scopes.push("analyst");
      if (specification.applicability.uiUx) scopes.push("ux");
    }
    scopes.push("architect");
    return <ReviewPane report={report} proposal={proposal} target={target} hasVersion={Boolean(active)}
      scopes={scopes} action={action} running={running} problem={problem} openArtifacts={openArtifacts}
      openDraft={openDraft} openModel={openModel}
      revise={async (scope, instruction) => {
        if (!target || !active) return false;
        const ok = await run("Menyiapkan revisi…", () => invoke("revise_draft", {
          projectId, versionId: active.version.id, scope, instruction, profileId: target.profileId, modelId: target.modelId,
        }));
        if (ok) openDraft();
        return ok;
      }}
      decide={(apply) => {
        if (proposal) void run("Menyimpan keputusan…", () => invoke("decide_proposal", { projectId, runId: proposal.runId, apply }));
      }} />;
  }

  if (view === "versions") {
    return <BlueprintHistory versions={versions} warning={Boolean(snapshot?.recoveryWarning)} running={running}
      pending={Boolean(proposal)} action={action} problem={problem}
      restore={(versionId) => {
        const expectedHead = versions[0]?.id;
        if (expectedHead) void run("Memulihkan versi…", () => invoke("restore_blueprint", { projectId, versionId, expectedHead }));
      }} />;
  }

  return <section className="editor draft-panel" aria-labelledby="draft-title">
    <header className="section-heading heading-row">
      <h2 id="draft-title">Draf blueprint</h2>
      <span className="badge" data-status={proposal || needsReview(active?.draft.qa) ? "interrupted" : progress?.status}>
        {proposal ? "Usulan menunggu persetujuan" : (running || canResume) && progress ? statuses[progress.status] : needsReview(active?.draft.qa)
          ? "Perlu ditinjau" : progress ? statuses[progress.status] : "Belum ada draf"}
      </span>
    </header>
    {target ? <div className="draft-destination">
      <p><strong>{target.modelId}</strong> <span className="profile-url">{target.baseUrl}</span></p>
      <p>Ide dan hasil tahap terkait dikirim ke endpoint ini. Provider dapat menyimpan data dan mengenakan biaya.</p>
    </div>
      : <p>Pilih dan simpan satu model untuk membuat draf.</p>}

    {canResume && current && <p>Melanjutkan memakai ide dan tujuan pengiriman proses sebelumnya, yaitu <strong>{current.target.modelId}</strong> di <strong className="profile-url">{current.target.baseUrl}</strong>.</p>}
    <div className="settings-actions">
      {!target && <button onClick={openModel}>Pilih model</button>}
      {!hasIdea && <button onClick={openIdea}>Tulis ide</button>}
      {display?.stages.some(({ output }) => Boolean(output)) && <button onClick={openArtifacts}>Buka artefak</button>}
      {(proposal || active?.draft.qa) && <button className={proposal ? "primary" : undefined} onClick={openReview}>
        {proposal ? "Bandingkan usulan" : "Buka review"}
      </button>}
      {running && current ? <button disabled={Boolean(action)} onClick={() => void run("Membatalkan…", () =>
        invoke("cancel_draft", { projectId, runId: current.id }))}>Batalkan proses</button> : <>
        {canResume && current && <button className="primary" disabled={Boolean(action)} onClick={() => void run("Melanjutkan…", () =>
          invoke("resume_draft", { projectId, runId: current.id }))}>{current.status === "failed" ? "Coba lagi" : "Lanjutkan draf"}</button>}
        {!proposal && <button className={canResume ? undefined : "primary"} disabled={Boolean(action) || !target || !hasIdea}
          onClick={() => void run("Menyiapkan draf…", async () => {
            if (!target) return;
            const revision = await prepare();
            if (revision === undefined) return;
            await invoke("start_draft", { projectId, revision, profileId: target.profileId, modelId: target.modelId });
          })}>{current || active ? "Buat draf baru" : "Buat draf"}</button>}
      </>}
    </div>
    <p className={action ? "draft-action" : "visually-hidden"} role="status">{action || (current ? statuses[current.status] : "")}</p>
    {problem && <p className="error" role="alert">{problem}</p>}
    {progress && <div className="draft-layout">
      <ol className="draft-stages" aria-label="Tahap penyusunan">
        {progress.stages.map(({ stage, status, error, attempts }) => <li key={stage}>
          <strong><span className="step-dot" data-status={status} aria-hidden="true" />{stepLabels[stage]}</strong>
          <span>{statuses[status]}{attempts > 1 ? ` · ${attempts} percobaan` : ""}</span>
          {error && <small className="error">{explain(error)}</small>}
        </li>)}
      </ol>
      <div className="draft-reading">
        {current?.error && <p className="error">{explain(current.error)}</p>}
        {running && current?.stages.every(({ status }) => status === "completed") && <p role="status">Meninjau draf…</p>}
        {display && <Results run={display} />}
      </div>
    </div>}
  </section>;
}
