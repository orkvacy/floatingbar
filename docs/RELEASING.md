# Menyiapkan rilis FloatingBar

Rilis yang disiapkan: **0.2.0**. Mengikuti rilis sebelumnya: tag ringan `v0.2.0`, judul **FloatingBar v0.2.0**, ditandai **pre-release**, dengan installer NSIS dan MSI. Catatan berbahasa Inggris mengikuti susunan Highlights / Install / Notes pada v0.1.0.

## Sebelum commit

Secret Discord lama pernah masuk riwayat Git publik. **Reset secret di Discord Developer Portal sebelum menerbitkan rilis**, lalu masukkan penggantinya melalui **Pengaturan integrasi Discord** pada aplikasi. Jangan menaruh secret baru di source, chat, screenshot, atau catatan rilis. Menghapus literal dari source terbaru tidak mencabut secret lama dari riwayat.

Screenshot README menggunakan data demo fiktif. Kredensial runtime berada di folder data aplikasi di luar repository. Jangan unggah binary lama: build baru menghilangkan secret bawaan.

```powershell
bun install --frozen-lockfile
bun run build
cargo check --manifest-path src-tauri/Cargo.toml
python scripts/check-release.py
bun run tauri build
python scripts/check-release.py --artifact src-tauri/target/release/floatingbar.exe
git diff --check
git status --short
```

Periksa diff sebelum staging; jangan sertakan `.env`, JSON OAuth unduhan, token, log, file konfigurasi pribadi, atau screenshot desktop pribadi. Setelah staging, jalankan `python scripts/check-release.py --staged` untuk memeriksa isi index yang benar-benar akan di-commit. Scanner memeriksa pola tertentu, bukan jaminan mendeteksi seluruh informasi privat.

## Commit dan tag oleh maintainer

Contoh pesan mengikuti Conventional Commits repository:

```powershell
git commit -m "feat(app): add daily planning and improve floating controls"
git tag v0.2.0
git push origin main
git push origin v0.2.0
```

Jalankan setelah meninjau dan melakukan staging file yang dipilih. Tag dibuat **setelah commit**, agar menunjuk kode rilis baru. Jangan mengganti tag v0.1.0 atau melakukan force push.

## GitHub Release

Buat release untuk tag `v0.2.0`, judul **FloatingBar v0.2.0**, centang **Set as a pre-release**, dan salin isi [catatan rilis](releases/v0.2.0.md) tanpa judul Markdown pertama.

Lampirkan hasil build baru:

- `src-tauri/target/release/bundle/nsis/FloatingBar_0.2.0_x64-setup.exe`
- `src-tauri/target/release/bundle/msi/FloatingBar_0.2.0_x64_en-US.msi`

Installer dipasang sebagai asset release, bukan di-commit ke Git. Source 0.2.0 tidak membawa kredensial Discord; setiap pengguna mengatur integrasinya sendiri.
