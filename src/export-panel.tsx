import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type Preview = { versionId: number; versionNumber: number; suggestedName: string; documents: string[]; issueCount: number };
type Saved = { path: string; documentCount: number; issueCount: number };

const errors: Record<string, string> = {
  missing_version: "Selesaikan atau terapkan versi blueprint sebelum mengekspor.",
  version_changed: "Versi aktif berubah. Muat ulang ringkasan sebelum menyimpan.",
  invalid_blueprint: "Struktur atau snapshot blueprint tidak bisa dibaca. Tinjau atau pulihkan versi yang valid.",
  invalid_package: "Pemeriksaan paket menemukan dokumen atau tautan yang tidak valid.",
  contains_credential: "Teks yang cocok dengan kredensial terdeteksi. Tinjau dokumen sebelum mengekspor.",
  credential_check: "Pemeriksaan kredensial Windows tidak tersedia. Coba lagi.",
  invalid_location: "Pilih nama berakhiran .zip dan tujuan yang tersedia.",
  write_failed: "ZIP gagal ditulis. Periksa izin dan ruang di lokasi tujuan, lalu coba lagi.",
  clipboard_unavailable: "Clipboard tidak tersedia. Coba salin lagi.",
};

export function ExportPanel({ projectId }: { projectId: number }) {
  const [preview, setPreview] = useState<Preview>();
  const [problem, setProblem] = useState("");
  const [busy, setBusy] = useState("");
  const [notice, setNotice] = useState("");
  const [saved, setSaved] = useState<Saved>();

  function failed(error: unknown) { setProblem(errors[String(error)] ?? "Terjadi masalah saat menyiapkan ekspor."); }

  async function refresh() {
    setBusy("Memeriksa blueprint…");
    setProblem("");
    setNotice("");
    setSaved(undefined);
    try { setPreview(await invoke<Preview>("export_preview", { projectId })); }
    catch (error) { setPreview(undefined); failed(error); }
    finally { setBusy(""); }
  }

  useEffect(() => { void refresh(); }, [projectId]);

  async function save() {
    if (!preview) return;
    setBusy("Menunggu lokasi dari dialog Windows…");
    setProblem("");
    setNotice("");
    setSaved(undefined);
    try {
      const output = await invoke<Saved | null>("save_blueprint", { projectId, versionId: preview.versionId });
      if (output) {
        setSaved(output);
        setNotice(`${output.documentCount} dokumen tersimpan.`);
      } else {
        setNotice("Penyimpanan dibatalkan.");
      }
    } catch (error) { failed(error); }
    finally { setBusy(""); }
  }

  async function copy() {
    setBusy("Menyalin prompt…");
    setProblem("");
    setNotice("");
    try {
      await invoke("copy_handoff");
      setNotice("Prompt handoff disalin.");
    } catch (error) { failed(error); }
    finally { setBusy(""); }
  }

  return <section className="editor export-panel" aria-labelledby="export-title">
    <header className="section-heading heading-row">
      <h2 id="export-title">Ekspor blueprint</h2>
      {preview && <span className="badge" data-status={preview.issueCount ? "interrupted" : "completed"}>
        {preview.issueCount ? "Perlu ditinjau" : "Siap diekspor"}
      </span>}
    </header>
    {preview && <>
      <p>Versi aktif {preview.versionNumber} · {preview.documents.length} dokumen</p>
      {preview.issueCount > 0 && <p>{preview.issueCount} hal perlu ditinjau. Rinciannya ikut disimpan di README dan dokumen QA.</p>}
      <details className="export-documents" open>
        <summary>Isi paket</summary>
        <ul>{preview.documents.map((file) => <li key={file}>{file}</li>)}</ul>
      </details>
      <div className="settings-actions">
        <button className="primary" disabled={Boolean(busy)} onClick={() => void save()}>Simpan ZIP…</button>
        <button disabled={Boolean(busy)} onClick={() => void copy()}>Salin prompt</button>
      </div>
      <p>Ekstrak ZIP sebelum memberikannya ke coding agent. Prompt disalin secara terpisah.</p>
    </>}
    {problem && <p className="error" role="alert">{problem} <button disabled={Boolean(busy)} onClick={() => void refresh()}>Muat ulang</button></p>}
    <p role="status">{busy || notice}</p>
    {saved && <div className="export-result">
      <strong>Lokasi ZIP</strong>
      <p>{saved.path}</p>
    </div>}
  </section>;
}
