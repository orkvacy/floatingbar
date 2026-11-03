import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { isTauri } from "../lib/native";

type Topic = "calendar" | "discord" | "codex";
type Step = { title: string; body: ReactNode };
const LINKS = {
  "google-console": "https://console.cloud.google.com/",
  "discord-portal": "https://discord.com/developers/applications",
  "codex-docs": "https://developers.openai.com/codex/cli",
};

function HelpLink({ link, children }: { link: keyof typeof LINKS; children: ReactNode }) {
  const [error, setError] = useState("");
  return <><button type="button" className="connection-help__link" onClick={() => {
    setError("");
    if (isTauri) void invoke("open_help_link", { link }).catch(() => setError(`Buka alamat ini di browser: ${LINKS[link]}`));
    else window.open(LINKS[link], "_blank", "noopener,noreferrer");
  }}>{children} ↗</button>{error && <p role="alert">{error}</p>}</>;
}

const GUIDES: Record<Topic, { name: string; steps: Step[] }> = {
  calendar: { name: "Google Calendar", steps: [
    { title: "Siapkan file koneksi", body: <><p>Koneksi kalender masih memerlukan pengaturan Google satu kali. Tidak perlu menulis kode.</p><p>Sudah punya file kredensial JSON bertipe <strong>Desktop app</strong>? Langsung pilih langkah <strong>4. Hubungkan akun</strong> di bawah.</p><p>Belum punya? Siapkan akun Google yang ingin dipakai, lalu ikuti langkah 2–3. File JSON adalah file pengenal aplikasi, bukan file ekspor kalender (.ics).</p></> },
    { title: "Siapkan proyek Google", body: <><ol><li><HelpLink link="google-console">Buka Google Cloud Console</HelpLink>, lalu login dengan akun Google Anda.</li><li>Klik pemilih proyek di bagian atas → <strong>New project / Proyek baru</strong>. Beri nama, misalnya <strong>FloatingBar Pribadi</strong>, lalu pilih proyek tersebut.</li><li>Buka <strong>APIs &amp; Services → Library</strong>, cari <strong>Google Calendar API</strong>, lalu klik <strong>Enable / Aktifkan</strong>.</li></ol><p>Jika akun kampus/kantor melarang pembuatan proyek, minta bantuan administrator atau gunakan akun pribadi yang diizinkan.</p></> },
    { title: "Unduh file JSON", body: <><ol><li>Buka <strong>Google Auth platform → Branding → Get started</strong>. Isi nama aplikasi, email dukungan, dan email kontak.</li><li>Untuk akun pribadi, pilih <strong>External</strong> pada Audience. Saat statusnya <strong>Testing</strong>, tambahkan email akun yang akan login ke <strong>Audience → Test users</strong>.</li><li>Di <strong>Data Access</strong>, tambahkan izin <code>calendar.events.readonly</code> dan <code>calendar.calendarlist.readonly</code> jika diminta menyiapkan scopes.</li><li>Buka <strong>Clients → Create client → Desktop app</strong>. Beri nama, klik Create, lalu unduh file JSON dari detail client.</li></ol><p>Nama menu Google dapat berbeda menurut bahasa. Simpan file ini di perangkat Anda; jangan unggah ke GitHub atau kirim lewat chat.</p></> },
    { title: "Hubungkan akun", body: <><ol><li>Tutup panduan ini, lalu klik <strong>Hubungkan Google Calendar</strong> jika formulir belum terbuka.</li><li>Klik <strong>Pilih file kredensial (.json)</strong> dan pilih file dari langkah 3.</li><li>Di browser yang terbuka, pilih akun yang tercantum sebagai test user, periksa nama aplikasi, lalu setujui akses baca kalender.</li><li>Kembali ke FloatingBar. Koneksi berhasil jika muncul <strong>Google Calendar</strong> dan menu <strong>Pilih kalender</strong>.</li></ol><p>Jika muncul peringatan aplikasi belum diverifikasi, periksa proyek dan pemiliknya terlebih dahulu. Jangan lanjutkan untuk aplikasi yang tidak Anda kenal. Panduan ini tidak mengubah atau menghapus acara.</p></> },
    { title: "Pilih agenda & cari bantuan", body: <><ol><li>Buka <strong>Pilih kalender</strong> dan centang kalender yang ingin ditampilkan.</li><li>Untuk akun lain, bagikan kalender dari akun pemilik ke akun yang terhubung, terima undangannya, lalu klik <strong>Perbarui agenda</strong>.</li></ol><details><summary>Agenda kosong / kalender tidak muncul</summary><p>FloatingBar menampilkan acara hari ini saja. Periksa tanggal acara, kalender yang dicentang, serta akun yang login. Jika ada tombol <strong>Perbarui izin Google</strong>, gunakan tombol tersebut untuk memberi akses baca daftar kalender.</p><p>Berpindah akun di situs Google Calendar saja tidak otomatis memberi FloatingBar akses ke akun kedua. Gunakan berbagi kalender atau <strong>Kelola koneksi → Hubungkan ulang / ganti akun</strong>.</p></details><details><summary>Login gagal / izin kedaluwarsa</summary><p>Pastikan API aktif, client bertipe Desktop app, dan email login tercantum di Test users untuk proyek Testing. Aplikasi Testing dapat meminta login ulang. Gunakan <strong>Kelola koneksi → Hubungkan ulang / ganti akun</strong>.</p><p>Login dibatalkan? Tekan <strong>Batalkan login</strong> jika masih menunggu, lalu coba lagi. Akun kampus/kantor dapat memerlukan izin administrator.</p></details></> },
  ] },
  discord: { name: "Discord", steps: [
    { title: "Siapkan Discord desktop", body: <><p>Buka aplikasi Discord di Windows, login, lalu masuk voice channel. Discord yang hanya terbuka di browser tidak menyediakan koneksi desktop ini.</p><p>Integrasi memerlukan Client ID dan Client secret aplikasi Discord Anda sendiri. Jika belum punya, lanjutkan ke langkah berikutnya.</p></> },
    { title: "Siapkan kredensial aplikasi", body: <><ol><li><HelpLink link="discord-portal">Buka Discord Developer Portal</HelpLink> dan pilih aplikasi Anda, atau buat aplikasi baru.</li><li>Di bagian <strong>OAuth2</strong>, temukan Client ID dan Client secret. Keduanya harus berasal dari aplikasi yang sama.</li><li>Jika akses RPC masih dibatasi, akun yang menguji perlu terdaftar sebagai tester aplikasi. Ketersediaan akses ditentukan Discord.</li></ol><p>Gunakan <strong>Client secret</strong>, bukan password Discord atau bot token. Jangan kirim secret ke orang lain atau masukkan ke repository.</p></> },
    { title: "Hubungkan dan periksa", body: <><ol><li>Tutup panduan, buka <strong>Pengaturan integrasi Discord</strong>, lalu isi Client ID dan Client secret.</li><li>Klik <strong>Simpan dan hubungkan</strong>, lalu periksa dan setujui permintaan otorisasi di Discord.</li><li>Saat berada di voice channel, nama channel dan anggota akan muncul di FloatingBar.</li></ol><p>Belum terhubung? Pastikan Discord desktop masih berjalan, kredensial sesuai, dan akses RPC/tester tersedia. Setelah mereset secret, simpan penggantinya di pengaturan yang sama.</p></> },
  ] },
  codex: { name: "ChatGPT / Codex", steps: [
    { title: "Siapkan Codex CLI", body: <><p>Panel ini memakai <strong>Codex CLI</strong>, yaitu program Codex yang dipasang di Windows. Login pada situs ChatGPT saja belum cukup.</p><p><HelpLink link="codex-docs">Buka panduan instalasi resmi Codex</HelpLink>. Pasang CLI di Windows, jalankan <code>codex</code>, lalu ikuti proses login. Jika belum terbiasa dengan terminal, minta bantuan untuk tahap instalasi ini.</p></> },
    { title: "Mulai percakapan", body: <><ol><li>Setelah CLI terpasang dan login, buka ulang FloatingBar.</li><li>Buka halaman <strong>AI → ChatGPT / Codex</strong>, lalu klik <strong>Muat ulang Codex</strong>.</li><li>Pilih percakapan atau sesi baru. Untuk sesi baru, isi <strong>Folder kerja</strong> dengan alamat folder yang sudah ada di komputer.</li><li>Ketik prompt dan kirim. Balasan tampil di panel; permintaan izin yang didukung muncul sebagai kartu Izinkan/Tolak.</li></ol><p>Pengiriman dari percakapan lama membuat salinan sesi. Riwayat ChatGPT biasa dan izin dari sesi desktop lain tidak otomatis dimuat.</p></> },
    { title: "Jika belum tersambung", body: <><p>Coba jalankan Codex CLI dan pastikan bisa login. Jika CLI baru dipasang, tutup dan buka kembali FloatingBar agar lokasi program terbaca.</p><p>Daftar kosong belum tentu gagal: Anda dapat memulai sesi baru. Untuk lokasi CLI khusus, pengaturan <code>FLOATINGBAR_CODEX_BIN</code> dijelaskan di README. Jangan isi variabel tersebut dengan password atau token.</p><p>Fitur ini masih menerima prompt teks. Lampiran gambar tersedia pada panel Claude.</p></> },
  ] },
};

