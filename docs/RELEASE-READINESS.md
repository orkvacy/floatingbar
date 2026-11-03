# Kesiapan rilis pertama

Status per **4 November 2026**: menyiapkan **1.0.0 stable**. Maintainer mengonfirmasi seluruh checklist penggunaan nyata telah diuji dan lulus. Hasil otomatis serta hasil yang dilaporkan maintainer dibedakan di bawah. Pemeriksaan dasar rc.1 dilakukan pada Windows 11 Pro x64, build 26200.

## Cakupan v1.0

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

Pemeriksaan ulang **4 November 2026**: CI Windows untuk commit `8cced86` sudah **lulus**, dan `v0.4.0-rc.1` sudah diterbitkan sebagai pre-release. [Hasil CI](https://github.com/orkvacy/floatingbar/actions/runs/41296035901) dan [rilis kandidat](https://github.com/orkvacy/floatingbar/releases/tag/v0.4.0-rc.1). Action dipin ke commit, izin repository read-only, tanpa publikasi otomatis. Hasil uji lokal di atas tetap merujuk pemeriksaan 1 November.

## Pengujian penggunaan nyata oleh maintainer

Dikonfirmasi lulus pada **4 November 2026** melalui checklist di chat. Hasil ini dilaporkan maintainer, bukan pengujian independen agen pada setiap perangkat.

| Pengujian | Status |
| --- | --- |
| Instalasi, update dari versi lama, dan uninstall | Lulus berdasarkan konfirmasi maintainer. |
| Penyimpanan to-do, posisi, dan pengaturan setelah restart | Lulus berdasarkan konfirmasi maintainer. |
| Koneksi Google Calendar/Discord serta chat dan izin Claude/Codex | Lulus berdasarkan konfirmasi maintainer. |
| Pemulihan internet putus dan sleep/resume | Lulus berdasarkan konfirmasi maintainer. |

Cakupan build Windows, DPI, monitor, dan jumlah tester tidak dirinci dalam konfirmasi tersebut. Pengujian pada perangkat tambahan tetap berguna dan tidak diklaim sudah dilakukan untuk setiap kombinasi.

Code signing belum tersedia; membantu distribusi dan kepercayaan installer, tetapi bukan syarat teknis untuk tag v1.0. Windows 10 belum tervalidasi untuk kandidat ini.

## Validasi tanpa data pribadi

Jalankan executable hasil build dengan `--test-profile` dan path absolut folder baru bernama `floatingbar-test-*` dalam direktori target yang diabaikan Git. Profil memiliki data WebView, konfigurasi, hook, dan fixture percakapan sendiri; ditandai banner **Uji terisolasi**. Worker media/notifikasi/Discord/UI Automation asli dinonaktifkan. Mode ini tidak membuktikan koneksi akun nyata.

Instalasi bersih berbeda dengan ekstraksi MSI atau membuka executable build. Catat sesuai pengujian sebenarnya. Jangan simpan token, isi percakapan, ID akun, screenshot pribadi, atau log lengkap di laporan publik.

Versi source dan installer ditetapkan ke **1.0.0** untuk publikasi stable. Build, pemeriksaan credential, dan CI commit rilis wajib lulus sebelum publikasi. Langkah Git ada di [panduan rilis](RELEASING.md).


## Pembaruan panduan koneksi (v1.0.0)

Panduan Google Calendar lima langkah, panduan Discord/Codex, bantuan masalah umum, dan screenshot demo ditambahkan. Panel tetap terbuka saat panduan dibaca; area panduan tidak memicu swipe halaman. Panduan lengkap ada di [koneksi akun](CONNECTIONS.id.md).

Pratinjau browser: navigasi langkah, pindah langsung ke langkah, buka bantuan agenda kosong, tutup panduan, dan pembukaan panduan Discord/Codex lulus. Pada 4 November 2026, build release v1.0.0 (EXE/MSI), 36 tes regresi, pemeriksaan versi/CSP/link/credential executable, serta tes helper/startup terisolasi lulus. Checksum kedua installer dibuat. CI commit baru menjadi pemeriksaan terakhir sebelum publikasi. Pengujian penggunaan nyata tercatat terpisah berdasarkan konfirmasi maintainer di atas.
