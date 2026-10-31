# FloatingBar

**Jawab Yes/No Claude langsung dari floating bar Windows.** Tinjau permintaan izin, lalu klik **Izinkan** atau **Tolak** tanpa berpindah jendela. FloatingBar juga menyediakan panel Claude, kontrol voice Discord, to-do list, agenda Google Calendar, musik, dan notifikasi.

Dibangun dengan Tauri 2 (Rust) dan React + TypeScript.

[Read in English](README.md)

<p align="center">
  <img src="docs/screenshots/permission.png" alt="Claude permission request with Deny and Allow buttons" width="720">
</p>

**Fitur utama: tinjau perintah Claude, lalu jawab Izinkan / Tolak dari pill.**

| Claude | Discord voice |
| :---: | :---: |
| ![Claude panel](docs/screenshots/claude.png) | ![Discord voice controls](docs/screenshots/discord.png) |

| To-do list | Google Calendar |
| :---: | :---: |
| ![Local to-do list](docs/screenshots/todos.png) | ![Today’s agenda](docs/screenshots/calendar.png) |

<details>
<summary>Pratinjau lainnya</summary>

| Music | Notifications |
| :---: | :---: |
| ![Music controls](docs/screenshots/music.png) | ![Notification history](docs/screenshots/notifications.png) |

| Compact music | Compact Discord |
| :---: | :---: |
| ![Compact music pill](docs/screenshots/compact.png) | ![Compact voice avatars](docs/screenshots/discord-compact.png) |

</details>

<sub>Semua screenshot memakai demo fiktif: tanpa chat, agenda, foto profil, akun, atau kredensial pribadi.</sub>

> FloatingBar adalah proyek independen, tidak berafiliasi dengan Apple maupun Anthropic. Teks antarmukanya saat ini berbahasa Indonesia.

---

## Daftar isi

