import { useEffect, useState, type FormEvent } from "react";
import { createRoot } from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { Workspace, type Project } from "./workspace";
import { ModelSettings } from "./model-settings";
import "./styles.css";

type ProjectSummary = { id: number; title: string; updatedAt: number };
const dates = new Intl.DateTimeFormat("id-ID", { dateStyle: "medium", timeStyle: "short" });

function App() {
  const [settings, setSettings] = useState(false);
  const [projects, setProjects] = useState<ProjectSummary[]>([]);
  const [project, setProject] = useState<Project>();
  const [title, setTitle] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  async function refresh() {
    setError("");
    try {
      setProjects(await invoke<ProjectSummary[]>("list_projects"));
    } catch {
      setError("Daftar proyek gagal dimuat. Coba lagi.");
    }
  }

  useEffect(() => { void refresh(); }, []);

  async function enter(id?: number) {
    setBusy(true);
    setError("");
    try {
      const next = id === undefined
        ? await invoke<Project>("create_project", { title })
        : await invoke<Project>("open_project", { id });
      setProject(next);
      setTitle("");
    } catch (problem) {
      setError(problem === "invalid_title"
        ? "Masukkan nama proyek, maksimal 100 karakter."
        : "Proyek gagal dibuka atau dibuat. Coba lagi.");
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
      <header className="masthead"><span className="brand-mark" aria-hidden="true">▤</span> ngodik {!project && !settings && <button className="settings-link" onClick={() => setSettings(true)}>Pengaturan model</button>}</header>
      {settings ? <ModelSettings back={() => setSettings(false)} /> : project ? <Workspace key={project.id} initial={project} back={() => {
        setProject(undefined);
        void refresh();
      }} /> : <main className="home">
        <section className="start">
          <h1>Proyek saya</h1>
          <p>Beri nama proyek, lalu ceritakan ide Anda dengan kata-kata sendiri.</p>
          <form onSubmit={create}>
            <label htmlFor="title">Nama proyek</label>
            <div className="create-row">
              <input id="title" value={title} onChange={(event) => setTitle(event.target.value)}
                placeholder="Contoh: Pencatat stok toko" maxLength={100} required />
              <button className="primary" disabled={busy || !title.trim()}>Buat proyek</button>
            </div>
          </form>
        </section>
        <section className="recent" aria-labelledby="recent-heading">
          <h2 id="recent-heading">Proyek terbaru</h2>
          {error && <p className="error" role="alert">{error} <button onClick={() => void refresh()}>Muat ulang</button></p>}
          {projects.length === 0 && !error && <p>Proyek yang sudah dibuat akan muncul di sini.</p>}
          <ul>{projects.map(({ id, title, updatedAt }) => (
            <li key={id}><button disabled={busy} onClick={() => void enter(id)}>
              <span>{title}</span><time dateTime={new Date(updatedAt).toISOString()}>{dates.format(updatedAt)}</time>
            </button></li>
          ))}</ul>
        </section>
      </main>}
    </>
  );
}

createRoot(document.getElementById("root")!).render(<App />);
