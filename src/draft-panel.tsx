import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type Target = { profileId: string; baseUrl: string; modelId: string };
type Requirement = {
  id: string; kind: "functional" | "nonfunctional"; statement: string; acceptanceCriteria: string[]; source: string;
};
type Finding = { text: string; source: string };
type Specification = {
  title: string; summary: string; problem: string; audience: string; goals: string[]; inScope: string[]; outOfScope: string[];
  requirements: Requirement[]; decisions: Finding[]; assumptions: Finding[]; openQuestions: Finding[];
};
type Document = { sections: { title: string; body: string; requirementRefs: string[] }[] };
type Step = {
  stage: "product" | "analyst" | "ux" | "architect";
  status: "waiting" | "running" | "completed" | "failed" | "canceled" | "interrupted";
  attempts: number;
  error: string | null;
  output: Specification | Document | null;
};
type Run = { id: number; ideaRevision: number; target: Target; status: Step["status"]; error: string | null; stages: Step[] };
type Snapshot = { target: Target | null; current: Run | null; previous: Run | null };

const stepLabels: Record<Step["stage"], string> = {
  product: "Produk dan kebutuhan", analyst: "Analisis sistem", ux: "Pengalaman pengguna", architect: "Arsitektur solusi"
};
const statuses: Record<Step["status"], string> = {
  waiting: "Menunggu", running: "Sedang diproses", completed: "Selesai", failed: "Gagal",
  canceled: "Dibatalkan", interrupted: "Terhenti"
};
const errors: Record<string, string> = {
  busy: "Proyek ini sedang menyusun draf.",
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

export function DraftPanel({ projectId, hasIdea, prepare, configurationVersion }: {
  projectId: number; hasIdea: boolean; prepare: () => Promise<number | undefined>; configurationVersion: number;
}) {
  const [snapshot, setSnapshot] = useState<Snapshot>();
  const [problem, setProblem] = useState("");
  const [action, setAction] = useState("");
  const current = snapshot?.current;
  const target = snapshot?.target;
  const running = current?.status === "running";
  const canResume = Boolean(current && !running && current.status !== "completed");

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
    try { await operation(); await refresh(); }
    catch (error) { setProblem(explain(error)); await refresh().catch(() => {}); }
    finally { setAction(""); }
  }

  return <section className="editor draft-panel" aria-labelledby="draft-title">
    <h2 id="draft-title">Draf blueprint</h2>
    {target ? <p>Untuk membuat draf, ngodik mengirim ide dan hasil tahap yang dibutuhkan ke <strong className="profile-url">{target.baseUrl}</strong> menggunakan <strong>{target.modelId}</strong>. Provider dapat menyimpan data dan mengenakan biaya sesuai kebijakannya.</p>
      : <p>Pilih dan simpan satu model di panel Model proyek untuk membuat draf.</p>}

    {canResume && current && <p>Melanjutkan memakai ide dan tujuan pengiriman proses sebelumnya, yaitu <strong>{current.target.modelId}</strong> di <strong className="profile-url">{current.target.baseUrl}</strong>.</p>}
    <div className="settings-actions">
      {running && current ? <button disabled={Boolean(action)} onClick={() => void run("Membatalkan…", () =>
        invoke("cancel_draft", { projectId, runId: current.id }))}>Batalkan proses</button> : <>
        {canResume && current && <button className="primary" disabled={Boolean(action)} onClick={() => void run("Melanjutkan…", () =>
          invoke("resume_draft", { projectId, runId: current.id }))}>{current.status === "failed" ? "Coba lagi" : "Lanjutkan draf"}</button>}
        <button className={canResume ? undefined : "primary"} disabled={Boolean(action) || !target || !hasIdea}
          onClick={() => void run("Menyiapkan draf…", async () => {
            if (!target) return;
            const revision = await prepare();
            if (revision === undefined) return;
            await invoke("start_draft", { projectId, revision, profileId: target.profileId, modelId: target.modelId });
          })}>{current ? "Buat draf baru" : "Buat draf"}</button>
      </>}
    </div>
    <p role="status">{action || (current ? statuses[current.status] : "")}</p>
    {problem && <p className="error" role="alert">{problem}</p>}
    {current && <>
      <ol className="draft-stages" aria-label="Tahap penyusunan">
        {current.stages.map(({ stage, status, error, attempts }) => <li key={stage}>
          <strong>{stepLabels[stage]}</strong>
          <span>{statuses[status]}{attempts > 1 ? ` · ${attempts} percobaan` : ""}</span>
          {error && <small className="error">{explain(error)}</small>}
        </li>)}
      </ol>
      {current.error && <p className="error">{explain(current.error)}</p>}
      <Results run={current} />
    </>}
    {snapshot?.previous && <details className="draft-previous">
      <summary>Draf selesai sebelumnya</summary>
      <Results run={snapshot.previous} />
    </details>}
  </section>;
}