- [Fitur](#fitur)
- [Kebutuhan](#kebutuhan)
- [Instalasi](#instalasi)
- [Memakai FloatingBar](#memakai-floatingbar)
- [Panel Discord](#panel-discord)
- [To-do list dan Google Calendar](#to-do-list-dan-google-calendar)
- [Integrasi Claude](#integrasi-claude)
- [Opsional: notifikasi hanya di FloatingBar](#opsional-notifikasi-hanya-di-floatingbar)
- [Konfigurasi dan file data](#konfigurasi-dan-file-data)
- [Privasi dan keamanan](#privasi-dan-keamanan)
- [Keterbatasan](#keterbatasan)
- [Pemecahan masalah](#pemecahan-masalah)
- [Uninstall](#uninstall)
- [Build dari source](#build-dari-source)
- [Cara kerja](#cara-kerja)
- [Berkontribusi](#berkontribusi)
- [Lisensi](#lisensi)

## Fitur

- **Izin Claude Yes/No**: jawab **Izinkan / Tolak** untuk prompt Claude desktop dan hook Claude Code.
- **Discord voice**: anggota, indikator bicara, mute/deafen, volume per anggota, avatar ringkas, dan pin.
- **Agenda dan tugas**: to-do lokal dan acara hari ini dari kalender Google pilihan, termasuk kalender bersama.
- **Pill mengambang** di tengah atas layar yang membesar saat di-hover, dengan animasi pegas. Bisa digeser ke mana saja dan posisinya diingat.
- **Tembus klik**: area kosong di sekitar pill tidak menghalangi aplikasi di bawahnya.
- **Lagu yang diputar** dari aplikasi apa pun yang terhubung ke kontrol media Windows (Spotify, YouTube di browser, pemutar media): cover, judul, artis, progres, putar/jeda/berikutnya/sebelumnya.
- **Volume per aplikasi**: slider (atau scroll di atas pill) mengatur volume aplikasi yang sedang memutar lagu, bukan volume seluruh sistem.
- **Notifikasi**: notifikasi Windows tampil sebagai kartu di pill, dan halaman Notifikasi menyimpan 20 yang terakhir.
- **Shortcut**: pintasan sekali klik ke situs, aplikasi, atau program, diatur lewat file JSON.
- **Baterai dan jam** sekilas pandang.
- **Panel Claude**: pilih mode Chat atau Code dan percakapan di aplikasi Claude desktop, baca pesan-pesan terakhirnya, kirim prompt (bisa dengan gambar yang ditempel), lalu ikuti balasannya, semuanya dari pill.
- **Ikon tray** untuk menampilkan/menyembunyikan, mengembalikan posisi, autostart, dan keluar. Hanya satu instance yang berjalan dalam satu waktu.

## Kebutuhan

- Windows 10 atau 11 (dikembangkan dan diuji di Windows 11).
- Microsoft Edge WebView2 Runtime (sudah ada di Windows 11; installer menambahkannya di Windows 10 bila belum ada).
- Untuk fitur Claude: [aplikasi Claude desktop](https://claude.ai/download) dengan antarmuka berbahasa Inggris.

## Instalasi

Unduh `FloatingBar_<versi>_x64-setup.exe` (atau `.msi`) dari halaman [Releases](https://github.com/orkvacy/floatingbar/releases) lalu jalankan. Windows SmartScreen mungkin memperingatkan bahwa aplikasinya tidak dikenal karena installer-nya belum ditandatangani (code-signed); pilih **More info → Run anyway** kalau kamu memercayai sumbernya.

Atau [build sendiri](#build-dari-source).

Setelah terpasang:

1. **Izinkan akses notifikasi** supaya FloatingBar bisa membacanya: *Settings → Privacy & security → Notifications* → nyalakan **Let apps access your notifications** (namanya bisa sedikit berbeda tergantung versi Windows). Kalau aksesnya belum ada, pill yang membesar menampilkan petunjuk berwarna oranye.
2. Opsional: klik kanan ikon tray → **Jalan saat Windows start**.
3. Opsional: [buat notifikasi hanya muncul di FloatingBar](#opsional-notifikasi-hanya-di-floatingbar).

## Memakai FloatingBar

### Pill

| Aksi | Hasil |
| --- | --- |
| Hover pill | Membesarkan pill. Pill mengecil lagi sesaat setelah kursor keluar. |
| Tarik header atas (atau pill ringkas) | Memindahkan jendela dan menyimpan posisi. Drag isi halaman untuk berpindah slide. |
| Klik dua kali header | Mengembalikannya ke tengah atas layar. |
| Scroll atas/bawah di atas pill | Mengatur volume aplikasi yang sedang memutar lagu (di halaman Musik atau saat pill kecil). |
| Klik kiri tahan lalu tarik isi halaman ke kiri/kanan; atau swipe touchpad / Shift + scroll | Berpindah slide tanpa memindahkan bar. |
| Klik titik di bawah halaman | Langsung ke halaman itu. |

Saat kecil, pill menampilkan cover album dan equalizer kecil kalau ada lagu yang diputar, atau jam dan baterai kalau tidak ada. Pill terbuka sendiri beberapa detik saat charger dicolok.

### Halaman

Pill yang membesar punya empat halaman, dari kiri ke kanan:

1. **Notifikasi**: 20 notifikasi terakhir lengkap dengan ikon aplikasi dan waktunya. Klik salah satu untuk menghapusnya, atau *Hapus semua*. Daftar ini hanya disimpan di memori dan kosong lagi setelah aplikasi dibuka ulang.
2. **Musik**: cover, judul, artis, progres, tombol sebelumnya/putar-jeda/berikutnya, serta slider volume dengan tombol mute. Halaman ini yang tampil pertama.
3. **Claude**: lihat [Integrasi Claude](#integrasi-claude).
4. **Agenda**: acara hari ini dari kalender Google yang dipilih (login opsional).

Header di setiap halaman menampilkan jam, tanggal, shortcut-mu, dan persentase baterai.

### Kartu notifikasi

Notifikasi baru tampil sebagai kartu di pill selama sekitar 6 detik (lebih lama selama kursor ada di atasnya). Klik kartunya untuk menutup. Notifikasi didahulukan dari tampilan musik, dan permintaan izin didahulukan dari notifikasi.

### Volume

Volume berlaku untuk aplikasi pemilik media yang sedang diputar: Spotify, Chrome, Edge, Firefox, Opera, Brave, VLC, dan aplikasi apa pun yang sesi medianya menyebut nama `.exe`-nya. Untuk aplikasi lain (misalnya aplikasi Media Player Windows), slider mengatur volume sistem. Hover slider untuk melihat mana yang sedang diatur.

Windows memperlakukan semua tab browser sebagai satu sumber suara, jadi volume browser berlaku untuk semua tabnya. Supaya musik bisa diatur terpisah, putar musik di browser atau aplikasi yang berbeda dari yang kamu pakai untuk suara lain.

### Shortcut

Ikon di header membuka situs, aplikasi, atau program. Klik **+** di header (atau tray → *Atur shortcut…*) untuk mengedit daftarnya di Notepad; perubahan muncul saat pill dibuka berikutnya. Lihat [shortcuts.json](#shortcutsjson).

### Menu tray

Ikon tray mungkin tersembunyi di balik panah **^** di taskbar; seret keluar supaya selalu terlihat.

- **Klik kiri**: tampilkan atau sembunyikan pill.
- **Klik kanan**: menu
  - *Sembunyikan pill / Tampilkan pill*
  - *Kembalikan ke tengah atas*
  - *Atur shortcut…*
  - *Jalan saat Windows start* (tanda centang menunjukkan status saat ini)
  - *Keluar*

Membuka FloatingBar lagi saat sudah berjalan hanya memunculkan pill kembali.

Saat diletakkan dekat bagian bawah monitor, pill membesar ke atas. Jendela dibatasi area kerja monitor agar tidak melewati tepi layar atau taskbar; panel panjang bisa di-scroll jika ruang terbatas.

## Panel Discord

Buka Discord desktop, lalu panel headset di FloatingBar. Pada **Pengaturan integrasi Discord**, masukkan Client ID dan Client Secret aplikasi milik Anda dari Discord Developer Portal, simpan, lalu setujui otorisasi Discord. Akses RPC voice bergantung pada izin aplikasi Discord; jika akses dibatasi, akun Anda mungkin perlu terdaftar sebagai tester. Tidak ada kredensial Discord yang disertakan dalam source atau installer baru. Kredensial dan token disimpan terenkripsi Windows DPAPI pada perangkat ini. Gunakan pengaturan yang sama untuk mengganti secret setelah rotasi.

Saat panel Discord dipilih dan voice aktif, kursor yang meninggalkan pill akan mengecilkannya menjadi deretan avatar anggota. Hover pill untuk membuka panel kembali. Tombol **pin** di kanan atas panel mempertahankan panel terbuka; klik lagi untuk kembali mengecil otomatis. Maksimal lima avatar ditampilkan dalam mode ringkas, diikuti jumlah anggota lainnya; lingkaran hijau menandai yang sedang berbicara.

Volume setiap anggota dibatasi ke lebar baris agar tidak keluar dari pill, termasuk ketika panah pagination tampil.

## To-do list dan Google Calendar

Klik ikon daftar tugas di sebelah kiri headset Discord untuk menambah, mengedit, mencentang, atau menghapus tugas. Daftar tersimpan di penyimpanan WebView perangkat ini dan tetap tersedia setelah aplikasi ditutup. Panel tetap terbuka sampai ditutup lewat ikon atau tombol ×.

Halaman keempat, **Agenda**, menampilkan acara hari ini dari **kalender Google yang dipilih**, termasuk kalender kelas dan kalender yang dibagikan dari akun lain: jam mulai/selesai, acara seharian, lokasi, tautan acara, dan penanda sedang berlangsung. Acara berulang diperluas oleh Google Calendar. Buka **Pilih kalender** untuk mencentang sumber agenda; pilihan awal mengikuti kalender yang ditampilkan di Google Calendar. Setiap acara menampilkan nama kalender asalnya. Akun lain yang hanya login terpisah di browser tidak otomatis terhubung: bagikan atau tambahkan kalendernya ke akun yang dihubungkan ke FloatingBar, atau gunakan **Hubungkan ulang / ganti akun** (mengganti akun aktif). Pilihan sumber hanya berlaku di FloatingBar. Hari dan jam mengikuti zona waktu komputer. Agenda diperbarui saat halaman aktif dibuka dan setiap satu menit selama halaman tersebut aktif; tombol perbarui tersedia untuk sinkronisasi manual. Jika koneksi gagal, agenda terakhir pada hari yang sama tetap terlihat dengan pesan error.

Setup satu kali:

1. Di [Google Cloud Console](https://console.cloud.google.com/apis/library/calendar-json.googleapis.com), pilih/buat project dan aktifkan **Google Calendar API**.
2. Atur Google Auth Platform / OAuth consent screen. Untuk aplikasi dalam status **Testing**, masukkan email Google yang akan dipakai sebagai **test user**.
3. Buat OAuth client bertipe **Desktop app** melalui Google Auth Platform → Clients, lalu unduh JSON kredensialnya.
4. Di FloatingBar, buka halaman **Agenda** → **Hubungkan Google Calendar** → pilih JSON tersebut (atau tempel isinya).
5. Lanjutkan login di browser default dan berikan izin baca kalender. Jangan commit file kredensial ke repository.

FloatingBar memakai OAuth Authorization Code + PKCE dengan callback loopback lokal dan hanya meminta scope baca `calendar.events.readonly` dan `calendar.calendarlist.readonly`. Pengguna versi lama cukup menekan **Perbarui izin Google** dan menyetujui tambahan izin baca daftar kalender; file JSON tidak perlu dipilih lagi. Jika login ulang dibatalkan, koneksi lama tetap tersimpan. Kredensial dan token disimpan terenkripsi dengan Windows DPAPI di `%APPDATA%\id.orkvacy.floatingbar\google-calendar.dat`; token tidak dikirim ke frontend atau disimpan dalam source. **Kelola koneksi → Putuskan koneksi** menghapus token lokal. Untuk mencabut akses Google sepenuhnya, gunakan **Kelola izin di Google**. Google dapat meminta login ulang ketika izin dicabut atau token aplikasi Testing berakhir.

Integrasi akun hanya berjalan pada aplikasi desktop. Pratinjau `bun run dev` dapat menggunakan `?demo=calendar` untuk agenda contoh dan `?demo=todos` untuk panel tugas. Contoh agenda bukan data dari akun Google.

## Integrasi Claude

Fitur-fitur ini bekerja dengan [aplikasi Claude desktop](https://claude.ai/download) dan Claude Code. FloatingBar mengendalikan aplikasi Claude lewat Windows UI Automation (API aksesibilitas yang juga dipakai screen reader) dengan membaca label berbahasa Inggris aplikasinya, jadi perubahan tampilan aplikasi Claude bisa membuatnya berhenti bekerja sampai FloatingBar diperbarui.

### Panel Claude

Di halaman Claude:

1. Pilih **Chat** atau **Code**. Mode di aplikasi Claude ikut berpindah.
2. Pilih percakapan dari daftar (diambil dari sidebar aplikasi), atau **＋ Chat baru / Sesi baru** untuk percakapan baru. Percakapan yang sedang terbuka di aplikasi otomatis terpilih dan ditandai **● … (terbuka)**. Memilih percakapan akan membukanya di aplikasi dan memuat 12 pesan terakhirnya ke panel.
3. Ketik prompt lalu tekan **Enter** untuk mengirim (**Shift+Enter** untuk baris baru). FloatingBar membuka percakapan itu di aplikasi, mengisi prompt, menekan Send, lalu menampilkan balasan selagi Claude menulis.

Kontrol lain:

- **⟳** memuat ulang daftar percakapan.
- **⤢ / ⤡** berpindah antara ukuran kecil dan tinggi. Kamu juga bisa menarik pegangan di bawah ke tinggi mana pun; klik dua kali untuk kembali ke awal. Ukurannya diingat.
- **Tempel gambar** (Ctrl+V) di kotak prompt untuk melampirkannya. Saat dikirim, FloatingBar membawa jendela Claude ke depan sebentar dan menempelkan gambarnya di sana dengan Ctrl+V. Gambar diambil dari clipboard saat itu, jadi jangan menyalin hal lain di antaranya.

Panel tetap terbuka selama kamu mengetik atau Claude sedang membalas. Kalau aplikasi Claude tidak berjalan, panel menawarkan untuk membukanya.

> Mengirim ke percakapan yang sedang terbuka di aplikasi akan menambahkan pesanmu ke percakapan itu. Pilih **＋ Chat baru** untuk memulai dari awal.

### Menjawab permintaan izin

FloatingBar bisa menjawab dua jenis permintaan izin. Di keduanya, kartu di pill menyediakan **Izinkan**, **Tolak**, dan **Buka di Claude** (jawab di aplikasi Claude saja).

**Permintaan izin di aplikasi Claude desktop** (misalnya tool konektor di Chat) tertangkap otomatis: FloatingBar memantau aplikasi untuk prompt *Allow once / Decline* dan menekan tombol yang sesuai saat kamu menjawab di pill.

**Permintaan izin Claude Code** (tab Code di aplikasi desktop atau CLI `claude`) memakai hook Claude Code. Tambahkan ini ke `~/.claude/settings.json` (gabungkan dengan `hooks` yang sudah ada, kalau ada):

```json
{
  "hooks": {
    "PermissionRequest": [
      {
        "hooks": [
          {
            "type": "http",
            "url": "http://127.0.0.1:47821/permission",
            "timeout": 60,
            "statusMessage": "Menunggu jawaban di FloatingBar…"
          }
        ]
      }
    ]
  }
}
```

Dengan hook ini, setiap permintaan izin muncul di pill bersama nama tool, folder proyek, dan perintah atau file yang diminta. Kalau tidak dijawab dalam 55 detik, kalau pill sedang disembunyikan, atau kalau FloatingBar tidak berjalan, Claude Code kembali ke prompt izinnya yang biasa. Untuk mematikannya, hapus blok `PermissionRequest` dari file pengaturan itu.

## Opsional: notifikasi hanya di FloatingBar

Secara default Windows tetap menampilkan banner notifikasinya sendiri di pojok layar. Skrip `scripts/quiet-windows-notifications.ps1` mematikan banner itu supaya notifikasi hanya muncul di pill. Skrip ini hanya mengubah pengaturan user (tanpa admin) dan bisa dibatalkan kapan saja.

Jalankan dari folder proyek di PowerShell:

```powershell
# 1. Lihat dulu apa yang akan diubah, tanpa mengubah apa pun
powershell -ExecutionPolicy Bypass -File scripts\quiet-windows-notifications.ps1 -WhatIf

# 2. Terapkan
powershell -ExecutionPolicy Bypass -File scripts\quiet-windows-notifications.ps1

# 3. Kembalikan semuanya seperti sebelumnya
powershell -ExecutionPolicy Bypass -File scripts\quiet-windows-notifications.ps1 -Undo
```

Yang dilakukan:

- Untuk setiap aplikasi yang pernah mengirim notifikasi, mematikan **banner pop-up** (`ShowBanner = 0` di `HKCU\Software\Microsoft\Windows\CurrentVersion\Notifications\Settings\<aplikasi>`). Notifikasi tetap masuk ke notification center, tempat FloatingBar membacanya. Aplikasi yang notifikasinya memang sudah dimatikan total tidak disentuh.
- Mematikan **kedipan tombol taskbar** (`TaskbarFlashing = 0` di `HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced`, Windows 11 22H2 ke atas), supaya taskbar yang tersembunyi otomatis tidak muncul saat ada aplikasi yang minta perhatian. Tambahkan `-KeepTaskbarFlashing` untuk melewatinya.
- Menyimpan nilai sebelumnya ke `%APPDATA%\id.orkvacy.floatingbar\quiet-notifications-backup.json`. Menjalankan skrip berkali-kali tetap menyimpan nilai aslinya, jadi `-Undo` selalu mengembalikan ke kondisi awalmu yang sebenarnya.

Perlu diketahui:

- Aplikasi yang mengirim notifikasi **pertama kalinya** setelah skrip dijalankan masih menampilkan banner. Jalankan skripnya lagi untuk menenangkannya.
- Kalau banner masih muncul, **sign out lalu masuk lagi** supaya Windows memuat ulang pengaturannya.
- Jangan matikan notifikasi sepenuhnya di Settings Windows, karena FloatingBar juga tidak akan menerima apa pun.

**Tanpa skrip**, kamu bisa melakukannya secara manual:

- *Settings → System → Notifications → (sebuah aplikasi)* → matikan **Show notification banners**, biarkan **Show notifications in notification center** menyala. Ulangi untuk setiap aplikasi.
- Atau nyalakan **Do not disturb** di *Settings → System → Notifications* dan matikan jadwal otomatisnya.
- *Settings → Personalization → Taskbar → Taskbar behaviors* → matikan **Show flashing on taskbar apps**.

Untuk mengembalikan secara manual, nyalakan lagi opsi-opsi tersebut.

## Konfigurasi dan file data

FloatingBar menyimpan file-filenya di `%APPDATA%\id.orkvacy.floatingbar\`:

| File | Kegunaan |
| --- | --- |
| `shortcuts.json` | Shortcut di header. Dibuat dengan isi default saat pertama dipakai. |
| `position.json` | Posisi terakhir pill. Hapus untuk mengembalikan ke tengah. |
| `discord-client.dat` | Client ID/secret Discord, terenkripsi DPAPI. |
| `discord-token.dat` | Token Discord, terenkripsi DPAPI. |
| `google-calendar.dat` | Kredensial dan token Google, terenkripsi DPAPI. |
| `quiet-notifications-backup.json` | Hanya dibuat oleh skrip opsional, dipakai oleh `-Undo`. |

Tinggi panel Claude disimpan di penyimpanan WebView aplikasi di `%LOCALAPPDATA%\id.orkvacy.floatingbar\`.

### shortcuts.json

Daftar shortcut, ditampilkan dari kiri ke kanan:

```json
[
  { "name": "GitHub", "target": "https://github.com" },
  { "name": "WhatsApp", "target": "whatsapp:" },
  { "name": "Notepad", "target": "C:\\Windows\\notepad.exe" },
  { "name": "Docs", "target": "https://example.com", "icon": "https://example.com/logo.png" }
]
```

- `name`: tampil sebagai tooltip.
- `target`: URL, protokol aplikasi (seperti `whatsapp:`), atau path ke program atau file. Dibuka dengan cara yang sama seperti Windows membukanya dari dialog Run.
- `icon` (opsional): URL gambar. Situs web memakai favicon-nya secara default; target lain menampilkan huruf pertama namanya.

Kalau isi file bukan JSON yang valid, daftar default yang ditampilkan sampai file-nya diperbaiki.

## Privasi dan keamanan

- **Discord**: komunikasi voice memakai IPC Discord lokal; pertukaran token OAuth menghubungi Discord melalui HTTPS. Token lama plaintext dimigrasikan ke format terenkripsi setelah berhasil disimpan.
- **Penyimpanan WebView**: tugas, pilihan kalender, dan tinggi panel Claude tersimpan lokal. File DPAPI hanya bisa dibuka oleh akun Windows yang sama.
- **Data lokal tetap di komputermu.** FloatingBar tidak punya telemetri dan tidak mengirim notifikasi, info media, atau percakapan Claude-mu ke mana pun. Integrasi kalender opsional menghubungi Google setelah kamu menghubungkan akun Google.
- **Permintaan jaringan**: ikon shortcut untuk situs web diambil dari layanan favicon Google (`https://www.google.com/s2/favicons?domain=…`), yang memberi tahu Google domain apa saja yang ada di shortcut-mu. Isi `icon` di shortcut untuk menghindarinya.
- **Notifikasi** dibaca dari notification center Windows selama FloatingBar berjalan dan hanya disimpan di memori; tidak ada yang ditulis ke disk.
- **Aplikasi Claude**: FloatingBar membaca pohon aksesibilitas jendela Claude secara lokal, yaitu label tombol sekitar setiap 1,5 detik untuk mendeteksi prompt izin, dan teks percakapan selama kamu memakai panel Claude. Mengirim prompt dengan gambar sempat memfokuskan jendela Claude dan memakai clipboard-mu.
- **Endpoint izin lokal**: FloatingBar mendengarkan di `http://127.0.0.1:47821` (hanya bisa diakses dari komputer ini) untuk hook Claude Code. Endpoint ini tanpa autentikasi: program lain di komputermu bisa mengirim permintaan dan memunculkan kartu, tapi tidak ada yang disetujui kecuali kamu menekan **Izinkan**, dan jawabannya hanya kembali ke pihak yang meminta.
- **Startup**: *Jalan saat Windows start* menambahkan entri di `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`; mematikannya menghapus entri itu.

## Keterbatasan

- Hanya untuk Windows.
- Fitur Claude bergantung pada label antarmuka berbahasa Inggris aplikasi Claude dan mungkin perlu diperbarui setelah aplikasinya berubah.
- Saat mengirim ke Claude di mode Chat, jeda panjang di tengah balasan bisa membuat FloatingBar menganggap balasannya sudah selesai. Buka lagi percakapannya (pilih dari daftar) untuk memuat sisanya.
- Volume browser mencakup semua tabnya (lihat [Volume](#volume)).
- Pengaturan kedipan taskbar di skrip opsional ditujukan untuk Windows 11 22H2 ke atas dan belum dipastikan di setiap build; kalau Windows memakai pengaturan lain, pengaturan ini tidak berefek. Cara manualnya ada di bagian tersebut.
- Sebagian aplikasi tidak menyediakan logo untuk notifikasinya; FloatingBar lalu menampilkan huruf pertama nama aplikasinya.
- Antarmukanya berbahasa Indonesia.

## Pemecahan masalah

**Discord belum terhubung padahal sudah di voice.**
Pastikan Discord desktop berjalan, isi pengaturan integrasi pada panel headset, lalu coba hubungkan kembali. Jika kredensial ditolak setelah reset secret, simpan secret baru dan autentikasi ulang. Izin RPC aplikasi Discord tetap diperlukan.

**Kalender akun lain tidak muncul.**
Kalender tersebut harus dibagikan/ditambahkan ke akun yang terhubung. Tekan **Perbarui izin Google** jika diminta, lalu centang sumber di **Pilih kalender**. Login akun lain di browser saja tidak menambah akses aplikasi.

**Notifikasi tidak muncul di pill.**
Pastikan akses notifikasi sudah diizinkan (lihat [Instalasi](#instalasi)); pill yang membesar menampilkan petunjuk oranye kalau belum. Notifikasi harus tetap menyala di Settings Windows, yang boleh dimatikan hanya banner-nya.

**Pill tidak muncul.**
Cek ikon tray: klik kiri untuk menampilkan pill, atau klik kanan → *Kembalikan ke tengah atas*. Kalau pill dipindah ke monitor yang sekarang tidak tersambung, pill kembali ke tengah atas saat start berikutnya.

**Membuka FloatingBar tidak terjadi apa-apa.**
Ada instance lain yang sudah berjalan (mungkin versi pengembangan); membukanya lagi hanya memunculkan instance itu. Keluarkan dulu dari tray.

**Pill kosong setelah Windows menyala.**
Autostart diaktifkan dari versi pengembangan, yang memuat antarmukanya dari dev server. Matikan autostart, pasang versi release, lalu aktifkan autostart dari sana.

**Panel Claude bilang aplikasinya tidak terbuka, atau daftarnya tidak sesuai aplikasi.**
Jalankan aplikasi Claude desktop lalu tekan **⟳**. Kalau masih gagal, mungkin tampilan aplikasinya berubah; silakan [buka issue](https://github.com/orkvacy/floatingbar/issues).

**Permintaan izin tidak sampai ke pill.**
Untuk Claude Code, periksa hook di `~/.claude/settings.json` (JSON valid, port `47821`) dan pastikan FloatingBar berjalan dan terlihat. Claude Code membaca pengaturan saat sesi dimulai, jadi mulai sesi baru setelah mengeditnya.

## Uninstall

1. Kalau kamu memakai skrip opsional, kembalikan pengaturan notifikasi dulu:
   ```powershell
   powershell -ExecutionPolicy Bypass -File scripts\quiet-windows-notifications.ps1 -Undo
   ```
2. Matikan *Jalan saat Windows start* di menu tray.
3. Hapus hook `PermissionRequest` dari `~/.claude/settings.json` kalau kamu menambahkannya.
4. Uninstall FloatingBar dari *Settings → Apps → Installed apps*.
5. Opsional: hapus `%APPDATA%\id.orkvacy.floatingbar` dan `%LOCALAPPDATA%\id.orkvacy.floatingbar`.

## Build dari source

### Prasyarat

1. **Microsoft C++ Build Tools** dengan workload *Desktop development with C++*:
   ```powershell
   winget install Microsoft.VisualStudio.2022.BuildTools --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
   ```
2. **Rust** lewat rustup:
   ```powershell
   winget install Rustlang.Rustup
   ```
3. **Bun**:
   ```powershell
   winget install Oven-sh.Bun
   ```

Buka terminal baru setelahnya supaya `cargo` dan `bun` terbaca.

### Perintah

```powershell
bun install            # pasang dependensi frontend
bun run tauri dev      # jalankan aplikasi desktop dengan hot reload (build Rust pertama agak lama)
bun run dev            # jalankan antarmukanya saja di browser, dengan data contoh
bun run tauri build    # buat installer
```

Installer-nya ada di `src-tauri\target\release\bundle\nsis\` (`.exe`) dan `src-tauri\target\release\bundle\msi\` (`.msi`).

Hanya satu FloatingBar yang bisa berjalan, jadi keluarkan versi yang terpasang sebelum menjalankan versi dev (dan sebaliknya).

## Cara kerja

```
src/                     antarmuka React
  App.tsx                pill, halaman, dan kartu
  lib/calendar.ts        Google Calendar state and sample agenda
  lib/discord.ts         Discord voice state and fictional demo members
  lib/todos.ts           local task storage
  lib/native.ts          hook yang berbicara dengan sisi Rust (dan data contoh di browser)
  icons.tsx, styles.css
src-tauri/src/           backend Rust
  lib.rs                 pengaturan jendela, hover tembus klik, posisi tersimpan, command
  calendar.rs            Google OAuth PKCE and selected-calendar agenda
  discord.rs             Discord IPC, OAuth, reconnect and voice controls
  placement.rs           work-area bounds and expansion direction
  secure_store.rs        Windows DPAPI encrypted storage
  media.rs               lagu yang diputar lewat Global System Media Transport Controls
  audio.rs               volume per aplikasi lewat sesi Core Audio
  battery.rs             baterai lewat GetSystemPowerStatus
  notify.rs              notifikasi lewat UserNotificationListener
  permission.rs          endpoint HTTP lokal untuk hook PermissionRequest Claude Code
  ui_permission.rs       prompt izin aplikasi Claude lewat UI Automation
  claude_app.rs          panel Claude: mode, percakapan, kirim, baca balasan
  ask.rs                 cadangan deep link claude://
  shortcuts.rs           shortcuts.json dan membuka target
  tray.rs                ikon tray, autostart, single instance
scripts/
  quiet-windows-notifications.ps1   saklar opsional banner notifikasi
  capture-screenshots.ps1           membuat ulang docs/screenshots dari mode demo
```

- **Jendela dan tembus klik.** Jendelanya transparan, tanpa bingkai, selalu di atas, dan lebih besar dari pill supaya pill bisa membesar. Secara default jendela mengabaikan mouse; thread Rust memantau posisi kursor dan hanya menerima input selama kursor berada di area pill yang dilaporkan antarmuka.
- **Polling.** Windows hanya mengirim event perubahan untuk API-API ini ke aplikasi ber-package, jadi FloatingBar melakukan polling: media setiap 0,7 detik, notifikasi setiap 0,8 detik, baterai setiap 3 detik, prompt Claude setiap 1,5 detik.
- **Pekerjaan UI Automation** berjalan di thread khusus yang memiliki objek COM-nya sendiri; antarmuka mengirim permintaan ke sana lewat channel.
- **Logo aplikasi ber-package** (seperti Claude) dibaca dari folder instalasi aplikasinya, karena membacanya lewat stream logo WinRT bisa macet.

## Berkontribusi

Issue dan pull request sangat diterima.

1. Fork dan clone repositorinya, lalu ikuti [Build dari source](#build-dari-source).
2. Pakai `bun run dev` untuk mengerjakan antarmuka: berjalan di browser biasa dengan data contoh. Coba `mockNotify()` dan `mockPermission()` di console browser, atau buka keadaan demo langsung dengan `?demo=` dan salah satu dari `compact`, `music`, `notifications`, `notification`, `permission`, `claude`, `todos`, `calendar`, `calendar-setup`, `discord`, atau `discord-compact` (misalnya `http://localhost:1420/?demo=claude`).
   Untuk memperbarui screenshot README, biarkan dev server berjalan lalu jalankan `powershell -ExecutionPolicy Bypass -File scripts\capture-screenshots.ps1`; hasilnya disimpan ke `docs/screenshots` memakai Edge headless.
3. Sebelum membuka pull request, pastikan `bun run build` (cek tipe + build) dan `cargo build --manifest-path src-tauri/Cargo.toml` berhasil.
4. Pesan commit mengikuti [Conventional Commits](https://www.conventionalcommits.org/), misalnya `feat(media): …` atau `fix(notifications): …`.

Kalau aplikasi Claude mengubah antarmukanya, label yang dicari FloatingBar dikumpulkan di bagian atas `src-tauri/src/claude_app.rs` dan di `src-tauri/src/ui_permission.rs`.

Panduan maintainer: [rilis dan pemeriksaan sebelum push](docs/RELEASING.md). Catatan rilis yang disiapkan: [v0.2.0](docs/releases/v0.2.0.md).

## Lisensi

FloatingBar adalah open source dengan [Lisensi MIT](LICENSE): kamu boleh memakai, mengubah, dan membagikannya, termasuk untuk keperluan komersial, asalkan pemberitahuan hak ciptanya tetap dicantumkan. Perangkat lunak ini diberikan tanpa garansi apa pun.
