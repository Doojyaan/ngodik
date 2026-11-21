import { useState, type FormEvent } from "react";

export const reviewLabels = {
  product: "Produk dan kebutuhan",
  analyst: "Analisis sistem",
  ux: "Pengalaman pengguna",
  architect: "Arsitektur solusi",
};

type Stage = keyof typeof reviewLabels;
type Scope = "all" | "review" | Stage;
export type Target = { profileId: string; baseUrl: string; modelId: string };

type Finding = {
  severity: "important" | "note"; message: string; suggestion: string | null;
  requirementRefs: string[]; documents: Stage[];
};

export type Report = {
  deterministic: Finding[]; semantic: Finding[]; semanticStatus: "complete" | "unavailable" | "not_reviewed";
};

export type Proposal = {
  runId: number; changes: { stage: Stage; field: string; before: string | null; after: string | null }[];
};

export type Version = {
  id: number; number: number; parentId: number | null; restoredFrom: number | null;
  createdAt: number; summary: string; recoverable: boolean;
};

const fieldLabels: Record<string, string> = {
  title: "Judul", summary: "Ringkasan", problem: "Masalah", audience: "Pengguna", goals: "Tujuan",
  inScope: "Dalam cakupan", outOfScope: "Di luar cakupan", decisions: "Keputusan", assumptions: "Asumsi",
  openQuestions: "Pertanyaan terbuka", applicability: "Dokumen yang berlaku", sections: "Bagian dokumen",
};

const date = new Intl.DateTimeFormat("id-ID", { dateStyle: "medium", timeStyle: "short" });

export function needsReview(report?: Report | null) {
  return Boolean(report && (report.deterministic.length > 0 || report.semanticStatus !== "complete"
    || report.semantic.some(({ severity }) => severity === "important")));
}

type ReviewProps = {
  report?: Report | null; proposal?: Proposal | null; target?: Target | null; hasVersion: boolean;
  scopes: Stage[]; action: string; running: boolean; problem: string;
  revise: (scope: Scope, note: string) => Promise<boolean>;
  decide: (apply: boolean) => void;
  openArtifacts: () => void;
  openDraft: () => void;
  openModel: () => void;
};

export function ReviewPane({ report, proposal, target, hasVersion, scopes, action, running, problem,
  revise, decide, openArtifacts, openDraft, openModel }: ReviewProps) {
  const [note, setNote] = useState("");
  const [scope, setScope] = useState<Exclude<Scope, "review">>("all");
  const awaiting = Boolean(proposal);
  const currentScope = scope !== "all" && !scopes.includes(scope) ? "all" : scope;
  const canRevise = hasVersion && !awaiting && !running;
  const noteTooLong = new TextEncoder().encode(note).length > 4096;

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (await revise(currentScope, note)) setNote("");
  }

  function findingList(findings: Finding[]) {
    return <ul className="review-findings">{findings.map((finding, index) => <li key={index}>
      <p><strong>{finding.severity === "important" ? "Perlu ditinjau" : "Catatan"}</strong> {finding.message}</p>
      <p className="meta">{finding.documents.map((stage) => reviewLabels[stage]).join(", ")}
        {finding.requirementRefs.length > 0 && ` · ${finding.requirementRefs.join(", ")}`}</p>
      {finding.suggestion && <div>
        <p>{finding.suggestion}</p>
        {canRevise && <button type="button" onClick={() => {
          setScope("all");
          setNote(finding.suggestion ?? "");
          document.querySelector<HTMLTextAreaElement>("#revision-note")?.focus();
        }}>Gunakan saran</button>}
      </div>}
    </li>)}</ul>;
  }

  return <section className="editor review-panel" aria-labelledby="review-title">
    <header className="section-heading heading-row">
      <h2 id="review-title">{proposal ? "Tinjau usulan revisi" : "Review draf"}</h2>
      {report && <span className="badge" data-status={needsReview(report) ? "interrupted" : "completed"}>
        {needsReview(report) ? "Perlu ditinjau" : "Pemeriksaan selesai"}
      </span>}
    </header>

    {running && <p>Draf sedang diproses. <button type="button" onClick={openDraft}>Lihat progres</button></p>}
    {!report && !running && <p>Hasil review tersedia setelah draf disusun.</p>}
    {report && <div className="review-groups">
      <section>
        <h3>Pemeriksaan struktur dan referensi</h3>
        {report.deterministic.length ? findingList(report.deterministic) : <p>Struktur dan referensi lulus pemeriksaan.</p>}
      </section>
      <section>
        <h3>Tinjauan semantik</h3>
        {report.semanticStatus === "complete" ? (report.semantic.length ? findingList(report.semantic) : <p>Model tidak menandai konflik.</p>)
          : <p>{report.semanticStatus === "not_reviewed" ? "Draf lama ini belum ditinjau oleh model."
            : "Tinjauan model tidak tersedia untuk draf ini. Periksa dokumen secara manual."}</p>}
      </section>
    </div>}

    {proposal && <section className="review-section">
      <h3>Ringkasan perubahan</h3>
      {proposal.changes.length > 0 ? <div className="review-changes">
        <p>{proposal.changes.length} bagian berubah. Buka bagian untuk melihat perbandingan.</p>
        {proposal.changes.map(({ stage, field, before, after }) => <details key={stage + field}>
          <summary>{reviewLabels[stage]} · {fieldLabels[field] ?? field}</summary>
          <div className="change-pair">
            <div><h4>Sebelum</h4><pre>{before ?? "Belum ada bagian ini"}</pre></div>
            <div><h4>Sesudah</h4><pre>{after ?? "Bagian dihapus"}</pre></div>
          </div>
        </details>)}
      </div> : <p>Tidak ada perubahan isi dibandingkan draf aktif.</p>}
      <div className="settings-actions">
        <button type="button" disabled={Boolean(action)} onClick={openArtifacts}>Buka artefak</button>
        <button type="button" disabled={Boolean(action)} onClick={() => decide(false)}>Buang usulan</button>
        <button className="primary" type="button" disabled={Boolean(action) || Boolean(report?.deterministic.length)} onClick={() => decide(true)}>Terapkan usulan</button>
      </div>
    </section>}

    {canRevise && <section className="review-section">
      <h3>Ajukan revisi</h3>
      {target ? <p>Instruksi, draf, dan bagian terkait dikirim ke <strong>{target.modelId}</strong> di <span className="profile-url">{target.baseUrl}</span>. Perubahan akan tampil sebagai usulan.</p>
        : <p>Pilih model proyek untuk membuat revisi. <button type="button" onClick={openModel}>Pilih model</button></p>}
      {report?.semanticStatus === "not_reviewed" && target && <button type="button" disabled={Boolean(action)} onClick={() => void revise("review", "")}>Tinjau draf lama</button>}
      <form className="revision-form" onSubmit={(event) => void submit(event)}>
        <label htmlFor="revision-scope">Cakupan revisi</label>
        <select id="revision-scope" value={currentScope} disabled={Boolean(action)} onChange={(event) => setScope(event.target.value as typeof scope)}>
          <option value="all">Seluruh blueprint</option>
          {scopes.map((item) => <option key={item} value={item}>{reviewLabels[item]}</option>)}
        </select>
        <label htmlFor="revision-note">Apa yang perlu diubah?</label>
        <textarea id="revision-note" value={note} onChange={(event) => setNote(event.target.value)}
          placeholder="Contoh: Tambahkan impor CSV untuk daftar barang." disabled={Boolean(action)} aria-invalid={noteTooLong || undefined} />
        {noteTooLong && <p className="error">Persingkat instruksi menjadi maksimal 4 KiB.</p>}
        <button className="primary" disabled={!target || !note.trim() || noteTooLong || Boolean(action)}>Buat usulan revisi</button>
      </form>
    </section>}
    <p role="status">{action}</p>
    {problem && <p className="error" role="alert">{problem}</p>}
  </section>;
}

