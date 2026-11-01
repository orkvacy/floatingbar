//! The Codex panel owns its turns. Existing desktop conversations are read-only;
//! sending from one forks it, so an active desktop session is never mutated.
use crate::codex_history::{approval_summary, parse_messages, History};
use crate::{
    codex_rpc::Client,
    permission::{Decision, PermissionRequest},
};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Default)]
pub struct Codex {
    client: Mutex<Option<Client>>,
    histories: Mutex<HashMap<String, History>>,
    items: Mutex<HashMap<String, Value>>,
    owned: Mutex<HashSet<String>>,
    turns: Mutex<HashMap<String, String>>,
    operation: Mutex<()>,
}

fn publish(app: &AppHandle, history: &History) {
    let _ = app.emit("codex-history", history);
}
fn on_event(app: &AppHandle, event: Value) {
    let state = app.state::<Codex>();
    let method = event["method"].as_str().unwrap_or("");
    let p = &event["params"];
    if method == "floatingbar/permissionClosed" {
        let _ = app.emit("permission-closed", p["id"].as_u64());
        return;
    }
    if method == "floatingbar/disconnected" {
        state.owned.lock().unwrap().clear();
        state.turns.lock().unwrap().clear();
        for history in state.histories.lock().unwrap().values_mut() {
            if history.working {
                history.working = false;
                history.error = Some("Koneksi Codex terputus. Balasan mungkin belum selesai; muat ulang sebelum mengirim lagi.".into());
                publish(app, history);
            }
        }
        return;
    }
    let Some(thread_id) = p["threadId"].as_str() else {
        return;
    };
    if let Some(id) = event["floatingbarId"].as_u64() {
        let items = state.items.lock().unwrap();
        let item = p["itemId"].as_str().and_then(|id| items.get(id));
        let summary = approval_summary(p, item);
        drop(items);
        let can_allow = crate::codex_rpc::approval_result(method, p, true).is_ok();
        let _ = app.emit(
            "permission",
            PermissionRequest {
                id,
                provider: "codex-rpc".into(),
                provider_name: "ChatGPT / Codex".into(),
                can_answer: true,
                can_allow: Some(can_allow),
                tool: if method.contains("commandExecution") || method == "execCommandApproval" {
                    "Perintah"
                } else if method.contains("fileChange") || method == "applyPatchApproval" {
                    "Perubahan file"
                } else {
                    "Akses tambahan"
                }
                .into(),
                summary,
                project: "sesi FloatingBar".into(),
            },
        );
        return;
    }
    let mut histories = state.histories.lock().unwrap();
    let history = histories
        .entry(thread_id.into())
        .or_insert_with(|| History {
            thread_id: thread_id.into(),
            ..History::default()
        });
    match method {
        "turn/started" => {
            if let Some(id) = p["turn"]["id"].as_str() {
                state
                    .turns
                    .lock()
                    .unwrap()
                    .insert(thread_id.into(), id.into());
            }
            history.working = true;
            history.error = None;
        }
        "item/started" | "item/completed" => {
            let item = &p["item"];
            if let Some(id) = item["id"].as_str() {
                let mut items = state.items.lock().unwrap();
                if items.len() > 300 {
                    items.clear();
                }
                items.insert(id.into(), item.clone());
            }
            history.upsert_item(item);
        }
        "item/agentMessage/delta" => {
            let Some(id) = p["itemId"].as_str() else {
                return;
            };
            let text = p["delta"].as_str().unwrap_or("");
            history.delta(id, text);
        }
        "turn/completed" => {
            state.turns.lock().unwrap().remove(thread_id);
            history.working = false;
            history.error = p["turn"]["error"]["message"].as_str().map(String::from);
            if history.error.is_none() && p["turn"]["status"] == "failed" {
                history.error = Some("Codex gagal menyelesaikan balasan".into());
            }
        }
        "error" => {
            history.error = p["error"]["message"].as_str().map(String::from);
        }
        "floatingbar/unsupported" => {
            history.error = p["message"].as_str().map(String::from);
        }
        _ => return,
    }
    if history.messages.len() > 40 {
        history.messages.drain(..history.messages.len() - 40);
    }
    publish(app, history);
}
fn client(app: &AppHandle) -> Result<Client, String> {
    let state = app.state::<Codex>();
    let mut current = state.client.lock().unwrap();
    if let Some(client) = current.as_ref().filter(|c| !c.closed()) {
        return Ok(client.clone());
    }
    let app = app.clone();
    let client = Client::start(move |e| on_event(&app, e))?;
    *current = Some(client.clone());
    Ok(client)
}