export function ConnectionHelp({ topic, onOpenChange }: { topic: Topic; onOpenChange?: (open: boolean) => void }) {
  const [open, setOpen] = useState(false);
  const [step, setStep] = useState(0);
  const id = useId();
  const content = useRef<HTMLDivElement>(null);
  const guide = GUIDES[topic];
  useEffect(() => {
    onOpenChange?.(open);
    return () => onOpenChange?.(false);
  }, [open, onOpenChange]);
  useEffect(() => { if (open) { content.current?.scrollTo(0, 0); content.current?.focus(); } }, [step, open]);
  return <div className="connection-help" data-no-swipe>
    <button type="button" className="connection-help__toggle" aria-expanded={open} aria-controls={id} onClick={() => setOpen(!open)}>{open ? "Tutup panduan" : `Panduan koneksi ${guide.name}`}</button>
    {open && <section id={id} className="connection-help__panel" aria-label={`Panduan ${guide.name}`}>
      <div className="connection-help__jump"><label htmlFor={`${id}-step`}>Pilih langkah</label><select id={`${id}-step`} value={step} onChange={(e) => setStep(Number(e.target.value))}>{guide.steps.map((s, i) => <option key={s.title} value={i}>{i + 1}. {s.title}</option>)}</select></div>
      <div className="connection-help__content" ref={content} tabIndex={0} aria-label={`Langkah ${step + 1}: ${guide.steps[step].title}`}>
        <h3>{guide.steps[step].title}</h3>{guide.steps[step].body}
      </div>
      <nav aria-label="Langkah panduan"><button type="button" disabled={step === 0} onClick={() => setStep(step - 1)}>Sebelumnya</button><span aria-live="polite">{step + 1} / {guide.steps.length}</span>{step < guide.steps.length - 1 ? <button type="button" onClick={() => setStep(step + 1)}>Berikutnya</button> : <button type="button" onClick={() => setOpen(false)}>Selesai</button>}</nav>
    </section>}
  </div>;
}
