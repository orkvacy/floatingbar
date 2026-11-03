# Menyiapkan rilis FloatingBar

Rilis saat ini: **1.0.0 stable**, tag `v1.0.0`, judul **FloatingBar v1.0.0**. Gunakan installer NSIS EXE, MSI, dan checksum; jangan centang **Set as a pre-release**. Rilis rc.1 lama tetap dipertahankan sebagai riwayat.

## Pemeriksaan sebelum publikasi

Maintainer mengonfirmasi reset secret Discord historis pada 1 November 2026. Simpan kredensial pengganti hanya di pengaturan lokal. Screenshot README/panduan menggunakan data demo. Jangan sertakan JSON OAuth, token, log, profil aplikasi, atau screenshot pribadi.

Pada 4 November 2026, maintainer mengonfirmasi seluruh checklist penggunaan nyata lulus: instalasi/update/uninstall, penyimpanan setelah restart, koneksi Google/Discord, chat/izin AI, serta pemulihan internet putus dan sleep/resume. Konfirmasi ini dibedakan dari tes otomatis; lihat [laporan validasi](RELEASE-READINESS.md).

```powershell
bun install --frozen-lockfile
python scripts/check-project.py
python scripts/check-release.py
bun run tauri build --ci
powershell -ExecutionPolicy Bypass -File scripts/test-codex.ps1
python scripts/test-hook-helper.py
python scripts/test-native-profile.py
python scripts/check-release.py --artifact src-tauri/target/release/floatingbar.exe
python scripts/release-manifest.py
git diff --check
```

Scanner memeriksa pola credential dan secret historis yang dikenali, bukan seluruh kemungkinan informasi privat. Tinjau gambar secara visual. Jangan unggah artifact lama.

## Commit dan push

Tinjau perubahan dan stage hanya file yang dimaksud. Setelah staging:

```powershell
python scripts/check-release.py --staged
git diff --cached --check
git commit -m "feat(onboarding): add connection guides for v1.0.0"
git push origin main
```

Tunggu CI commit yang sama hijau. Setelah itu:

```powershell
git tag v1.0.0
git push origin v1.0.0
```

Commit/push dilakukan maintainer atau agen yang ditugaskan secara eksplisit; tanpa trailer kontributor tambahan. Jangan memindahkan tag lama atau force push.

## GitHub Release

Pakai [catatan rilis v1.0.0](releases/v1.0.0.md) tanpa judul Markdown pertama. Buat stable release berjudul **FloatingBar v1.0.0** dan tandai sebagai latest.

Lampirkan tiga file dari `src-tauri/target/release/release-assets/1.0.0/`:

- `FloatingBar_1.0.0_x64-setup.exe`
- `FloatingBar_1.0.0_x64_en-US.msi`
- `SHA256SUMS.txt`

Package, Cargo, Tauri, dan versi MSI memakai `1.0.0`. Pertahankan identifier/upgrade identity. Installer menjadi asset GitHub Release, bukan file yang di-commit.

CI Windows menjalankan build EXE/MSI, 36 tes offline, pemeriksaan CSP/versi/link/credential, helper fallback, dan checksum. Action dipin ke commit; CI tidak menerbitkan release otomatis. Verifikasi hasil untuk commit baru, bukan hanya hasil rc.1 lama.

## Cakupan dan batas dukungan

Windows 11 x64 adalah platform tervalidasi. Installer belum code-signed. Google Calendar masih memakai OAuth Desktop client milik pengguna. Discord RPC bergantung pada akses aplikasi/akun. Panel ChatGPT / Codex memakai Codex CLI/App Server; riwayat ChatGPT biasa dan chat Antigravity tidak termasuk dukungan stabil. Deteksi desktop tambahan tetap eksperimental dan default mati.