#[tauri::command]
pub async fn codex_threads(app: AppHandle, cursor: Option<String>) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || client(&app)?.call("thread/list", json!({"limit":30,"cursor":cursor,"useStateDbOnly":true,"sortKey":"updated_at","sourceKinds":["cli","vscode","appServer","unknown"]}))).await.map_err(|_| "Pekerja Codex berhenti".to_string())?
}
#[tauri::command]
pub async fn codex_read(app: AppHandle, thread_id: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let data = client(&app)?.call("thread/read", json!({"threadId":thread_id,"includeTurns":true}))?;
        let state = app.state::<Codex>();
        // Do not overwrite a locally streaming turn with a stale disk snapshot.
        let mut histories = state.histories.lock().unwrap();
        let history = histories.entry(thread_id.clone()).or_default();
        if !history.working { *history = History { thread_id, messages:parse_messages(&data["thread"]), working:false, error:None }; }
        Ok(json!({"history":history,"cwd":data["thread"]["cwd"],"owned":state.owned.lock().unwrap().contains(&history.thread_id)}))
    }).await.map_err(|_| "Pekerja Codex berhenti".to_string())?
}
#[tauri::command]
pub async fn codex_send(
    app: AppHandle,
    thread_id: Option<String>,
    cwd: String,
    prompt: String,
) -> Result<String, String> {
    if prompt.trim().is_empty() || prompt.len() > 100_000 {
        return Err("Prompt kosong atau terlalu panjang".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<Codex>();
        let _operation = state.operation.try_lock().map_err(|_| "Operasi Codex lain masih berlangsung")?;
        let rpc = client(&app)?;
        let seed = thread_id.as_ref().and_then(|id| state.histories.lock().unwrap().get(id).cloned());
        let owned = thread_id.as_ref().is_some_and(|id| state.owned.lock().unwrap().contains(id));
        let id = if owned { thread_id.unwrap() } else {
            let path = Path::new(&cwd);
            if !path.is_absolute() || !path.is_dir() { return Err("Pilih folder kerja absolut yang sudah ada".into()); }
            let params = json!({"cwd":cwd,"approvalPolicy":"on-request","sandbox":"read-only","approvalsReviewer":"user"});
            let result = if let Some(id) = thread_id { let mut params = params; params["threadId"] = json!(id); params["excludeTurns"] = json!(true); rpc.call("thread/fork",params)? }
            else { rpc.call("thread/start",params)? };
            let id = result["thread"]["id"].as_str().ok_or("Codex tidak mengembalikan ID sesi")?.to_owned();
            state.owned.lock().unwrap().insert(id.clone());
            id
        };
        {
            let mut histories = state.histories.lock().unwrap();
            let history = histories.entry(id.clone()).or_insert_with(|| History { thread_id:id.clone(), messages: seed.map(|h|h.messages).unwrap_or_default(), ..History::default() });
            if history.working { return Err("Sesi Codex ini masih bekerja".into()); }
            history.working = true; history.error = None; publish(&app, history);
        }
        if let Err(error) = rpc.call("turn/start",json!({"threadId":id,"input":[{"type":"text","text":prompt}]})) {
            if let Some(history) = state.histories.lock().unwrap().get_mut(&id) { history.working = error.starts_with("Codex belum merespons"); history.error = Some(error.clone()); publish(&app, history); }
            return Err(error);
        }
        Ok(id)
    }).await.map_err(|_| "Pekerja Codex berhenti".to_string())?
}
pub async fn answer(app: AppHandle, id: u64, decision: Decision) -> Result<(), String> {
    if matches!(decision, Decision::Ask) {
        return Err("Jawab permintaan sesi FloatingBar melalui Izinkan atau Tolak".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<Codex>();
        let rpc = state
            .client
            .lock()
            .unwrap()
            .as_ref()
            .cloned()
            .ok_or("Koneksi Codex belum tersedia")?;
        rpc.answer(id, matches!(decision, Decision::Allow))
    })
    .await
    .map_err(|_| "Pekerja Codex berhenti".to_string())?
}

#[tauri::command]
pub async fn codex_interrupt(app: AppHandle, thread_id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<Codex>();
        if !state.owned.lock().unwrap().contains(&thread_id) {
            return Err("Hanya sesi FloatingBar yang bisa dihentikan".into());
        }
        let turn = state
            .turns
            .lock()
            .unwrap()
            .get(&thread_id)
            .cloned()
            .ok_or("Turn Codex belum aktif atau sudah selesai")?;
        client(&app)?.call(
            "turn/interrupt",
            json!({"threadId":thread_id,"turnId":turn}),
        )?;
        Ok(())
    })
    .await
    .map_err(|_| "Pekerja Codex berhenti".to_string())?
}
