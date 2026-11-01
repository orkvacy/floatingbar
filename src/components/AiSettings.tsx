import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { isTauri } from "../lib/native";

type HookStatus = { running: boolean; error: string | null; testProfile: boolean };
export function AiSettings() {
  const [status,setStatus]=useState<HookStatus|null>(null);
  const [experimental,setExperimental]=useState(false);
  const [busy,setBusy]=useState(false);
  const [message,setMessage]=useState("");
  useEffect(() => {
    if (!isTauri) return;
    Promise.all([invoke<HookStatus>("get_hook_status"),invoke<{experimentalDesktopAdapters:boolean}>("get_app_preferences")])
      .then(([s,p])=>{setStatus(s);setExperimental(p.experimentalDesktopAdapters);}).catch((e)=>setMessage(String(e)));
  },[]);
  const install=async()=>{
    setBusy(true);setMessage("");
    try { const count=await invoke<number>("configure_permission_hook");setMessage(`Hook siap (${count}). Mulai sesi Claude Code baru agar konfigurasi dimuat.`);setStatus(await invoke<HookStatus>("get_hook_status")); }
    catch(e){setMessage(String(e));}finally{setBusy(false);}
  };
  const toggle=async(enabled:boolean)=>{
    setBusy(true);setMessage("");
    try{await invoke("set_experimental_adapters",{enabled});setExperimental(enabled);}catch(e){setMessage(String(e));}finally{setBusy(false);}
  };
  return <details className="ai__settings"><summary>Pengaturan AI</summary>
    <p>Claude Code memakai helper lokal terautentikasi. Token disimpan terenkripsi pada akun Windows ini.</p>
    <button disabled={!isTauri || !status?.running || busy} onClick={()=>void install()}>{busy?"Memproses…":"Siapkan / perbarui hook Claude Code"}</button>
    {status?.error && <p role="alert">{status.error}</p>}
    {status && !status.running && <p role="alert">Layanan izin belum aktif. Mulai ulang FloatingBar sebelum menyiapkan hook.</p>}
    <label><input type="checkbox" checked={experimental} disabled={!isTauri || !status || busy} onChange={(e)=>void toggle(e.target.checked)} />Deteksi aplikasi desktop eksperimental</label>
    <p>Beta: mendeteksi prompt jendela ChatGPT / Codex dan Antigravity untuk dibuka di aplikasi asal. Adapter Claude dan sesi Codex FloatingBar tetap aktif.</p>
    {message && <p role="status">{message}</p>}
  </details>;
}
