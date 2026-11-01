//! Answers Claude Code permission prompts from the pill.
//!
//! A `PermissionRequest` hook of type "http" in ~/.claude/settings.json POSTs
//! each request here and waits for the response, which carries the decision.
//! Replying `{}` (no decision) hands the request back to Claude's own prompt.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

/// Shorter than the hook's own timeout, so Claude always gets an answer.
const WAIT: Duration = Duration::from_secs(50);

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRequest {
    pub id: u64,
    pub provider: String,
    pub provider_name: String,
    pub can_answer: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub can_allow: Option<bool>,
    pub tool: String,
    /// The command, file or URL the tool wants to use.
    pub summary: String,
    /// Folder name of the session's working directory.
    pub project: String,
}

#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Decision {
    Allow,
    Deny,
    /// Let Claude show its own prompt instead.
    Ask,
}

#[derive(Default)]
pub struct Pending {
    next: AtomicU64,
    waiting: Mutex<HashMap<u64, mpsc::Sender<Decision>>>,
}

fn summarize(tool: &str, input: &Value) -> String {
    let field = |k: &str| input.get(k).and_then(Value::as_str).map(str::to_string);
    let main = match tool {
        "Bash" | "PowerShell" => field("command"),
        "Edit" | "Write" | "Read" | "NotebookEdit" => field("file_path"),
        "WebFetch" => field("url"),
        _ => None,
    };
    main.or_else(|| field("description"))
        .unwrap_or_else(|| input.to_string().chars().take(300).collect())
}

fn handle(app: &AppHandle, input: Value) -> Result<Value, u16> {
    // Only ask through the pill while it is on screen.
    let visible = app
        .get_webview_window("main")
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);
    if !visible {
        return Ok(json!({}));
    }

    let pending = app.state::<Pending>();
    let id = pending.next.fetch_add(1, Ordering::Relaxed);
    let (tx, rx) = mpsc::channel();
    {
        let mut waiting = pending.waiting.lock().unwrap();
        if waiting.len() >= 4 {
            return Err(429);
        }
        waiting.insert(id, tx);
    }

    let tool = input["tool_name"].as_str().unwrap_or("Tool").to_string();
    let project = input["cwd"]
        .as_str()
        .map(|c| {
            c.trim_end_matches(['\\', '/'])
                .rsplit(['\\', '/'])
                .next()
                .unwrap_or(c)
                .to_string()
        })
        .unwrap_or_default();
    let request = PermissionRequest {
        id,
        provider: "claude-code".into(),
        provider_name: "Claude Code".into(),
        can_answer: true,
        can_allow: None,
        summary: summarize(&tool, &input["tool_input"]),
        tool,
        project,
    };
    publish(app, request);

    let decision = rx.recv_timeout(WAIT).unwrap_or(Decision::Ask);
    pending.waiting.lock().unwrap().remove(&id);
    close(app, id);

    let behavior = match decision {
        Decision::Ask => return Ok(json!({})),
        Decision::Allow => json!({ "behavior": "allow" }),
        Decision::Deny => json!({ "behavior": "deny", "message": "Ditolak lewat FloatingBar" }),
    };
    Ok(
        json!({ "hookSpecificOutput": { "hookEventName": "PermissionRequest", "decision": behavior } }),
    )
}

pub fn answer(app: &AppHandle, id: u64, decision: Decision) -> Result<(), String> {
    let tx = app
        .state::<Pending>()
        .waiting
        .lock()
        .unwrap()
        .remove(&id)
        .ok_or("Permintaan sudah berakhir")?;
    tx.send(decision)
        .map_err(|_| "Permintaan sudah berakhir".to_string())?;
    close(app, id);
    Ok(())
}

#[derive(Default)]
pub struct HookState {
    service: Mutex<Option<crate::hook_http::Service>>,
    error: Mutex<Option<String>>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookStatus {
    running: bool,
    error: Option<String>,
    test_profile: bool,
}
#[tauri::command]
pub fn get_hook_status(state: tauri::State<HookState>) -> HookStatus {
    HookStatus {
        running: state.service.lock().unwrap().is_some(),
        error: state.error.lock().unwrap().clone(),
        test_profile: crate::profile::test_dir().is_some(),
    }
}
#[tauri::command]
pub async fn configure_permission_hook(app: AppHandle) -> Result<usize, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<HookState>();
        if state.service.lock().unwrap().is_none() {
            return Err("Layanan izin belum aktif".into());
        }
        let result = crate::permission_bridge::configure(true);
        *state.error.lock().unwrap() = result.as_ref().err().cloned();
        result
    })
    .await
    .map_err(|_| "Pekerja konfigurasi berhenti".to_string())?
}
pub fn spawn_server(app: AppHandle) {
    let state = app.state::<HookState>();
    let result = (|| -> Result<crate::hook_http::Service, String> {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).map_err(|_| "Token lokal tidak bisa dibuat")?;
        let token: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let owner = app.clone();
        let service =
            crate::hook_http::Service::start(token.clone(), move |body| handle(&owner, body))
                .map_err(|_| "Layanan izin lokal tidak bisa dijalankan")?;
        crate::permission_bridge::create(service.address.port(), token)?;
        // Upgrade only an already-configured FloatingBar endpoint.
        if let Err(e) = crate::permission_bridge::configure(false) {
            *state.error.lock().unwrap() = Some(e);
        }
        Ok(service)
    })();
    match result {
        Ok(service) => *state.service.lock().unwrap() = Some(service),
        Err(e) => *state.error.lock().unwrap() = Some(e),
    }
}

#[derive(Default)]
pub struct LiveRequests(Mutex<Vec<PermissionRequest>>);
pub fn publish(app: &AppHandle, request: PermissionRequest) {
    let state = app.state::<LiveRequests>();
    let mut queue = state.0.lock().unwrap();
    if queue.iter().any(|r| r.id == request.id) {
        return;
    }
    queue.push(request.clone());
    let _ = app.emit("permission", request);
}
pub fn close(app: &AppHandle, id: u64) {
    let state = app.state::<LiveRequests>();
    let mut queue = state.0.lock().unwrap();
    let before = queue.len();
    queue.retain(|r| r.id != id);
    if before != queue.len() {
        let _ = app.emit("permission-closed", id);
    }
}
#[tauri::command]
pub fn get_pending_permissions(state: tauri::State<LiveRequests>) -> Vec<PermissionRequest> {
    state.0.lock().unwrap().clone()
}
