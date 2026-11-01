# Integrasi AI di FloatingBar (preview)

Halaman ketiga, **AI**, menyediakan pilihan Claude desktop dan **ChatGPT / Codex**. Dukungan setiap aplikasi berbeda; nama sumber pada kartu izin mengikuti adapter yang menghasilkan permintaan.

| Sumber | Percakapan | Persetujuan |
| --- | --- | --- |
| Claude desktop / Claude Code | Panel Claude yang sudah ada: daftar, baca, kirim, gambar lewat clipboard | UI Automation Claude atau hook PermissionRequest |
| ChatGPT / Codex (App Server) | Daftar dengan pagination, baca 40 pesan terakhir, kirim, balasan streaming, hentikan turn | Izinkan sekali / Tolak untuk permintaan sesi FloatingBar |
| ChatGPT / Codex (sesi desktop yang sudah berjalan) | Belum dimirror melalui UI Automation | Deteksi eksperimental; Buka aplikasi |
| Antigravity desktop | Belum tersedia | Deteksi eksperimental; Buka Antigravity |

## Memakai ChatGPT / Codex

1. Pastikan Codex CLI terpasang dan sudah login. FloatingBar memakai akun/config CLI tersebut. Tidak ada kredensial Codex yang dibundel.
2. Buka halaman **AI**, pilih **ChatGPT / Codex**. Daftar yang dimuat adalah sesi Codex CLI melalui App Server resmi.
3. Pilih percakapan untuk membaca riwayat, atau pilih **Sesi ChatGPT / Codex baru**. Masukkan **Folder kerja** yang sudah ada untuk sesi baru.
4. Ketik prompt dan tekan Enter atau tombol kirim. Shift+Enter menambah baris.
5. Jika Codex meminta izin, kartu menampilkan tindakan yang diminta. Jawab **Izinkan** atau **Tolak**. Detail panjang dapat di-scroll.

Percakapan lama dibaca saja. **Pengiriman pertama dari percakapan lama membuat salinan sesi (fork) di FloatingBar**. Pengiriman berikutnya melanjutkan salinan tersebut selama koneksi yang sama aktif. Ini menjaga percakapan asli, termasuk sesi yang masih berjalan di desktop. Setelah koneksi terputus, pengiriman dapat membuat salinan lagi. Riwayat disimpan oleh Codex di penyimpanan normalnya, di luar repository.

Sesi FloatingBar dimulai dengan sandbox **read-only** dan kebijakan **on-request**. Akses tambahan memerlukan permintaan izin terpisah. Persetujuan perintah memakai keputusan sekali pakai; permintaan akses filesystem/network dibatasi pada turn terkait. FloatingBar tidak memilih izin permanen atau izin seluruh sesi. **Tolak** memakai `decline` jika tersedia, atau `cancel` yang menghentikan turn. Permintaan yang sudah selesai/kedaluwarsa tidak bisa dijawab ulang.

Integrasi App Server ini menangani sesi yang dijalankan lewat FloatingBar. Persetujuan sesi desktop/CLI lain yang sudah berjalan tidak otomatis dialihkan ke koneksi ini. Lampiran gambar masih tersedia pada adapter Claude; panel Codex saat ini menerima teks. Interaksi form, login, dan pertanyaan multiopsi belum didukung pada panel Codex; backend mengembalikan error untuk jenis interaksi yang tidak dikenali tanpa memberikan izin.

### Lokasi CLI

Pencarian mendukung binary npm Windows x64 dan `codex.exe` di PATH. Untuk instalasi lain, atur `FLOATINGBAR_CODEX_BIN` ke path absolut `codex.exe`, lalu buka ulang FloatingBar. Jangan mengisi variabel tersebut dengan credential atau command shell.

### Jika balasan/perintah gagal

Error ditampilkan di panel dan draft tetap tersedia jika pengiriman gagal. Setelah timeout, muat ulang sebelum mengirim lagi karena operasi mungkin sudah berjalan. Gunakan **Hentikan balasan Codex** untuk turn milik FloatingBar. Izin sekali pakai tidak menghapus batas sandbox; perintah masih dapat ditolak oleh lingkungan eksekusi. Contohnya, mode PowerShell terbatas menolak pemanggilan metode .NET tertentu.

## Adapter desktop eksperimental

