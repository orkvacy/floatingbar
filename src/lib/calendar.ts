import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { demo, isTauri } from "./native";

export type CalendarEvent = {
  id: string; title: string; start: string; end: string; allDay: boolean;
  location: string | null; url: string | null;
  calendarId: string; calendarName: string; calendarColor: string;
};
export type CalendarSource = { id: string; name: string; color: string; selected: boolean; primary: boolean };
type CalendarAgenda = { events: CalendarEvent[]; calendars: CalendarSource[]; warnings: string[]; needsReconnect: boolean };
const SELECTION_KEY = "floatingbar.calendar.selection.v1";
const SAMPLE_CALENDARS: CalendarSource[] = [
  { id: "classes", name: "Kelas reg", color: "#b19cd9", selected: true, primary: false },
  { id: "primary", name: "Pribadi", color: "#81c995", selected: true, primary: true },
];

export function todayRange(now = new Date()) {
  return {
    start: new Date(now.getFullYear(), now.getMonth(), now.getDate()).toISOString(),
    end: new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1).toISOString(),
  };
}

function sampleEvents(): CalendarEvent[] {
  const at = (hour: number) => {
    const date = new Date(); date.setHours(hour, 0, 0, 0); return date.toISOString();
  };
  return [
    { id: "demo-1", title: "Kelas pemrograman", start: at(9), end: at(11), allDay: false, location: "Ruang belajar", url: null, calendarId: "classes", calendarName: "Kelas reg", calendarColor: "#b19cd9" },
    { id: "demo-2", title: "Diskusi tim", start: at(14), end: at(15), allDay: false, location: "Google Meet", url: null, calendarId: "primary", calendarName: "Pribadi", calendarColor: "#81c995" },
    { id: "demo-3", title: "Review desain aplikasi", start: at(19), end: at(20), allDay: false, location: null, url: null, calendarId: "primary", calendarName: "Pribadi", calendarColor: "#81c995" },
  ];
}

export async function openCalendarLink(url: string) {
  if (isTauri) await invoke("calendar_open", { url });
  else window.open(url, "_blank", "noopener,noreferrer");
}

export function useCalendar(active: boolean) {
  const [connected, setConnected] = useState(demo === "calendar");
  const [checking, setChecking] = useState(isTauri);
  const [connecting, setConnecting] = useState(false);
  const [loading, setLoading] = useState(false);
  const [events, setEvents] = useState<CalendarEvent[]>(demo === "calendar" ? sampleEvents : []);
  const [error, setError] = useState<string | null>(null);
  const [updatedAt, setUpdatedAt] = useState<Date | null>(null);
  const [calendars, setCalendars] = useState<CalendarSource[]>(demo === "calendar" ? SAMPLE_CALENDARS : []);
  const [warnings, setWarnings] = useState<string[]>([]);
  const [needsReconnect, setNeedsReconnect] = useState(false);
  const [selectedIds, setSelectedIds] = useState<string[] | null>(() => {
    if (demo) return null;
    try {
      const ids: unknown = JSON.parse(localStorage.getItem(SELECTION_KEY) ?? "null");
      return Array.isArray(ids) && ids.length <= 2500 && ids.every((id) => typeof id === "string") ? ids : null;
    } catch { return null; }
  });
  const eventsDay = useRef(todayRange().start);
  const request = useRef(0);
  const pending = useRef(false);
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; request.current += 1; };
  }, []);

  useEffect(() => {
    if (!isTauri) return;
    let active = true;
    invoke<boolean>("calendar_status").then((value) => { if (active) setConnected(value); })
      .catch((e) => { if (active) setError(String(e)); })
      .finally(() => { if (active) setChecking(false); });
    return () => { active = false; };
  }, []);

  const refresh = useCallback(async () => {
    if (!isTauri || !connected || pending.current) return;
    const id = ++request.current;
    pending.current = true;
    const range = todayRange();
    setLoading(true);
    setError(null);
    // A previous day's cached events must not appear under today's heading.
    if (eventsDay.current !== range.start) { setEvents([]); setUpdatedAt(null); }
    eventsDay.current = range.start;
    try {
      const agenda = await invoke<CalendarAgenda>("calendar_events", { ...range, calendarIds: selectedIds });
      if (mounted.current && id === request.current) {
        setEvents(agenda.events); setCalendars(agenda.calendars); setWarnings(agenda.warnings);
        setNeedsReconnect(agenda.needsReconnect); setUpdatedAt(new Date());
      }
    } catch (e) {
      if (mounted.current && id === request.current) setError(String(e));
    } finally {
      pending.current = false;
      if (mounted.current && id === request.current) setLoading(false);
    }
  }, [connected, selectedIds]);

  useEffect(() => {
    if (!active || !connected || connecting) return;
    void refresh();
    const timer = setInterval(() => { if (!document.hidden) void refresh(); }, 60_000);
    const onVisible = () => { if (!document.hidden) void refresh(); };
    document.addEventListener("visibilitychange", onVisible);
    return () => { clearInterval(timer); document.removeEventListener("visibilitychange", onVisible); };
  }, [active, connected, connecting, refresh]);

  const connect = async (json: string | null) => {
    if (!isTauri) { setError("Hubungkan akun melalui aplikasi desktop FloatingBar."); return; }
    setConnecting(true); setError(null);
    try {
      await invoke("calendar_connect", { credentialsJson: json });
      if (mounted.current) {
        setConnected(true); setEvents([]); setUpdatedAt(null); setSelectedIds(null);
        setNeedsReconnect(false); setWarnings([]);
        try { localStorage.removeItem(SELECTION_KEY); } catch { /* Selection is also reset in memory. */ }
      }
    } catch (e) { if (mounted.current) setError(String(e)); }
    finally { if (mounted.current) setConnecting(false); }
  };

  return {
    connected, checking, connecting, loading, error, updatedAt, warnings, needsReconnect,
    calendars: demo === "calendar" ? calendars.map((c) => ({ ...c, selected: selectedIds === null || selectedIds.includes(c.id) })) : calendars,
    events: eventsDay.current === todayRange().start ? events.filter((e) => !demo || selectedIds === null || selectedIds.includes(e.calendarId)) : [],
    refresh, connect,
    selectCalendar(id: string, selected: boolean) {
      if (pending.current || needsReconnect) return;
      const chosen = selectedIds ?? calendars.filter((c) => c.selected).map((c) => c.id);
      const next = selected ? [...new Set([...chosen, id])] : chosen.filter((value) => value !== id);
      if (!demo) {
        try { localStorage.setItem(SELECTION_KEY, JSON.stringify(next)); }
        catch { setError("Pilihan kalender hanya tersimpan untuk sesi ini."); }
        setEvents([]); setUpdatedAt(null);
      }
      setCalendars((list) => list.map((c) => ({ ...c, selected: next.includes(c.id) })));
      setSelectedIds(next);
    },
    reportError: (error: unknown) => setError(String(error)),
    cancel: () => { void invoke("calendar_cancel").catch((e) => setError(String(e))); },
    async disconnect() {
      request.current += 1;
      try {
        if (isTauri) await invoke("calendar_disconnect");
        if (mounted.current) { setConnected(false); setEvents([]); setCalendars([]); setWarnings([]); setNeedsReconnect(false); setUpdatedAt(null); setError(null); setLoading(false); }
      } catch (e) { if (mounted.current) setError(String(e)); }
    },
  };
}