type HistoryProps = {
  versions: Version[]; warning: boolean; running: boolean; pending: boolean; action: string; problem: string;
  restore: (id: number) => void;
};

export function BlueprintHistory({ versions, warning, running, pending, action, problem, restore }: HistoryProps) {
  const [selected, setSelected] = useState<Version>();
  const [undo, setUndo] = useState(false);
  const latest = versions[0];
  const parent = versions.find(({ id, recoverable }) => id === latest?.parentId && recoverable);
  const busy = running || pending || Boolean(action);
  return <section className="editor history version-panel" aria-labelledby="version-title">
    <header className="section-heading heading-row">
      <h2 id="version-title">Versi blueprint</h2>
      {parent && <button disabled={busy} onClick={() => { setSelected(parent); setUndo(true); }}>Urungkan perubahan terakhir</button>}
    </header>
    {warning && <p className="error">Snapshot versi aktif tidak bisa dibaca. Pilih titik pemulihan yang masih tersedia.</p>}
    {pending && <p>Selesaikan usulan di tab Review sebelum memulihkan versi.</p>}
    {versions.length === 0 ? <p>Versi pertama muncul setelah draf selesai.</p> : <>
      <p>ngodik menyimpan hingga 10 titik pemulihan. Pemulihan membuat versi aktif baru.</p>
      <ol>{versions.map((version) => <li key={version.id} className="version-row">
        <div><strong>Versi {version.number}</strong> {latest?.id === version.id && <span className="badge" data-status="completed">Aktif</span>}
          <p>{version.summary}</p>
          <time dateTime={new Date(version.createdAt).toISOString()}>{date.format(version.createdAt)}</time>
        </div>
        {latest?.id !== version.id && (version.recoverable
          ? <button disabled={busy} onClick={() => { setSelected(version); setUndo(false); }}>Pulihkan versi {version.number}</button>
          : <span className="meta">Titik pemulihan habis</span>)}
      </li>)}</ol>
      {selected && <div className="review-section">
        <p>{undo ? "Urungkan perubahan" : "Pulihkan draf"} ke versi {selected.number}?</p>
        <div className="settings-actions">
          <button disabled={Boolean(action)} onClick={() => setSelected(undefined)}>Batal</button>
          <button className="primary" disabled={busy} onClick={() => {
            restore(selected.id);
            setSelected(undefined);
          }}>Konfirmasi pemulihan</button>
        </div>
      </div>}
    </>}
    <p role="status">{action}</p>
    {problem && <p className="error" role="alert">{problem}</p>}
  </section>;
}