Default **mati**. Buka **AI → Pengaturan AI** untuk mengaktifkan deteksi desktop eksperimental. Preferensi disimpan lokal. Ini tidak mematikan Claude atau sesi Codex App Server yang dibuat melalui FloatingBar.

Pencocokan memakai executable dan identitas paket Windows. Codex yang memakai `ChatGPT.exe` dikenali melalui paket `OpenAI.Codex_*` dengan `GetApplicationUserModelId`. Nama proses/judul jendela saja tidak membuktikan adanya prompt.

UI Automation mencari pasangan tombol pada container yang berdekatan (maksimal lima ancestor, tidak memakai seluruh jendela), dengan konteks permintaan izin dan tombol yang terlihat/aktif. Sebelum menjawab Claude, identitas kedua elemen, sumber, window handle, dan teks dipindai ulang. Jawaban terlambat ditolak. Adapter baru tidak menyediakan jawaban langsung; aktifkan hanya setelah diuji pada prompt asli.

Versi browser, jendela yang diminimalkan/elevated, bahasa/versi UI berbeda, dan nama executable lain bisa tidak terdeteksi. Antigravity pada pemeriksaan asli mengekspos daftar percakapan, Message input, dan Send message, tetapi adapter chat-nya belum diimplementasikan. Nama **ChatGPT / Codex** menyatukan label host yang terdeteksi pada instalasi ini. Jalur yang diuji tetap Codex App Server; riwayat chat ChatGPT biasa tidak otomatis dimuat oleh API tersebut.

## Verifikasi pada 1 November 2026

Tes langsung memakai CLI/akun Codex yang terpasang dan transport produksi `codex_rpc.rs`. Sesi model untuk tes dibuat **ephemeral** di folder target yang diabaikan Git. Judul, ID, dan isi chat pribadi tidak dimasukkan ke dokumentasi atau screenshot.

- Handshake, daftar percakapan, dan baca riwayat: lulus.
- Filter sumber, fork ephemeral, dan pembacaan metadata salinan: lulus. API tidak mendukung includeTurns pada sesi ephemeral; riwayat percakapan tersimpan diuji terpisah.
- Kirim prompt teks dan menerima balasan streaming dengan marker yang ditentukan: lulus.
- Permintaan izin nyata untuk percobaan menulis file: diterima dan ditolak; file tidak dibuat.
- Permintaan izin nyata untuk `Write-Output` yang diperiksa: disetujui sekali, menghasilkan marker yang tepat, status completed dan exit code 0.
- Fixture stdio: tidak ada persetujuan otomatis, ID string tetap utuh, ID kedaluwarsa ditolak, disconnect diteruskan, serta penolakan cancel-only ditangani.
- Parsing riwayat: pesan tool/reasoning tidak ditampilkan, completion mengganti delta tanpa duplikasi, dan jumlah pesan dibatasi.
- UI browser demo: memilih Codex/Claude dan mengirim pesan demo lulus. Ini pengujian UI terpisah dari tes backend langsung; belum berupa pengujian klik native end-to-end pada installer.

Hasil tersebut membuktikan alur yang diuji pada lingkungan ini, tidak menjamin semua model, tool, atau versi aplikasi lain.

## Menjalankan tes tanpa model

```powershell
cargo build --manifest-path src-tauri/Cargo.toml
powershell -ExecutionPolicy Bypass -File scripts/test-codex.ps1
bun run build
```

Tes memakai peer stdio fixture dan tidak membaca chat pribadi atau menghubungi model. UI demo memakai data fiktif:

- `http://localhost:1420/?demo=codex`
- `http://localhost:1420/?demo=permission&provider=codex-rpc`
- `http://localhost:1420/?demo=permission&provider=chatgpt`
- `http://localhost:1420/?demo=permission&queue=claude-code,codex,antigravity`

## Rujukan

[OpenAI Docs: App Server](https://learn.chatgpt.com/docs/app-server) menjelaskan thread, turn, streaming, dan persetujuan. Implementasi juga diperiksa terhadap schema yang dihasilkan CLI terpasang. [Microsoft Learn: GetApplicationUserModelId](https://learn.microsoft.com/en-us/windows/win32/api/appmodel/nf-appmodel-getapplicationusermodelid) menjelaskan identitas proses. [Google Antigravity: Agent Settings](https://antigravity.google/docs/agent-settings) menjelaskan pengaturan review; FloatingBar tidak mengubahnya.
