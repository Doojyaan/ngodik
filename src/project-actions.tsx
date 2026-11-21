import { useEffect, useRef, useState, type FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";

export type ProjectAction = {
  id: number;
  title: string;
  kind: "rename" | "delete";
  trigger: HTMLButtonElement;
};

export function ProjectActions({ id, title, disabled, onChoose }: {
  id: number;
  title: string;
  disabled: boolean;
  onChoose: (action: ProjectAction) => void;
}) {
  const trigger = useRef<HTMLButtonElement>(null);
  const popover = useRef<HTMLDivElement>(null);
  const menuId = `project-actions-${id}`;

  function position() {
    const bounds = trigger.current!.getBoundingClientRect();
    const menu = popover.current!;
    menu.style.right = `${document.documentElement.clientWidth - bounds.right}px`;
    const below = window.innerHeight - bounds.bottom >= 112;
    menu.style.top = below ? `${bounds.bottom + 4}px` : "auto";
    menu.style.bottom = below ? "auto" : `${window.innerHeight - bounds.top + 4}px`;
  }

  function choose(kind: ProjectAction["kind"]) {
    popover.current!.hidePopover();
    onChoose({ id, title, kind, trigger: trigger.current! });
  }

  return <>
    <button ref={trigger} className="project-more" type="button" title="Opsi proyek"
      aria-label={`Opsi proyek ${title}`} popoverTarget={menuId} disabled={disabled} onClick={position}>
      <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor" aria-hidden="true">
        <circle cx="5" cy="12" r="1.7" /><circle cx="12" cy="12" r="1.7" /><circle cx="19" cy="12" r="1.7" />
      </svg>
    </button>
    <div ref={popover} id={menuId} className="project-options" popover="auto">
      <button type="button" onClick={() => choose("rename")}>Ubah nama</button>
      <button type="button" className="danger" onClick={() => choose("delete")}>Hapus proyek</button>
    </div>
  </>;
}

export function ProjectActionDialog({ action, onSaved, onClose }: {
  action: ProjectAction;
  onSaved: () => Promise<void>;
  onClose: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const initialFocus = useRef<HTMLInputElement | HTMLButtonElement>(null);
  const [title, setTitle] = useState(action.title);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const deleting = action.kind === "delete";

  useEffect(() => {
    dialog.current!.showModal();
    initialFocus.current?.focus();
    if (initialFocus.current instanceof HTMLInputElement) initialFocus.current.select();
  }, []);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (busy) return;
    setBusy(true);
    setError("");
    try {
      if (deleting) await invoke("delete_project", { id: action.id });
      else await invoke("rename_project", { id: action.id, title });
      await onSaved();
      dialog.current!.close();
    } catch (problem) {
      setError(problem === "draft_running" ? "Draf masih disusun. Selesaikan atau batalkan draf sebelum menghapus proyek."
        : problem === "invalid_title" ? "Masukkan nama proyek, maksimal 100 karakter."
        : "Perubahan gagal disimpan. Coba lagi.");
    } finally {
      setBusy(false);
    }
  }

  function closed() {
    onClose();
    requestAnimationFrame(() => {
      const target = action.trigger.isConnected ? action.trigger : document.querySelector<HTMLElement>("main h1");
      target?.focus({ preventScroll: true });
    });
  }

  return <dialog ref={dialog} className="editor project-action-dialog" aria-labelledby="project-action-title"
    aria-describedby={deleting ? "project-action-description" : undefined} onClose={closed}
    onCancel={event => { if (busy) event.preventDefault(); }}>
    <form onSubmit={submit} aria-busy={busy}>
      <h2 id="project-action-title">{deleting ? "Hapus proyek?" : "Ubah nama proyek"}</h2>
      {deleting ? <p id="project-action-description">
        Proyek <strong>{action.title}</strong> beserta ide, draf, dan riwayatnya akan dihapus permanen.
        ZIP yang disimpan di luar folder data proyek tetap ada.
      </p> : <>
        <label htmlFor="project-action-name">Nama proyek</label>
        <input ref={node => { initialFocus.current = node; }} id="project-action-name" value={title}
          onChange={event => setTitle(event.target.value)} maxLength={100} required disabled={busy}
          aria-describedby={error ? "project-action-error" : undefined} />
      </>}
      {error && <p id="project-action-error" className="error" role="alert">{error}</p>}
      <div className="button-group">
        <button type="button" ref={deleting ? node => { initialFocus.current = node; } : undefined}
          disabled={busy} onClick={() => dialog.current!.close()}>Batal</button>
        <button className={deleting ? "danger" : "primary"} disabled={busy || (!deleting && !title.trim())}>
          {busy ? "Memproses…" : deleting ? "Hapus proyek" : "Simpan nama"}
        </button>
      </div>
    </form>
  </dialog>;
}
