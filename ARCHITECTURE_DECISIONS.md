# Architecture Decisions — ngodik

Catatan ini merangkum keputusan dari diskusi produk. Coding agent perlu mempertahankan keputusan tersebut atau mengajukan perubahan secara eksplisit sebelum implementasi.

## ADR-001 — Desktop Windows lokal untuk MVP

**Status:** Accepted  
**Keputusan:** MVP menjadi aplikasi desktop lokal Windows menggunakan Tauri.  
**Konteks:** pemilik produk ingin memulai dengan penggunaan pribadi dan kemudahan bagi pengguna non-teknis; hosting ditunda.  
**Konsekuensi:** integrasi Windows dan installer lokal menjadi bagian alpha; hosted/web deployment dan akun berada di luar cakupan. UI web dapat berpotensi digunakan ulang kelak.

## ADR-002 — Stack UI dan backend

**Status:** Accepted  
**Keputusan:** Tauri, Rust untuk local core, React + TypeScript + Vite untuk UI. GSAP digunakan untuk animasi UI yang memang membantu.  
**Konsekuensi:** kontrak UI/backend, versi dependency, component library, dan crate masih perlu diputuskan.

## ADR-003 — Penyimpanan lokal

**Status:** Accepted  
**Keputusan:** SQLite untuk data proyek terstruktur; dokumen/aset berada di folder yang dikelola aplikasi; backup lokal maksimum 10 recovery points terbaru per proyek.  
**Konsekuensi:** backup di perangkat yang sama tidak melindungi dari kehilangan perangkat; schema dan strategi migrasi ditentukan saat perencanaan teknis.

## ADR-004 — Provider LLM dikonfigurasi pengguna

**Status:** Accepted  
**Keputusan:** pengguna memasukkan API key dan base URL endpoint yang kompatibel OpenAI, dapat menyimpan beberapa profil, menyinkronkan model bila bisa atau memasukkan ID manual, lalu memilih satu model per proyek.  
**Konsekuensi:** tidak ada kredensial/model milik ngodik; koneksi internet ke endpoint diperlukan. Alpha menargetkan Chat Completions kompatibel OpenAI.

## ADR-005 — API key dalam Windows Credential Manager

**Status:** Accepted  
**Keputusan:** simpan secret lewat Windows Credential Manager dan kelola hanya di Rust backend.  
**Konsekuensi:** port lintas-platform di masa depan memerlukan adapter credential store; secret tidak boleh masuk database proyek, backup, renderer, log, atau ZIP.

## ADR-006 — Satu model per proyek

**Status:** Accepted  
**Keputusan:** seluruh agen penyusun dan semantic QA dalam satu proyek memakai model terpilih yang sama.  
**Konsekuensi:** tidak ada pemilihan model terpisah per agen pada MVP.

## ADR-007 — Canonical project specification

**Status:** Accepted  
**Keputusan:** satu canonical specification menjadi source of truth; Product & Requirements memeliharanya, agen lain memberi dokumen turunan/proposal perubahan.  
**Konsekuensi:** proses reconciliation dan traceability harus terlihat dan dapat diperiksa.

## ADR-008 — QA adalah tahap, bukan agen kelima

**Status:** Accepted  
**Keputusan:** empat agen penyusun dokumen; QA menjalankan pemeriksaan deterministik dan satu semantic LLM review. Orchestrator adalah komponen sistem.  
**Konsekuensi:** semantic QA memakai model proyek yang sama. Temuan belum selesai tetap dapat diekspor dengan label “Perlu ditinjau”.

## ADR-009 — Draf pertama langsung, pertanyaan ditandai

**Status:** Accepted  
**Keputusan:** buat draf satu kali proses dari ide awal, tampilkan asumsi dan pertanyaan terbuka, lalu pengguna merevisi.  
**Konsekuensi:** pipeline harus mampu menghasilkan dokumen parsial yang berguna alih-alih menunggu wawancara lengkap.

## ADR-010 — Ekspor berisi dokumen, prompt terpisah

**Status:** Accepted  
**Keputusan:** ZIP blueprint memuat dokumen proyek dan instruksi `AGENTS.md`; prompt copy-paste untuk coding agent adalah teks terpisah dari ZIP.  
**Konsekuensi:** format ekspor adaptif; PDF/DOCX ditunda.

## ADR-011 — Pengguna memilih tujuan ZIP saat ekspor

**Status:** Accepted  
**Keputusan:** lokasi output dipilih pengguna ketika mengekspor blueprint.  
**Konsekuensi:** app tidak meminta konfigurasi lokasi ekspor permanen untuk membuat proyek.

## ADR-012 — Rilis alpha dan pembaruan

**Status:** Accepted  
**Keputusan:** alpha privat menggunakan instalasi manual; hosting dan updater otomatis dibahas setelah tahap open source.  
**Konsekuensi:** signing, release pipeline, distribusi publik, dan keamanan kunci updater belum diputuskan.
