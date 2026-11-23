# Panduan Penggunaan ngodik

ngodik adalah aplikasi desktop Windows untuk mengubah ide awal menjadi blueprint proyek yang dapat ditinjau dan diekspor. Aplikasi ini berstatus alpha; periksa setiap hasil sebelum dijadikan dasar implementasi.

## 1. Buat proyek dan tulis ide

1. Buka ngodik lalu buat proyek baru.
2. Tulis konteks, masalah, pengguna sasaran, tujuan, dan batasan yang sudah diketahui.
3. Perubahan ide disimpan otomatis dan memiliki checkpoint revisi.

Tuliskan detail yang nyata. Tandai hal yang belum diketahui sebagai pertanyaan agar draf dapat memisahkan asumsi dari keputusan.

## 2. Hubungkan model

1. Buka **Pengaturan** dan buat profil provider.
2. Masukkan nama profil dan base URL API yang kompatibel dengan OpenAI Chat Completions. Ikuti format base URL yang disyaratkan provider Anda.
3. Tekan **Masukkan API key**. Tempel key pada dialog Windows. ngodik menyimpan key melalui Windows Credential Manager.
4. Muat daftar model atau masukkan ID model secara manual.
5. Uji koneksi. Uji koneksi hanya mengirim pesan singkat, tidak mengirim ide proyek.
6. Buka proyek dan pilih satu profil serta model untuk proyek tersebut.

Gunakan HTTPS untuk endpoint remote. HTTP hanya dapat digunakan pada localhost atau alamat loopback untuk layanan di komputer yang sama. Buat profil baru jika alamat endpoint berubah agar key tetap terikat ke tujuan yang benar.

## 3. Buat blueprint

1. Pastikan ide dan model proyek sudah dipilih.
2. Mulai pembuatan draf.
3. Ikuti status empat tahap: produk dan kebutuhan, analisis sistem, pengalaman pengguna, serta arsitektur solusi.
4. Bila proses terputus, pilih **Lanjutkan draf** atau **Coba lagi** untuk memakai salinan ide dan tujuan model yang disimpan saat proses dimulai.
5. Pilih pembuatan draf baru jika ingin menggunakan ide atau model terbaru.

## 4. Tinjau, revisi, dan pulihkan

- Buka tab **Review** untuk melihat pemeriksaan struktur dan referensi, serta satu tinjauan semantik berbantuan model.
- Tinjau setiap temuan dan saran. Pemeriksaan semantik memerlukan koneksi ke model.
- Mengirim revisi membuat proposal. Bandingkan perubahan sebelum memilih terapkan atau buang.
- Gunakan tab **Versi** untuk melihat snapshot blueprint. Pemulihan membuat versi aktif baru berdasarkan snapshot yang dipilih.
- Riwayat penyimpanan ide terpisah dari riwayat versi blueprint.

Temuan yang belum selesai dapat ditandai pada hasil ekspor. Jangan anggap hasil AI sebagai keputusan produk yang sudah disetujui.

## 5. Ekspor handoff

1. Buka tab **Ekspor**.
2. Periksa daftar dokumen dan status temuan.
3. Pilih **Simpan ZIP**, lalu tentukan folder tujuan.
4. Buka paket ZIP sebelum diberikan ke coding agent. Pastikan spesifikasi, dokumen turunan, dan catatan temuan sesuai kebutuhan.

Paket memuat dokumen blueprint dan instruksi agent. Prompt handoff dapat disalin terpisah melalui tombol yang tersedia.

## Lokasi dan cadangan data

Di Windows, database tersimpan di direktori data lokal aplikasi, biasanya %LOCALAPPDATA%\id.ngodik.desktop\ngodik.sqlite3. Berkas aplikasi dan data pemulihan berada di direktori lokal aplikasi yang sama.

Salin data penting secara berkala saat aplikasi ditutup. Folder di perangkat yang sama tidak melindungi data jika perangkat hilang atau rusak.
