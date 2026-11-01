# Kesiapan rilis pertama

Status per **1 November 2026**: kandidat **0.4.0-rc.1**. Belum dinyatakan stable v1.0. Hasil berikut mencatat pemeriksaan yang dijalankan pada Windows 11 Pro x64, build 26200.

## Cakupan v1.0 yang dituju

Windows 11 x64: floating bar, header drag/body swipe, collapse/pin, media, notifikasi, tugas lokal, Google Calendar read-only, Discord voice, panel Claude, serta chat dan izin sesi Codex yang dibuat melalui FloatingBar. Integrasi memerlukan akun dan izin layanan milik pengguna.

**ChatGPT / Codex** adalah label bersama; jalur chat yang diuji adalah Codex App Server. ChatGPT biasa, chat Antigravity, pertanyaan tool multiopsi Codex, dan persetujuan langsung sesi desktop eksternal tidak dijanjikan sebagai fitur stabil. Detektor desktop tambahan default mati dan berlabel eksperimental.

## Pemeriksaan yang sudah lulus

- Build frontend TypeScript/Vite dan Rust release; installer NSIS EXE dan MSI `0.4.0-rc.1` berhasil dibuat.
- 36 tes offline dari modul produksi: protokol/izin Codex, parsing riwayat, identitas adapter, posisi/multi-DPI, autentikasi HTTP, batas antrean/ukuran/waktu, migrasi konfigurasi, sanitasi keputusan hook.
- Helper GUI Windows mendukung stdin/stdout, mengembalikan `{}` saat offline, tanpa membuka UI atau memberi izin otomatis.
- Helper terautentikasi pada executable hasil MSI menampilkan kartu sintetis; setelah 50 detik tanpa jawaban, hasil `{}` tanpa eksekusi. Tes profil native memvalidasi migrasi file hook lama, backup, dan pelestarian pengaturan lain.
- Source dan executable release lolos pemindaian pola credential serta secret Discord historis yang diketahui. Ini tidak mencabut secret bocor dan bukan pemeriksaan semua kemungkinan data privat.
- Maintainer mengonfirmasi secret Discord historis sudah direset pada 1 November 2026. Blocker rotasi ditutup berdasarkan konfirmasi tersebut; secret pengganti tetap hanya disimpan di pengaturan lokal, dan koneksi voice dengan pengganti perlu diperiksa.
- Native dengan profil terpisah: render/CSP/IPC, buka panel, tambah dan tandai tugas fiktif selesai berhasil. Persistensi tugas setelah restart belum diverifikasi ulang lewat UI.
- Administrative extraction MSI selesai dengan exit code 0. Ini memvalidasi ekstraksi payload, belum instalasi bersih/upgrade/uninstall.
- Tes Codex langsung sebelumnya: daftar/baca, kirim/streaming, penolakan nyata, izin sekali perintah output yang diperiksa. Detail dan batas ada di [integrasi AI](AI-PERMISSIONS.md).

CI Windows sudah disiapkan: action dipin ke commit, izin repository read-only, tanpa publikasi otomatis. **Run GitHub pertama belum dijalankan**; menunggu push maintainer dan status hijau.

## Syarat yang masih terbuka sebelum stable

| Syarat | Status / bukti yang diperlukan |
| --- | --- |
| Instalasi akun/mesin Windows bersih | Belum; pasang, buka tanpa credential, hubungkan layanan milik tester. |
| Upgrade dan uninstall | Belum; uji dari versi sebelumnya, data pengguna tetap benar, hook menunjuk executable baru, tidak ada proses tertinggal. |
| Reboot, sleep/resume, pemulihan layanan | Belum; periksa startup, expiry/login ulang, reconnect Discord, pembatalan OAuth, fallback izin. |
| Layar dan input nyata | Geometri otomatis lulus; native 100/125/150/200%, monitor kedua, mouse/touchpad masih perlu cakupan perangkat. |
| Pengguna di luar mesin pengembang | Belum; perbaiki crash/hang, kehilangan data, atau izin yang salah sebelum v1.0. |
| CI kandidat | Tersedia; run remote pertama menunggu push maintainer. |

Code signing belum tersedia; membantu distribusi dan kepercayaan installer, tetapi bukan syarat teknis untuk tag v1.0. Windows 10 belum tervalidasi untuk kandidat ini.

## Validasi tanpa data pribadi

Jalankan executable hasil build dengan `--test-profile` dan path absolut folder baru bernama `floatingbar-test-*` dalam direktori target yang diabaikan Git. Profil memiliki data WebView, konfigurasi, hook, dan fixture percakapan sendiri; ditandai banner **Uji terisolasi**. Worker media/notifikasi/Discord/UI Automation asli dinonaktifkan. Mode ini tidak membuktikan koneksi akun nyata.

Instalasi bersih berbeda dengan ekstraksi MSI atau membuka executable build. Catat sesuai pengujian sebenarnya. Jangan simpan token, isi percakapan, ID akun, screenshot pribadi, atau log lengkap di laporan publik.

Setelah syarat terbuka selesai dan kandidat dipakai tanpa masalah kritis, naikkan versi konsisten ke **1.0.0**, jalankan pemeriksaan/build lagi, buat tag **v1.0.0**, lalu terbitkan tanpa label pre-release. Langkah Git ada di [panduan rilis](RELEASING.md).
