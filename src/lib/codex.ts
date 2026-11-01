import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState } from "react";
import { demo, isTauri } from "./native";

type Thread = { id: string; name?: string; preview: string; cwd?: string };
export type CodexHistory = { threadId: string; messages: { id: string; role: string; text: string }[]; working: boolean; error: string | null };
const fixture: CodexHistory = { threadId: "demo-thread", messages: [
  { id: "user", role: "user", text: "Bantu jelaskan struktur project ini." },
  { id: "assistant", role: "assistant", text: "Folder src berisi antarmuka, sedangkan src-tauri berisi integrasi desktop dan backend Rust." },
], working: false, error: null };

export function useCodex() {
  const [threads, setThreads] = useState<Thread[]>(isTauri ? [] : [{ id: "demo-thread", name: "Review struktur project", preview: "Review struktur project" }]);
  const [cursor, setCursor] = useState<string | null>(null);
  const [selected, setSelected] = useState<string | null>(demo === "codex" ? "demo-thread" : null);
  const selectedRef = useRef(selected);
  const records = useRef(new Map<string, CodexHistory>());
  const [history, setHistory] = useState<CodexHistory | null>(demo === "codex" ? fixture : null);
  const [cwd, setCwd] = useState(demo === "codex" ? "D:\\projects\\my-app" : "");
  const [owned, setOwned] = useState(false);
  const [draft, setDraft] = useState("");
  const [loading, setLoading] = useState(false);
  const [sending, setSending] = useState(false);
  const pending = useRef(false);
  const selectionVersion = useRef(0);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    if (!isTauri) return;
    let stopped = false;
    const listener = listen<CodexHistory>("codex-history", (e) => {
      if (stopped) return;
      records.current.set(e.payload.threadId, e.payload);
      if (records.current.size > 80) {
        const oldest = records.current.keys().next().value;
        if (oldest && oldest !== selectedRef.current) records.current.delete(oldest);
      }
      if (e.payload.threadId === selectedRef.current) setHistory(e.payload);
    });
    return () => { stopped = true; listener.then((f) => f()); };
  }, []);
  const refresh = useCallback(async (more = false) => {
    if (!isTauri) return;
    setLoading(true); setError(null);
    try {
      const data = await invoke<{ data: Thread[]; nextCursor: string | null }>("codex_threads", { cursor: more ? cursor : null });
      setThreads((old) => more ? [...old, ...data.data.filter((t) => !old.some((o) => o.id === t.id))] : [...data.data, ...old.filter((t) => t.id === selectedRef.current && !data.data.some((n) => n.id === t.id))]);
      setCursor(data.nextCursor);
    } catch (e) { setError(String(e)); }
    finally { setLoading(false); }
  }, [cursor]);
  const select = useCallback(async (id: string | null) => {
    if (pending.current) return;
    const version = ++selectionVersion.current;
    selectedRef.current = id; setSelected(id); setError(null); setOwned(false);
    setHistory(id ? records.current.get(id) ?? null : null);
    if (!id) return;
    if (!isTauri) { setHistory(fixture); return; }
    setLoading(true);
    try {
      const data = await invoke<{ history: CodexHistory; cwd: string; owned: boolean }>("codex_read", { threadId: id });
      if (version !== selectionVersion.current) return;
      const stream = records.current.get(id);
      setHistory(stream?.working ? stream : data.history);
      setCwd(data.cwd || ""); setOwned(data.owned);
    } catch (e) { if (version === selectionVersion.current) setError(String(e)); }
    finally { if (version === selectionVersion.current) setLoading(false); }
  }, []);
  const send = useCallback(async () => {
    const prompt = draft.trim();
    if (!prompt || pending.current || history?.working || loading) return;
    pending.current = true; setSending(true); setError(null);
    try {
      if (!isTauri) {
        setHistory({ threadId: "demo-thread", messages: [...(history?.messages ?? []), { id: String(Date.now()), role: "user", text: prompt }, { id: "demo-answer", role: "assistant", text: "Ini balasan demo; tidak ada permintaan yang dikirim ke Codex." }], working: false, error: null });
      } else {
        const id = await invoke<string>("codex_send", { threadId: selected, cwd, prompt });
        ++selectionVersion.current;
        selectedRef.current = id; setSelected(id); setOwned(true);
        setHistory(records.current.get(id) ?? { threadId: id, messages: [], working: true, error: null });
        setThreads((old) => old.some((t) => t.id === id) ? old : [{ id, name: prompt.slice(0, 60), preview: prompt, cwd }, ...old]);
      }
      setDraft("");
    } catch (e) { setError(String(e)); }
    finally { pending.current = false; setSending(false); }
  }, [draft, history, loading, selected, cwd]);
  const stop = useCallback(async () => {
    if (!isTauri || !selected) return;
    try { await invoke("codex_interrupt", { threadId: selected }); }
    catch (e) { setError(String(e)); }
  }, [selected]);
  return { threads, selected, select, history, cwd, setCwd, draft, setDraft, refresh, cursor, error, owned, loading, send, stop, busy: sending || !!history?.working };
}
