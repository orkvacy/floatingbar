import { useEffect, useRef } from "react";
import type { useCodex } from "../lib/codex";

export function CodexPanel({ codex, onTyping }: { codex: ReturnType<typeof useCodex>; onTyping: (v: boolean) => void }) {
  const refreshRef = useRef(codex.refresh);
  refreshRef.current = codex.refresh;
  useEffect(() => { void refreshRef.current(); }, []);
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => { box.current?.scrollTo({ top: box.current.scrollHeight }); }, [codex.history]);
  return <div className="ask">
    <div className="ask__head">
      <select className="ask__chat" aria-label="Percakapan Codex" value={codex.selected ?? ""} disabled={codex.busy || codex.loading}
        onChange={(e) => void codex.select(e.target.value || null)} onFocus={() => onTyping(true)} onBlur={() => onTyping(false)}>
        <option value="">＋ Sesi ChatGPT / Codex baru</option>
        {codex.threads.map((t) => <option key={t.id} value={t.id}>{t.name || t.preview || "Tanpa judul"}</option>)}
      </select>
      <button className="ask__expand" aria-label="Muat ulang Codex" disabled={codex.loading} onClick={() => void codex.refresh()}>⟳</button>
    </div>
    {codex.cursor && <button className="ask__link" disabled={codex.loading} onClick={() => void codex.refresh(true)}>Muat percakapan berikutnya</button>}
    <label className="codex__folder">Folder kerja<input aria-label="Folder kerja Codex" value={codex.cwd} disabled={codex.busy || codex.owned}
      placeholder="D:\projects\my-app" onChange={(e) => codex.setCwd(e.target.value)} onFocus={() => onTyping(true)} onBlur={() => onTyping(false)} /></label>
    <p className="ask__status">{codex.selected && !codex.owned ? "Percakapan asli dibaca saja. Mengirim membuat salinan sesi di FloatingBar." : "Codex App Server · sesi FloatingBar · read-only, izin tambahan diminta terpisah."}</p>
    {(codex.error || codex.history?.error) && <p role="alert" className="ask__status ask__status--error">{codex.error || codex.history?.error}</p>}
    <div className="ask__reply codex__reply" ref={box} aria-live="polite">
      {codex.history?.messages.map((m) => <p key={m.id} className={`ask__text ask__text--${m.role === "user" ? "user" : "claude"}`}>{m.text}</p>)}
      {codex.busy && <p className="ask__status ask__typing">ChatGPT / Codex sedang bekerja…</p>}
      {codex.loading && <p className="ask__status">Memuat ChatGPT / Codex…</p>}
    </div>
    <div className="ask__row">
      <textarea className="ask__input" rows={3} aria-label="Prompt Codex" placeholder="Tanya ChatGPT / Codex…" value={codex.draft} onChange={(e) => codex.setDraft(e.target.value)}
        onFocus={() => onTyping(true)} onBlur={() => onTyping(false)} onKeyDown={(e) => { if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); void codex.send(); } }} />
      <button className="ask__send" aria-label="Kirim ke Codex" disabled={!codex.draft.trim() || codex.busy || codex.loading || (!codex.owned && !codex.cwd.trim())} onClick={() => void codex.send()}>↑</button>
    </div>
    {codex.busy && codex.owned && <button className="ask__link" onClick={() => void codex.stop()}>Hentikan balasan Codex</button>}
  </div>;
}
