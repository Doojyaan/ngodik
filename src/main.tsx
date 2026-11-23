import { useEffect, useRef, useState, type FormEvent } from "react";
import { createRoot } from "react-dom/client";
import ngodikAppIcon from "../assets/branding/ngodik-app-icon.svg";
import { invoke } from "@tauri-apps/api/core";
import { Workspace, type Project } from "./workspace";
import { ModelSettings } from "./model-settings";
import { stages } from "./draft-panel";
import { useButtonHoverMotion, useNavigationMotion } from "./navigation-motion";
import { ProjectActions, ProjectActionDialog, type ProjectAction } from "./project-actions";
import "./styles.css";

type Page = "projects" | "new" | "settings" | "workspace";
type ProjectSummary = { id: number; title: string; updatedAt: number; draftStatus: string | null; completedStages: number };
const dates = new Intl.DateTimeFormat("id-ID", { dateStyle: "medium", timeStyle: "short" });
const projectStatuses: Record<string, string> = {
  pending_review: "Menunggu tinjauan", completed: "Draf selesai", running: "Menyusun draf", failed: "Perlu dicoba lagi", canceled: "Dibatalkan", interrupted: "Dapat dilanjutkan",
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
  const [projectAction, setProjectAction] = useState<ProjectAction>();
  const saveBeforeLeaving = useRef<(() => Promise<boolean>) | null>(null);
  const pageSurface = useRef<HTMLDivElement>(null);
  useNavigationMotion(pageSurface, page, "main h1");
  useButtonHoverMotion(page);
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
        <button className="brand-mark" aria-label="ngodik · Proyek" title="Proyek" disabled={busy} onClick={() => void navigate("projects")}><img src={ngodikAppIcon} alt="" /></button>
        <nav aria-label="Navigasi utama">
          <div className="rail-pages">
            <button className="rail-button" title="Proyek" aria-label="Proyek" aria-current={page === "projects" || page === "workspace" ? "page" : undefined}
              disabled={busy} onClick={() => void navigate("projects")}><span className="grid-icon" aria-hidden="true"><i /><i /><i /><i /></span></button>
            <button className="rail-button" title="Proyek baru" aria-label="Proyek baru" aria-current={page === "new" ? "page" : undefined}
              disabled={busy} onClick={() => void navigate("new")}><span aria-hidden="true">+</span></button>
          </div>
          <div className="rail-utilities">
            <button className="rail-button" title="Setelan" aria-label="Setelan" aria-current={page === "settings" ? "page" : undefined}
              disabled={busy} onClick={() => void navigate("settings")}><svg className="settings-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.65"
                strokeLinejoin="round" aria-hidden="true" focusable="false">
                <polygon points="8.43,5.81 9.79,5.20 10.23,2.87 13.77,2.87 14.21,5.20 15.58,5.81 16.78,6.69 19.02,5.90 20.79,8.97 18.99,10.51 19.15,12.00 18.99,13.49 20.79,15.03 19.02,18.10 16.78,17.31 15.58,18.19 14.21,18.80 13.77,21.13 10.23,21.13 9.79,18.80 8.43,18.19 7.22,17.31 4.98,18.10 3.21,15.03 5.01,13.49 4.85,12.00 5.01,10.51 3.21,8.97 4.98,5.90 7.22,6.69" />
                <circle cx="12" cy="12" r="3.05" />
              </svg></button>
            <button type="button" className="rail-button" title="Akun lokal" aria-label="Akun lokal" popoverTarget="local-profile">
              <span className="profile-icon" aria-hidden="true" />
            </button>
          </div>
        </nav>
        <section id="local-profile" className="profile-popover" popover="auto" aria-labelledby="local-profile-title">
          <h2 id="local-profile-title">Profil lokal</h2>
          <p>Proyek dan setelan tersimpan di perangkat ini.</p>
          <button type="button" onClick={(event) => {
            event.currentTarget.closest<HTMLElement>("[popover]")?.hidePopover();
            void navigate("settings");
          }}>Buka setelan</button>
        </section>
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
          {!loading && projects.length === 0 && !error && <section className="empty-home" aria-labelledby="empty-projects">
            <span className="file-mark" aria-hidden="true" />
            <div>
              <h2 id="empty-projects">Belum ada proyek</h2>
              <p>Mulai dengan ide singkat. Draf blueprint dan riwayatnya tersimpan di perangkat ini.</p>
            </div>
          </section>}
          {projects.length > 0 && <section className="project-register" aria-label="Daftar proyek">
            <div className="project-columns" aria-hidden="true">
              <span>Nama proyek</span><span>Status draf</span><span>Progres</span><span>Terakhir disunting</span>
            </div>
            <ul>
              {projects.map(({ id, title, updatedAt, draftStatus, completedStages }) => <li key={id} className="project-entry">
                <button className="project-row" disabled={busy} onClick={() => void enter(id)}>
                  <span className="project-name"><span className="file-mark" aria-hidden="true" /><strong>{title}</strong></span>
                  <span className="project-state"><span className="step-dot" data-status={draftStatus} aria-hidden="true" />
                    <span>{projectStatuses[draftStatus ?? ""] ?? "Belum ada draf"}</span></span>
                  <span className="project-count">{completedStages} / {stages.length} tahap</span>
                  <time dateTime={new Date(updatedAt).toISOString()}>{dates.format(updatedAt)}</time>
                </button>
                <ProjectActions id={id} title={title} disabled={busy} onChoose={setProjectAction} />
              </li>)}
            </ul>
          </section>}
        </main>}
      </div>
      {projectAction && <ProjectActionDialog action={projectAction} onSaved={refresh}
        onClose={() => setProjectAction(undefined)} />}
    </>
  );
}

createRoot(document.getElementById("root")!).render(<App />);
