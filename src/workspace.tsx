import { useCallback, useEffect, useRef, useState, type RefObject } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { DraftPanel } from "./draft-panel";
import { ProjectModel } from "./model-settings";
import { useNavigationMotion } from "./navigation-motion";

type Checkpoint = { revision: number; savedAt: number };
export type Project = { id: number; title: string; idea: string; revision: number; history: Checkpoint[] };

const date = new Intl.DateTimeFormat("id-ID", { dateStyle: "medium", timeStyle: "medium" });
const limit = 16 * 1024;

type Panel = "draft" | "artifacts" | "idea" | "model" | "history";
const panels: { id: Panel; label: string }[] = [
  { id: "draft", label: "Draf blueprint" },
  { id: "artifacts", label: "Artefak" },
  { id: "idea", label: "Ide proyek" },
  { id: "model", label: "Model proyek" },
  { id: "history", label: "Riwayat simpan" },
];

export function Workspace({ initial, back, saveBeforeLeaving }: {
  initial: Project; back: () => void; saveBeforeLeaving: RefObject<(() => Promise<boolean>) | null>;
}) {
  const [project, setProject] = useState(initial);
  const [idea, setIdea] = useState(initial.idea);
  const [status, setStatus] = useState("Tersimpan di perangkat");
  const [error, setError] = useState("");
  const [configurationVersion, setConfigurationVersion] = useState(0);
  const [panel, setPanel] = useState<Panel>(initial.idea.trim() ? "draft" : "idea");
  const panelSurface = useRef<HTMLDivElement>(null);
  useNavigationMotion(panelSurface, panel);
  const current = useRef(initial);
  const latest = useRef(initial.idea);
  const active = useRef<Promise<boolean> | null>(null);


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

  useEffect(() => {
    saveBeforeLeaving.current = save;
    return () => { if (saveBeforeLeaving.current === save) saveBeforeLeaving.current = null; };
  }, [save, saveBeforeLeaving]);
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

  return (
    <main className="workspace project-workspace">
      <header className="workspace-heading">
        <button className="back" onClick={back}>← Semua proyek</button>
        <h1 tabIndex={-1}>{project.title}</h1>
        <p className="meta">Workspace proyek</p>
      </header>
      <div className="project-layout">
        <nav className="section-nav" aria-label="Bagian proyek">
          {panels.map(({ id, label }) => <button key={id} aria-current={panel === id ? "page" : undefined}
            onClick={() => setPanel(id)}>{label}</button>)}
        </nav>
        <div className="project-detail" ref={panelSurface}>
          <div hidden={panel !== "draft" && panel !== "artifacts"}>
            <DraftPanel projectId={project.id} hasIdea={Boolean(idea.trim())} configurationVersion={configurationVersion}
              view={panel === "artifacts" ? "artifacts" : "draft"} openModel={() => setPanel("model")}
              openIdea={() => setPanel("idea")} openArtifacts={() => setPanel("artifacts")}
              prepare={async () => await save() ? current.current.revision : undefined} />
          </div>
          <section className="editor" aria-labelledby="idea-label" hidden={panel !== "idea"}>
            <label id="idea-label" htmlFor="idea">Ide proyek</label>
            <p id="idea-hint">Apa yang ingin Anda buat, siapa yang akan memakainya, dan apa yang perlu bisa dilakukan?</p>
            <textarea id="idea" aria-describedby={error ? "idea-hint save-error" : "idea-hint"} value={idea}
              onChange={(event) => {
                latest.current = event.target.value;
                setIdea(event.target.value);
                setStatus("Perubahan belum tersimpan");
                setError("");
              }} />
            <div className="editor-footer">
              <span role="status">{status}</span>
              <div className="button-group">
                <button onClick={() => { void save().then((saved) => { if (saved) setPanel("draft"); }); }}>Ke draf</button>
                <button className="primary" onClick={() => void save()}>Simpan ide</button>
              </div>
            </div>
          </section>
          {error && <p id="save-error" className="error" role="alert">{error}</p>}
          <section className="editor history" aria-labelledby="history-heading" hidden={panel !== "history"}>
            <h2 id="history-heading">Riwayat simpan</h2>
            <ol>{project.history.map(({ revision, savedAt }) => (
              <li key={revision}>
                <span>{revision === 0 ? "Proyek dibuat" : `Versi ${revision}`}</span>
                <time dateTime={new Date(savedAt).toISOString()}>{date.format(savedAt)}</time>
              </li>
            ))}</ol>
          </section>
          <div hidden={panel !== "model"}>
            <ProjectModel projectId={project.id} onSaved={() => setConfigurationVersion((version) => version + 1)} />
          </div>
        </div>
      </div>
    </main>
  );
}
