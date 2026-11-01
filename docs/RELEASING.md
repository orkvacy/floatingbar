# Menyiapkan rilis FloatingBar

Rilis yang disiapkan: **0.4.0-rc.1**. Mengikuti rilis sebelumnya: tag ringan `v0.4.0-rc.1`, judul **FloatingBar v0.4.0-rc.1**, ditandai **pre-release**, dengan installer NSIS dan MSI. Catatan berbahasa Inggris mengikuti susunan Highlights / Install / Notes pada v0.1.0.

Label UI untuk host OpenAI disatukan sebagai **ChatGPT / Codex**. Implementasi chat memakai Codex CLI/App Server dan cakupan yang diuji dicatat di [integrasi AI](AI-PERMISSIONS.md).

Lihat [kesiapan rilis pertama](RELEASE-READINESS.md) untuk hasil pemeriksaan dan syarat stable v1.0. Kandidat ini belum dinyatakan stable.

## Sebelum commit

Secret Discord lama pernah masuk riwayat Git publik. **Maintainer mengonfirmasi reset secret pada 1 November 2026**; blocker rotasi ditutup berdasarkan konfirmasi tersebut. Simpan penggantinya melalui **Pengaturan integrasi Discord** pada aplikasi dan periksa koneksi voice. Jangan menaruh secret baru di source, chat, screenshot, atau catatan rilis. Menghapus literal dari source terbaru tidak mencabut secret lama dari riwayat.

Screenshot README menggunakan data demo fiktif. Kredensial runtime berada di folder data aplikasi di luar repository. Jangan unggah binary lama: build baru menghilangkan secret bawaan.

```powershell
bun install --frozen-lockfile
python scripts/check-project.py
bun run tauri build --ci
powershell -ExecutionPolicy Bypass -File scripts/test-codex.ps1
python scripts/check-release.py
python scripts/test-hook-helper.py
python scripts/test-native-profile.py
python scripts/check-release.py --artifact src-tauri/target/release/floatingbar.exe
python scripts/release-manifest.py
git diff --check
git status --short
```

Periksa diff sebelum staging; jangan sertakan `.env`, JSON OAuth unduhan, token, log, file konfigurasi pribadi, atau screenshot desktop pribadi. Setelah staging, jalankan `python scripts/check-release.py --staged` untuk memeriksa isi index yang benar-benar akan di-commit. Scanner memeriksa pola tertentu, bukan jaminan mendeteksi seluruh informasi privat.

## Commit dan tag oleh maintainer

Contoh pesan mengikuti Conventional Commits repository:

```powershell
git commit -m "fix(release): harden permission hooks and add release checks"
git tag v0.4.0-rc.1
git push origin main
git push origin v0.4.0-rc.1
```

Jalankan setelah meninjau dan melakukan staging file yang dipilih. Tag dibuat **setelah commit**, agar menunjuk kode rilis baru. Jangan mengganti tag rilis lama atau melakukan force push.

## GitHub Release

Buat release untuk tag `v0.4.0-rc.1`, judul **FloatingBar v0.4.0-rc.1**, centang **Set as a pre-release**, dan salin isi [catatan rilis](releases/v0.4.0-rc.1.md) tanpa judul Markdown pertama.

Lampirkan hasil build baru:

- `src-tauri/target/release/bundle/nsis/FloatingBar_0.4.0-rc.1_x64-setup.exe`
- `src-tauri/target/release/bundle/msi/FloatingBar_0.4.0-rc.1_x64_en-US.msi`

Installer dipasang sebagai asset release, bukan di-commit ke Git. Source 0.4.0-rc.1 tidak membawa kredensial Discord; setiap pengguna mengatur integrasinya sendiri.


## Versi installer dan CI

Package, Cargo, dan Tauri memakai versi `0.4.0-rc.1`. MSI memerlukan angka: override WiX `0.4.0.1`; nama artifact tetap memakai versi SemVer. Perbarui override untuk kandidat berikutnya. Saat stable `1.0.0`, hapus override atau samakan dengan versi stable, pertahankan identitas upgrade, dan uji upgrade nyata.

CI Windows menjalankan pemeriksaan source, CSP, konsistensi versi, build EXE/MSI, 36 tes offline, helper fallback, pemindaian credential executable, dan pembuatan checksum. Action dipin ke commit, izin repository read-only. CI menyimpan artifact tanpa menerbitkan release. Run remote pertama menunggu push maintainer.

Untuk upload, gunakan tiga file di `src-tauri/target/release/release-assets/0.4.0-rc.1/`: installer EXE, MSI, dan `SHA256SUMS.txt`. Jangan upload artifact versi lama. Jalankan `python scripts/check-release.py --staged` sesudah memilih file staging dan sebelum commit. Commit, tag, dan push dilakukan maintainer sendiri, tanpa trailer kontributor tambahan.

Gunakan **v1.0.0** dan lepaskan label pre-release setelah syarat stable dalam [kesiapan rilis](RELEASE-READINESS.md) terpenuhi. Rotasi secret telah dikonfirmasi maintainer; untuk kandidat ini, lanjutkan pemeriksaan konfigurasi Discord lokal, push source, dan tunggu CI hijau sebelum publikasi pre-release.
