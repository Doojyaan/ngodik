import { useEffect, useRef, useState, type FormEvent } from "react";
import { createRoot } from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { Workspace, type Project } from "./workspace";
import { ModelSettings } from "./model-settings";
import { stages } from "./draft-panel";
import { useNavigationMotion } from "./navigation-motion";
import "./styles.css";

type Page = "projects" | "new" | "settings" | "workspace";
type ProjectSummary = { id: number; title: string; updatedAt: number; draftStatus: string | null; completedStages: number };
const dates = new Intl.DateTimeFormat("id-ID", { dateStyle: "medium", timeStyle: "short" });
const projectStatuses: Record<string, string> = {
  completed: "Draf selesai", running: "Menyusun draf", failed: "Perlu dicoba lagi", canceled: "Dibatalkan", interrupted: "Dapat dilanjutkan",
};

function App() {
  const [page, setPage] = useState<Page>("projects");
  const [projects, setProjects] = useState<ProjectSummary[]>([]);
  const [project, setProject] = useState<Project>();
  const [title, setTitle] = useState("");
  const [idea, setIdea] = useState("");
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const saveBeforeLeaving = useRef<(() => Promise<boolean>) | null>(null);
  const pageSurface = useRef<HTMLDivElement>(null);
  useNavigationMotion(pageSurface, page, "main h1");
  const ideaBytes = new TextEncoder().encode(idea).length;
  const ideaLimit = 16 * 1024;

  async function refresh() {
    setError("");
    try {
      setProjects(await invoke<ProjectSummary[]>("list_projects"));
    } catch {
      setError("Daftar proyek gagal dimuat.");
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => { if (page === "projects") void refresh(); }, [page]);
  useEffect(() => {
    document.querySelector<HTMLElement>("main h1")?.focus({ preventScroll: true });
    window.scrollTo(0, 0);
  }, [page]);

  async function navigate(next: Exclude<Page, "workspace">) {
    if (busy || next === page) return;
    setBusy(true);
    try {
      if (saveBeforeLeaving.current && !await saveBeforeLeaving.current()) return;
      setError("");
      setProject(undefined);
      setPage(next);
    } finally {
      setBusy(false);
    }
  }

  async function enter(id?: number) {
    setBusy(true);
    setError("");
    try {
      const next = id === undefined
        ? await invoke<Project>("create_project", { title, idea })
        : await invoke<Project>("open_project", { id });
      setProject(next);
      setTitle("");
      setIdea("");
      setPage("workspace");
    } catch (problem) {
      setError(problem === "invalid_title" ? "Masukkan nama proyek, maksimal 100 karakter."
        : problem === "idea_too_long" ? "Ide terlalu panjang. Persingkat teks lalu coba lagi."
        : "Proyek gagal dibuka atau disimpan. Coba lagi.");
    } finally {
      setBusy(false);
    }
  }

  function create(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    void enter();
  }

  return (
    <>
      <aside className="app-rail" aria-label="ngodik">
        <button className="brand-mark" aria-label="ngodik · Proyek" title="Proyek" disabled={busy} onClick={() => void navigate("projects")}>N</button>
        <nav aria-label="Navigasi utama">
          <button className="rail-button" title="Proyek" aria-label="Proyek" aria-current={page === "projects" || page === "workspace" ? "page" : undefined}
            disabled={busy} onClick={() => void navigate("projects")}><span className="grid-icon" aria-hidden="true"><i /><i /><i /><i /></span></button>
          <button className="rail-button" title="Proyek baru" aria-label="Proyek baru" aria-current={page === "new" ? "page" : undefined}
            disabled={busy} onClick={() => void navigate("new")}><span aria-hidden="true">+</span></button>
          <button className="rail-button" title="Setelan" aria-label="Setelan" aria-current={page === "settings" ? "page" : undefined}
            disabled={busy} onClick={() => void navigate("settings")}><span className="settings-icon" aria-hidden="true" /></button>
        </nav>
      </aside>
      <div ref={pageSurface}>
      {page === "settings" ? <ModelSettings back={() => void navigate("projects")} />
        : page === "workspace" && project ? <Workspace key={project.id} initial={project}
          saveBeforeLeaving={saveBeforeLeaving} back={() => void navigate("projects")} />
        : page === "new" ? <main className="workspace new-project">
          <header className="workspace-heading">
            <button className="back" onClick={() => void navigate("projects")}>← Semua proyek</button>
            <h1 tabIndex={-1}>Proyek baru</h1>
            <p>Simpan ide, lalu pilih model untuk menyusun draf.</p>
          </header>
          <div className="workspace-grid">
            <form className="editor new-form" onSubmit={create} aria-busy={busy}>
              <label htmlFor="title">Nama proyek</label>
              <input id="title" value={title} onChange={(event) => setTitle(event.target.value)}
                placeholder="Contoh: Pencatat stok toko" maxLength={100} required />
              <label htmlFor="new-idea">Ide proyek</label>
              <textarea id="new-idea" value={idea} onChange={(event) => setIdea(event.target.value)}
                placeholder="Ceritakan apa yang ingin dibuat, siapa yang memakainya, dan hal utama yang perlu bisa dilakukan."
                aria-describedby="idea-size" />
              <footer className="editor-footer">
                <span id="idea-size" className={ideaBytes > ideaLimit ? "error" : "meta"}>{(ideaBytes / 1024).toFixed(1)} / 16 KiB</span>
                <div className="button-group">
                  <button type="button" disabled={busy} onClick={() => void navigate("projects")}>Kembali</button>
                  <button className="primary" disabled={busy || !title.trim() || ideaBytes > ideaLimit}>Buat proyek</button>
                </div>
              </footer>
              {error && <p className="error" role="alert">{error}</p>}
            </form>
            <aside className="editor project-guide" aria-labelledby="project-guide">
              <h2 id="project-guide">Yang akan disusun</h2>
              <ol>{stages.map(({ label, name }, index) => <li key={name}>
                <span className="step-number" aria-hidden="true">{String(index + 1).padStart(2, "0")}</span>
                <span>{label}</span>
              </li>)}</ol>
              <p>Anda akan melihat asumsi dan pertanyaan terbuka di draf awal.</p>
            </aside>
          </div>
        </main> : <main className="workspace home">
          <header className="workspace-heading heading-row">
            <div>
              <h1 tabIndex={-1}>Proyek</h1>
              <p className="meta">{loading ? "Memuat proyek…" : `${projects.length} proyek lokal`}</p>
            </div>
            <button className="primary" disabled={busy} onClick={() => void navigate("new")}>Proyek baru</button>
          </header>
          {error && <div className="notice error" role="alert">{error} <button onClick={() => void refresh()}>Muat ulang</button></div>}
          {!loading && projects.length === 0 && !error && <section className="empty-home">
            <div><h2>Dari ide ke blueprint.</h2><p>Tulis ide Anda. ngodik menyusun dokumen proyek yang bisa Anda tinjau dan serahkan ke coding agent.</p></div>
            <div className="editor"><h3>Proyek pertama</h3><p>Proyek tersimpan di perangkat ini.</p>
              <button className="primary" onClick={() => void navigate("new")}>Buat proyek pertama</button></div>
          </section>}
          {projects.length > 0 && <>
            <ul className="project-cards" aria-label="Proyek tersimpan">
              {projects.map(({ id, title, updatedAt, draftStatus, completedStages }) => <li key={id}>
                <button className="project-card" disabled={busy} onClick={() => void enter(id)}>
                  <strong>{title}</strong>
                  <span className="project-progress" aria-hidden="true">
                    {stages.map(({ name }, index) => <i key={name} data-complete={index < completedStages} data-finished={draftStatus === "completed"} />)}
                  </span>
                  <span className="card-meta"><span className="meta">{completedStages} dari {stages.length} tahap</span>
                    <span className={draftStatus === "completed" ? "success" : "accent"}>{projectStatuses[draftStatus ?? ""] ?? "Belum ada draf"}</span></span>
                  <time dateTime={new Date(updatedAt).toISOString()}>{dates.format(updatedAt)}</time>
                </button>
              </li>)}
            </ul>
            <section className="activity" aria-labelledby="recent-heading">
              <h2 id="recent-heading">Terakhir disunting</h2>
              <ul>{projects.slice(0, 5).map(({ id, title, updatedAt }) => <li key={id}>
                <time dateTime={new Date(updatedAt).toISOString()}>{dates.format(updatedAt)}</time>
                <button disabled={busy} onClick={() => void enter(id)}>{title}</button>
              </li>)}</ul>
            </section>
          </>}
        </main>}
      </div>
    </>
  );
}

createRoot(document.getElementById("root")!).render(<App />);
