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
