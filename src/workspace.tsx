import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { DraftPanel } from "./draft-panel";
import { ProjectModel } from "./model-settings";

type Checkpoint = { revision: number; savedAt: number };
export type Project = { id: number; title: string; idea: string; revision: number; history: Checkpoint[] };

const date = new Intl.DateTimeFormat("id-ID", { dateStyle: "medium", timeStyle: "medium" });
const limit = 16 * 1024;

export function Workspace({ initial, back }: { initial: Project; back: () => void }) {
  const [project, setProject] = useState(initial);
  const [idea, setIdea] = useState(initial.idea);
  const [status, setStatus] = useState("Tersimpan di perangkat");
  const [error, setError] = useState("");
  const [configurationVersion, setConfigurationVersion] = useState(0);
  const [leaving, setLeaving] = useState(false);
  const current = useRef(initial);
  const latest = useRef(initial.idea);
  const active = useRef<Promise<boolean> | null>(null);
  const title = useRef<HTMLHeadingElement>(null);

  const save = useCallback(() => {
    if (active.current) return active.current;
    if (latest.current === current.current.idea) {
      setStatus("Tersimpan di perangkat");
      return Promise.resolve(true);
    }

    active.current = (async () => {
      setError("");
      setStatus("Menyimpan…");
      try {
        while (latest.current !== current.current.idea) {
          const value = latest.current;
          if (new TextEncoder().encode(value).length > limit) throw "idea_too_long";
          const { id, revision } = current.current;
          const saved = await invoke<Project>("save_idea", { id, revision, idea: value });
          current.current = saved;
          setProject(saved);
        }
        setStatus("Tersimpan di perangkat");
        return true;
      } catch (problem) {
        setStatus("Perubahan belum tersimpan");
        setError(problem === "idea_too_long"
          ? "Ide melebihi batas 16 KiB. Persingkat teks lalu coba lagi."
          : problem === "changed_elsewhere"
            ? "Versi tersimpan berubah di jendela lain. Salin teks Anda sebelum membuka ulang proyek."
            : "Penyimpanan gagal. Coba simpan lagi.");
        return false;
      }
    })().finally(() => { active.current = null; });
    return active.current;
  }, []);

  useEffect(() => { title.current?.focus(); }, []);
  useEffect(() => {
    const timer = window.setTimeout(() => void save(), 500);
    return () => window.clearTimeout(timer);
  }, [idea, save]);

  useEffect(() => {
    const windowHandle = getCurrentWindow();
    const listener = windowHandle.onCloseRequested(async (event) => {
      event.preventDefault();
      if (await save()) await windowHandle.destroy();
    });
    void listener.catch(() => {
      setError("Penyimpanan saat menutup jendela tidak tersedia. Simpan sebelum menutup.");
    });
    return () => { void listener.then((unlisten) => unlisten()).catch(() => undefined); };
  }, [save]);

  async function returnHome() {
    setLeaving(true);
    if (await save()) back();
    else setLeaving(false);
  }

  return (
    <main className="workspace">
      <div className="workspace-heading">
        <button className="back" disabled={leaving} onClick={() => void returnHome()}>← Proyek saya</button>
        <h1 ref={title} tabIndex={-1}>{project.title}</h1>
      </div>
      <div className="workspace-grid">
        <section className="editor" aria-labelledby="idea-label">
          <label id="idea-label" htmlFor="idea">Ide proyek</label>
          <p id="idea-hint">Apa yang ingin Anda buat, siapa yang akan memakainya, dan apa yang perlu bisa dilakukan?</p>
          <textarea id="idea" aria-describedby={error ? "idea-hint save-error" : "idea-hint"} value={idea} disabled={leaving} onChange={(event) => {
            latest.current = event.target.value;
            setIdea(event.target.value);
            setStatus("Perubahan belum tersimpan");
            setError("");
          }} />
          <div className="editor-footer">
            <span role="status">{status}</span>
            <button className="primary" disabled={leaving} onClick={() => void save()}>Simpan ide</button>
          </div>
          {error && <p id="save-error" className="error" role="alert">{error}</p>}
        </section>
        <aside className="history">
          <h2>Riwayat simpan</h2>
          <ol>{project.history.map(({ revision, savedAt }) => (
            <li key={revision}>
              <span>{revision === 0 ? "Proyek dibuat" : `Versi ${revision}`}</span>
              <time dateTime={new Date(savedAt).toISOString()}>{date.format(savedAt)}</time>
            </li>
          ))}</ol>
        </aside>
      </div>
      <ProjectModel projectId={project.id} onSaved={() => setConfigurationVersion((version) => version + 1)} />
      <DraftPanel projectId={project.id} hasIdea={Boolean(idea.trim())} configurationVersion={configurationVersion}
        prepare={async () => await save() ? current.current.revision : undefined} />
    </main>
  );
}
