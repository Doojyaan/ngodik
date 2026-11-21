# ngodik

Aplikasi desktop Windows untuk menyusun ide menjadi blueprint proyek. Tahap 1 menyediakan pembuatan proyek, editor ide, simpan otomatis, dan riwayat simpan awal.

## Pengembangan Windows

Pasang Node.js 24, npm, Rust MSVC, Microsoft C++ Build Tools dengan Windows SDK, dan WebView2. Dari folder proyek:

- `npm.cmd ci` memasang dependency frontend dari lockfile.
- `npm.cmd run tauri dev` menjalankan aplikasi.
- `npm.cmd run check` memeriksa tipe frontend.
- `cargo test --locked --manifest-path src-tauri/Cargo.toml` menjalankan tes SQLite.
- `npm.cmd run tauri build -- --no-bundle` membangun executable Windows.

Data disimpan di `%LOCALAPPDATA%\id.ngodik.desktop\ngodik.sqlite3`. Penyimpanan ide mencatat maksimal 10 checkpoint per proyek. Checkpoint ini berada di DB aktif. Fitur pemulihan versi blueprint dan snapshot pemulihan dijadwalkan pada Tahap 4.

## Menghubungkan model

1. Buka **Pengaturan model** dari halaman proyek. Buat profil dengan nama dan URL dasar API yang kompatibel dengan OpenAI Chat Completions, misalnya dengan akhiran /v1 jika provider memerlukannya.
2. Pilih **Masukkan API key**. Tempel key pada kolom Password di dialog Windows. Key disimpan di Windows Credential Manager.
3. Muat daftar model atau ketik ID model secara manual. **Uji koneksi** mengirim pesan pendek, tanpa ide proyek.
4. Buka proyek, lalu pilih profil dan satu ID model di panel **Model proyek**.

Endpoint remote harus memakai HTTPS. Untuk API di perangkat sendiri, HTTP dapat dipakai pada localhost atau IP loopback. Buat profil baru saat mengganti alamat API agar key lama tetap terkait dengan tujuan semula. Menghapus profil juga melepas pilihan model pada proyek yang memakainya.

## Membuat draf awal

Simpan satu model pada proyek, tulis ide, lalu periksa alamat pengiriman di panel Draf blueprint. Draf menyimpan salinan ide dan tujuan pengiriman saat proses dimulai. Bila proses gagal, dibatalkan, atau terhenti ketika aplikasi ditutup, **Lanjutkan draf** atau **Coba lagi** memakai salinan tersebut dan meneruskan tahap yang belum selesai. **Buat draf baru** memakai ide dan pilihan model terbaru.

Skema kanonis v1 berada di src-tauri/schemas/project-specification.v1.json. Draf awal sudah menjalani pemeriksaan struktur dasar dan ID kebutuhan. QA semantik dan revisi tersedia di tab Review. Ekspor ZIP dijadwalkan pada Tahap 5.

## Meninjau dan memulihkan blueprint

Setelah draf disusun, tab **Review** memisahkan pemeriksaan struktur dan referensi dari satu tinjauan semantik dengan model proyek. Temuan dapat menyertakan saran dan kebutuhan yang terdampak. Menyalin saran ke formulir revisi tidak langsung mengirim permintaan.

Ajukan revisi untuk seluruh blueprint atau dokumen yang tersedia, lalu bandingkan usulan sebelum menerapkan atau membuangnya. **Versi blueprint** menyimpan sepuluh snapshot terbaru dengan checksum. Pemulihan menerbitkan versi aktif baru dari snapshot yang dipilih. Ide di editor mempunyai riwayat simpan sendiri.
