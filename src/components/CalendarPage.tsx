import { useEffect, useState } from "react";
import { CalendarIcon, RefreshIcon } from "../icons";
import { demo } from "../lib/native";
import { openCalendarLink, type CalendarEvent, type useCalendar } from "../lib/calendar";

const time = (date: string) => new Date(date).toLocaleTimeString("id-ID", { hour: "2-digit", minute: "2-digit" }).replace(".", ":");

export function CalendarPage({ calendar, now, onTyping }: {
  calendar: ReturnType<typeof useCalendar>; now: Date; onTyping: (value: boolean) => void;
}) {
  const [showSetup, setShowSetup] = useState(demo === "calendar-setup");
  const [json, setJson] = useState("");
  // Keep the input mounted while Windows' file picker or Google login is open.
  useEffect(() => {
    onTyping(showSetup && !calendar.connected && !calendar.connecting);
    return () => onTyping(false);
  }, [showSetup, calendar.connected, calendar.connecting, onTyping]);
  const open = (url: string) => { void openCalendarLink(url).catch(calendar.reportError); };
  const connect = () => { const credentials = json; setJson(""); void calendar.connect(credentials); };
  const date = now.toLocaleDateString("id-ID", { weekday: "long", day: "numeric", month: "long" });
  return (
    <section className="organizer agenda" aria-label="Agenda hari ini">
      <div className="organizer__head">
        <div><h2><CalendarIcon />Agenda hari ini</h2><p>{date}</p></div>
        {calendar.connected && <button className="organizer__icon" onClick={() => void calendar.refresh()} disabled={calendar.loading || calendar.connecting} aria-label="Perbarui agenda" title="Perbarui agenda"><RefreshIcon className={calendar.loading ? "organizer__spin" : ""} /></button>}
      </div>
      {calendar.error && <p className="organizer__error" role="alert">{calendar.error}</p>}
      {calendar.checking ? <p className="organizer__empty" role="status">Memeriksa koneksi kalender…</p> : calendar.connecting ? (
        <div className="organizer__empty" role="status"><CalendarIcon /><strong>Lanjutkan di browser</strong><span>Pilih akun Google dan izinkan akses baca kalender. Panel ini akan diperbarui setelah login.</span><button className="organizer__text" onClick={calendar.cancel}>Batalkan login</button></div>
      ) : !calendar.connected ? (
        <>
          {!showSetup && <div className="agenda__welcome"><CalendarIcon /><strong>Harimu, dalam satu pandangan</strong><p>Lihat agenda dari kalender pribadi, kelas, dan kalender yang dibagikan ke akunmu.</p></div>}
          {!showSetup ? <button className="organizer__primary agenda__connect" onClick={() => setShowSetup(true)}>Hubungkan Google Calendar</button> : (
            <div className="agenda__setup">
              <details><summary>Setup Google satu kali</summary><ol>
                <li>Buka <button className="organizer__text" onClick={() => open("https://console.cloud.google.com/apis/library/calendar-json.googleapis.com")}>Google Cloud</button> dan aktifkan Google Calendar API.</li>
                <li>Atur OAuth consent screen. Jika masih Testing, tambahkan emailmu sebagai test user.</li>
                <li>Buat OAuth client bertipe <strong>Desktop app</strong>, lalu unduh JSON kredensial.</li>
              </ol></details>
              <label className="agenda__upload">Pilih file kredensial (.json)<input type="file" accept=".json,application/json" onChange={async (e) => {
                const file = e.target.files?.[0]; e.target.value = "";
                if (!file) return;
                if (file.size > 64 * 1024) { calendar.reportError("File kredensial maksimal 64 KB."); return; }
                try { await calendar.connect(await file.text()); } catch (err) { calendar.reportError(err); }
              }} /></label>
              <details><summary>Atau tempel JSON kredensial</summary>
                <textarea aria-label="JSON kredensial OAuth Desktop app" placeholder={'{"installed": {"client_id": "…", "client_secret": "…"}}'} value={json} onChange={(e) => setJson(e.target.value)} spellCheck={false} />
                <button className="organizer__primary" disabled={!json.trim()} onClick={connect}>Lanjutkan ke Google</button>
              </details>
              <button className="organizer__text" onClick={() => { setShowSetup(false); setJson(""); }}>Kembali</button>
            </div>
          )}
          <p className="agenda__privacy">Izin baca saja · Token dilindungi akun Windows</p>
        </>
      ) : (
        <>
          <div className="agenda__source"><span className="agenda__dot" />Google Calendar<span>{calendar.calendars.filter((c) => c.selected).length} kalender dipilih</span></div>
          {calendar.needsReconnect && <div className="agenda__consent"><p>Koneksi lama baru membaca kalender utama. Izinkan akses daftar kalender untuk menampilkan kalender kelas dan kalender yang dibagikan.</p><button className="organizer__primary" disabled={calendar.loading} onClick={() => void calendar.connect(null)}>Perbarui izin Google</button></div>}
          {calendar.warnings.length > 0 && <p className="organizer__error" role="status">Sebagian agenda belum dimuat: {calendar.warnings.join("; ")}</p>}
          {!calendar.needsReconnect && calendar.calendars.length > 0 && <details className="agenda__calendars"><summary>Pilih kalender</summary><div className="agenda__calendar-list">{calendar.calendars.map((source) => <label key={source.id}><input type="checkbox" checked={source.selected} disabled={calendar.loading} onChange={(e) => calendar.selectCalendar(source.id, e.target.checked)} /><span className="agenda__calendar-dot" style={{ backgroundColor: source.color }} /><span>{source.name}{source.primary && <small> Utama</small>}</span></label>)}</div><p>Pilihan hanya berlaku di FloatingBar. Akun lain harus membagikan kalendernya ke akun yang terhubung.</p></details>}
          {calendar.loading && calendar.events.length === 0 ? <p className="organizer__empty" role="status">Memuat agenda hari ini…</p> : calendar.events.length === 0 ? (
            <div className="organizer__empty"><CalendarIcon /><strong>{calendar.error ? "Agenda belum tersedia" : "Hari ini masih lapang"}</strong><span>{calendar.error ? "Coba perbarui agenda setelah koneksi pulih." : calendar.calendars.some((c) => c.selected) ? "Belum ada acara di kalender yang dipilih hari ini." : "Pilih setidaknya satu kalender untuk melihat agenda."}</span></div>
          ) : <ul className="organizer__list agenda__list">{calendar.events.map((event) => <AgendaRow key={event.id} event={event} now={now} open={open} />)}</ul>}
          <div className="organizer__footer"><span>{calendar.updatedAt ? `Diperbarui ${time(calendar.updatedAt.toISOString())}` : "Belum disinkronkan"}</span><button className="organizer__text" onClick={() => open("https://calendar.google.com/calendar/u/0/r/day")}>Buka kalender ↗</button></div>
          <details className="agenda__account"><summary>Kelola koneksi</summary><p>Putuskan koneksi untuk menghapus token dari perangkat ini.</p><button className="organizer__text" disabled={calendar.loading} onClick={() => void calendar.connect(null)}>Hubungkan ulang / ganti akun</button><button className="organizer__text" disabled={calendar.loading} onClick={() => void calendar.disconnect()}>Putuskan koneksi</button><button className="organizer__text" onClick={() => open("https://myaccount.google.com/connections")}>Kelola izin di Google ↗</button></details>
        </>
      )}
    </section>
  );
}

function AgendaRow({ event, now, open }: { event: CalendarEvent; now: Date; open: (url: string) => void }) {
  const start = new Date(event.start).getTime();
  const end = new Date(event.end).getTime();
  const ongoing = !event.allDay && start <= now.getTime() && now.getTime() < end;
  const past = !event.allDay && end <= now.getTime();
  return (
    <li className={`agenda__event ${ongoing ? "agenda__event--now" : ""} ${past ? "agenda__event--past" : ""}`}>
      <div className="agenda__time">{event.allDay ? <span>Seharian</span> : <><strong>{time(event.start)}</strong><span>{time(event.end)}</span></>}</div>
      <div className="agenda__detail">{ongoing && <span className="agenda__live">Berlangsung</span>}{event.url ? <button className="agenda__title" onClick={() => open(event.url!)} title="Buka acara di Google Calendar">{event.title}<span aria-hidden="true"> ↗</span></button> : <strong className="agenda__title">{event.title}</strong>}<span className="agenda__event-source"><span className="agenda__calendar-dot" style={{ backgroundColor: event.calendarColor }} />{event.calendarName}</span>{event.location && <span className="agenda__location">{event.location}</span>}{past && <span className="agenda__status">Selesai</span>}</div>
    </li>
  );
}
