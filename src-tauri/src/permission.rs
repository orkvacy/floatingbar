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
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

pub const PORT: u16 = 47821;
/// Shorter than the hook's own timeout, so Claude always gets an answer.
const WAIT: Duration = Duration::from_secs(55);

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRequest {
    pub id: u64,
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

fn handle(app: &AppHandle, body: &str) -> String {
    let Ok(input) = serde_json::from_str::<Value>(body) else {
        return "{}".into();
    };
    // Only ask through the pill while it is on screen.
    let visible = app
        .get_webview_window("main")
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);
    if !visible {
        return "{}".into();
    }

    let pending = app.state::<Pending>();
    let id = pending.next.fetch_add(1, Ordering::Relaxed);
    let (tx, rx) = mpsc::channel();
    pending.waiting.lock().unwrap().insert(id, tx);

    let tool = input["tool_name"].as_str().unwrap_or("Tool").to_string();
    let project = input["cwd"]
        .as_str()
        .map(|c| c.trim_end_matches(['\\', '/']).rsplit(['\\', '/']).next().unwrap_or(c).to_string())
        .unwrap_or_default();
    let request = PermissionRequest { id, summary: summarize(&tool, &input["tool_input"]), tool, project };
    let _ = app.emit("permission", request);

    let decision = rx.recv_timeout(WAIT).unwrap_or(Decision::Ask);
    pending.waiting.lock().unwrap().remove(&id);
    let _ = app.emit("permission-closed", id);

    let behavior = match decision {
        Decision::Ask => return "{}".into(),
        Decision::Allow => json!({ "behavior": "allow" }),
        Decision::Deny => json!({ "behavior": "deny", "message": "Ditolak lewat FloatingBar" }),
    };
    json!({ "hookSpecificOutput": { "hookEventName": "PermissionRequest", "decision": behavior } }).to_string()
}

pub fn answer(app: &AppHandle, id: u64, decision: Decision) {
    if let Some(tx) = app.state::<Pending>().waiting.lock().unwrap().remove(&id) {
        let _ = tx.send(decision);
    }
}

pub fn spawn_server(app: AppHandle) {
    thread::spawn(move || {
        let server = match tiny_http::Server::http(("127.0.0.1", PORT)) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("permission server: {e}");
                return;
            }
        };
        let json_header: tiny_http::Header = "Content-Type: application/json".parse().unwrap();
        for mut request in server.incoming_requests() {
            let (app, header) = (app.clone(), json_header.clone());
            // Each request blocks until answered, so serve them in parallel.
            thread::spawn(move || {
                let mut body = String::new();
                let _ = request.as_reader().read_to_string(&mut body);
                let out = handle(&app, &body);
                let _ = request.respond(tiny_http::Response::from_string(out).with_header(header));
            });
        }
    });
}
