import { useEffect, useState, type FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";

type Profile = { id: string; label: string; baseUrl: string; hasKey: boolean };
type Selection = { profileId: string; modelId: string };

const errors: Record<string, string> = {
  invalid_label: "Isi nama profil dengan maksimal 80 karakter.",
  invalid_endpoint: "Gunakan URL HTTPS tanpa kredensial, parameter, atau fragmen. HTTP hanya untuk alamat di perangkat ini.",
  endpoint_locked: "Buat profil baru untuk memakai alamat endpoint lain.",
  invalid_model: "Isi ID model dengan maksimal 256 byte tanpa spasi.",
  missing_key: "Tambahkan API key pada profil ini terlebih dahulu.",
  invalid_key: "API key kosong atau mengandung karakter yang tidak dapat dipakai.",
  unauthorized: "Endpoint menolak API key. Periksa key dan izin model Anda.",
  timeout: "Endpoint tidak merespons tepat waktu. Coba lagi.",
  unavailable_route: "Endpoint tidak menyediakan operasi ini. Untuk daftar model, Anda bisa memasukkan ID secara manual.",
  endpoint_unavailable: "Endpoint sedang bermasalah. Coba lagi.",
  redirect_blocked: "Endpoint mengalihkan permintaan. Buat profil dengan alamat API tujuan yang benar.",
  invalid_response: "Respons endpoint tidak sesuai. Periksa alamat dan kompatibilitas Chat Completions.",
  rate_limited: "Batas permintaan provider tercapai. Coba lagi nanti.",
  request_rejected: "Provider menolak permintaan. Periksa ID model dan izin penggunaannya.",
  credential_unavailable: "Windows Credential Manager tidak dapat diakses.",
  network_unavailable: "Koneksi ke endpoint gagal. Periksa alamat, jaringan, atau sertifikat.",
};
const errorText = (error: unknown) => errors[String(error)] ?? "Perubahan gagal. Coba lagi.";

function ModelField({ id, value, models, change, disabled }: {
  id: string; value: string; models: string[]; change: (value: string) => void; disabled: boolean;
}) {
  return <>
    <label htmlFor={id}>ID model</label>
    <input id={id} list={id + "-options"} maxLength={256} placeholder="Masukkan ID model"
      value={value} onChange={(event) => change(event.target.value)} disabled={disabled} />
    <datalist id={id + "-options"}>{models.map((model) => <option key={model} value={model} />)}</datalist>
  </>;
}

export function ModelSettings({ back }: { back: () => void }) {
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [selected, setSelected] = useState<Profile>();
  const [label, setLabel] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const [model, setModel] = useState("");
  const [models, setModels] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState("");
  const [notice, setNotice] = useState("");
  const [confirmDelete, setConfirmDelete] = useState(false);

  async function refresh() {
    const items = await invoke<Profile[]>("list_profiles");
    setProfiles(items);
    return items;
  }

  useEffect(() => {
    void refresh().catch((error) => setProblem(errorText(error)));
  }, []);

  function select(profile?: Profile) {
    setSelected(profile);
    setLabel(profile?.label ?? "");
    setBaseUrl(profile?.baseUrl ?? "");
    setModel("");
    setModels([]);
    setConfirmDelete(false);
    setProblem("");
    setNotice("");
  }

  async function run(work: () => Promise<void>) {
    setBusy(true);
    setProblem("");
    setNotice("");
    try { await work(); }
    catch (error) { setProblem(errorText(error)); }
    finally { setBusy(false); }
  }

  function save(event: FormEvent) {
    event.preventDefault();
    void run(async () => {
      const profile = await invoke<Profile>("save_profile", { id: selected?.id ?? null, label, baseUrl });
      setSelected(profile);
      setLabel(profile.label);
      setBaseUrl(profile.baseUrl);
      await refresh();
      setNotice("Profil tersimpan.");
    });
  }

  return <main className="workspace">
    <div className="workspace-heading">
      <button className="back" disabled={busy} onClick={back}>← Proyek saya</button>
      <h1 tabIndex={-1}>Setelan</h1>
      <p>Kelola profil endpoint dan API key untuk proyek di perangkat ini.</p>
    </div>
    <div className="workspace-grid settings-grid">
      <section className="editor settings" aria-label="Detail profil" aria-busy={busy}>
        <h2>{selected ? "Profil koneksi" : "Profil baru"}</h2>
        <form onSubmit={save} className="settings-form">
          <label htmlFor="profile-label">Nama profil</label>
          <input id="profile-label" value={label} maxLength={80} required disabled={busy}
            onChange={(event) => setLabel(event.target.value)} />
          <label htmlFor="profile-url">Alamat API</label>
          <input id="profile-url" value={baseUrl} maxLength={500} placeholder="https://api.example.com/v1"
            type="url" required readOnly={Boolean(selected)} disabled={busy}
            onChange={(event) => setBaseUrl(event.target.value)} />
          <button className="primary" disabled={busy}>{selected ? "Simpan nama" : "Buat profil"}</button>
        </form>
        {selected && <>
          <section className="settings-section" aria-labelledby="credential-heading">
            <h2 id="credential-heading">API key</h2>
            <p>{selected.hasKey ? "Tersimpan di Windows Credential Manager." : "Belum ditambahkan."}</p>
            <button disabled={busy} onClick={() => void run(async () => {
              const changed = await invoke<boolean>("set_profile_key", { id: selected.id });
              if (changed) {
                const items = await refresh();
                setSelected(items.find((item) => item.id === selected.id));
                setNotice("API key tersimpan.");
              }
            })}>{selected.hasKey ? "Ganti API key" : "Masukkan API key"}</button>
          </section>
          <section className="settings-section" aria-labelledby="connection-heading">
            <h2 id="connection-heading">Uji koneksi</h2>
            <p>Uji memakai pesan singkat ke alamat API di atas. Provider dapat mengenakan biaya.</p>
            <ModelField id="test-model" value={model} models={models} change={setModel} disabled={busy} />
            <div className="settings-actions">
              <button disabled={busy || !selected.hasKey} onClick={() => void run(async () => {
                setModels([]);
                const ids = await invoke<string[]>("sync_models", { id: selected.id });
                setModels(ids);
                setNotice(ids.length ? "Daftar model dimuat. Pilih atau ketik ID model." : "Daftar kosong. Masukkan ID model secara manual.");
              })}>Muat daftar model</button>
              <button disabled={busy || !selected.hasKey || !model.trim()} onClick={() => void run(async () => {
                await invoke("test_connection", { id: selected.id, modelId: model });
                setNotice("Uji koneksi berhasil.");
              })}>Uji koneksi</button>
            </div>
          </section>
          <section className="settings-section" aria-label="Hapus profil">
            {confirmDelete ? <>
              <p>Pilihan model pada proyek yang memakai profil ini juga akan dilepas.</p>
              <div className="settings-actions">
                <button className="danger" disabled={busy} onClick={() => void run(async () => {
                  await invoke("delete_profile", { id: selected.id });
                  select();
                  await refresh();
                  setNotice("Profil dihapus.");
                })}>Ya, hapus profil</button>
                <button disabled={busy} onClick={() => setConfirmDelete(false)}>Batal</button>
              </div>
            </> : <button disabled={busy} onClick={() => setConfirmDelete(true)}>Hapus profil…</button>}
          </section>
        </>}
        <div role="status">{busy ? "Memproses…" : notice}</div>
        {problem && <p className="error" role="alert">{problem}</p>}
      </section>
      <aside className="editor recent settings-profiles">
        <h2>Profil tersimpan</h2>
        <button disabled={busy} onClick={() => select()}>+ Profil baru</button>
        <ul className="profile-list">{profiles.map((profile) => <li key={profile.id}>
          <button disabled={busy} aria-pressed={selected?.id === profile.id} onClick={() => select(profile)}>
            <span>{profile.label}</span><span className="profile-url">{profile.baseUrl}</span>
          </button>
        </li>)}</ul>
      </aside>
    </div>
  </main>;
}

export function ProjectModel({ projectId, onSaved }: { projectId: number; onSaved: () => void }) {
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [profileId, setProfileId] = useState("");
  const [modelId, setModelId] = useState("");
  const [models, setModels] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState("");
  const [notice, setNotice] = useState("");

  useEffect(() => {
    void Promise.all([
      invoke<Profile[]>("list_profiles"),
      invoke<Selection | null>("project_model", { projectId })
    ]).then(([items, selection]) => {
      setProfiles(items);
      if (selection) { setProfileId(selection.profileId); setModelId(selection.modelId); }
    }).catch((error) => setProblem(errorText(error)));
  }, [projectId]);

  async function run(work: () => Promise<void>) {
    setBusy(true);
    setNotice("");
    setProblem("");
    try { await work(); }
    catch (error) { setProblem(errorText(error)); }
    finally { setBusy(false); }
  }

  const profile = profiles.find((profile) => profile.id === profileId);
  return <section className="editor settings project-model" aria-labelledby="project-model-heading" aria-busy={busy}>
    <h2 id="project-model-heading">Model proyek</h2>
    {profiles.length > 0 ? <>
      <label htmlFor="project-profile">Profil koneksi</label>
      <select id="project-profile" value={profileId} disabled={busy} onChange={(event) => {
        setProfileId(event.target.value);
        setModels([]);
        setModelId("");
        setNotice("");
        setProblem("");
      }}>
        <option value="">Pilih profil</option>
        {profiles.map((profile) => <option key={profile.id} value={profile.id}>{profile.label}</option>)}
      </select>
      <ModelField id="project-model" value={modelId} models={models} change={setModelId} disabled={busy || !profile} />
      {profile && <p>ngodik akan mengirim ide dan bagian dokumen yang relevan ke <strong className="profile-url">{profile.baseUrl}</strong> saat Anda menyusun atau merevisi blueprint. Kebijakan penyimpanan data mengikuti provider.</p>}
      <div className="settings-actions">
        <button disabled={busy || !profile?.hasKey} onClick={() => void run(async () => {
          setModels([]);
          const ids = await invoke<string[]>("sync_models", { id: profileId });
          setModels(ids);
          setNotice(ids.length ? "Daftar model dimuat." : "Daftar kosong. Masukkan ID model secara manual.");
        })}>Muat daftar model</button>
        <button className="primary" disabled={busy || !profile || !modelId.trim()} onClick={() => void run(async () => {
          await invoke("set_project_model", { projectId, profileId, modelId });
          setNotice("Pilihan model proyek tersimpan.");
          onSaved();
        })}>Pakai model ini</button>
      </div>
      {profile && !profile.hasKey && <p>Tambahkan API key melalui Pengaturan model agar proyek dapat memakai endpoint.</p>}
    </> : <p>Buat profil dari Pengaturan model di halaman Proyek saya.</p>}
    <div role="status">{busy ? "Memproses…" : notice}</div>
    {problem && <p className="error" role="alert">{problem}</p>}
  </section>;
}
