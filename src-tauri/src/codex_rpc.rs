//! Codex App Server transport. No model calls until the user starts a turn.
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc, Arc, Mutex,
};
use std::time::Duration;

pub const ID_BASE: u64 = 1 << 48;
static NEXT_REMOTE: AtomicU64 = AtomicU64::new(ID_BASE);
type Reply = Result<Value, String>;
struct Remote {
    wire_id: Value,
    method: String,
    params: Value,
}
struct Inner {
    child: Mutex<Child>,
    stdin: Mutex<ChildStdin>,
    pending: Mutex<HashMap<u64, mpsc::Sender<Reply>>>,
    remote: Mutex<HashMap<u64, Remote>>,
    next: AtomicU64,
    closed: AtomicBool,
    event: Box<dyn Fn(Value) + Send + Sync>,
}
impl Drop for Inner {
    fn drop(&mut self) {
        if let Ok(c) = self.child.get_mut() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}
#[derive(Clone)]
pub struct Client(Arc<Inner>);

pub fn executable() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("FLOATINGBAR_CODEX_BIN") {
        let path = PathBuf::from(path);
        if path.is_absolute()
            && path.is_file()
            && path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
        {
            return Ok(path);
        }
        return Err("FLOATINGBAR_CODEX_BIN harus menunjuk codex.exe yang valid".into());
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        let path = PathBuf::from(appdata).join("npm/node_modules/@openai/codex/node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/bin/codex.exe");
        if path.is_file() {
            return Ok(path);
        }
    }
    for dir in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        let path = dir.join("codex.exe");
        if path.is_file() {
            return Ok(path);
        }
    }
    Err("Codex CLI belum ditemukan. Instal/login Codex CLI, atau atur FLOATINGBAR_CODEX_BIN ke codex.exe.".into())
}

fn write(inner: &Inner, value: &Value) -> Result<(), String> {
    if inner.closed.load(Ordering::Relaxed) {
        return Err("Koneksi Codex terputus".into());
    }
    let mut stdin = inner.stdin.lock().unwrap();
    serde_json::to_writer(&mut *stdin, value).map_err(|_| "Pesan Codex tidak valid")?;
    stdin
        .write_all(b"\n")
        .and_then(|_| stdin.flush())
        .map_err(|_| "Koneksi Codex terputus".into())
}
pub fn approval_result(method: &str, params: &Value, allow: bool) -> Reply {
    match method {
        "execCommandApproval" | "applyPatchApproval" => Ok(
            json!({"decision": if allow { json!("approved") } else { json!({"denied":{"rejection":"Ditolak melalui FloatingBar"}}) }}),
        ),
        "item/commandExecution/requestApproval" | "item/fileChange/requestApproval" => {
            let mut decision = if allow { "accept" } else { "decline" };
            if let Some(choices) = params["availableDecisions"].as_array() {
                if !allow
                    && !choices.iter().any(|v| v == "decline")
                    && choices.iter().any(|v| v == "cancel")
                {
                    decision = "cancel";
                }
                if !choices.iter().any(|v| v.as_str() == Some(decision)) {
                    return Err("Keputusan sekali pakai tidak tersedia untuk permintaan ini".into());
                }
            }
            Ok(json!({ "decision": decision }))
        }
        "item/permissions/requestApproval" => Ok(json!({
            "permissions": if allow { params["permissions"].clone() } else { json!({}) },
            "scope": "turn"
        })),
        _ => Err("Jenis persetujuan belum didukung".into()),
    }
}

impl Client {
    pub fn start(event: impl Fn(Value) + Send + Sync + 'static) -> Result<Self, String> {
        let mut cmd = Command::new(executable()?);
        cmd.args(["app-server", "--listen", "stdio://"]);
        Self::spawn(cmd, event)
    }
    pub fn spawn(
        mut cmd: Command,
        event: impl Fn(Value) + Send + Sync + 'static,
    ) -> Result<Self, String> {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = cmd
            .spawn()
            .map_err(|_| "Codex App Server tidak bisa dijalankan")?;
        let stdout = child.stdout.take().ok_or("Codex stdout tidak tersedia")?;
        let stdin = child.stdin.take().ok_or("Codex stdin tidak tersedia")?;
        let inner = Arc::new(Inner {
            child: Mutex::new(child),
            stdin: Mutex::new(stdin),
            pending: Mutex::new(HashMap::new()),
            remote: Mutex::new(HashMap::new()),
            next: AtomicU64::new(1),
            closed: AtomicBool::new(false),
            event: Box::new(event),
        });
        let weak = Arc::downgrade(&inner);
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Some(inner) = weak.upgrade() else { return };
                let Ok(line) = line else { break };
                let Ok(mut message) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };
                if message["method"].is_string() && !message["id"].is_null() {
                    let method = message["method"].as_str().unwrap().to_owned();
                    let mut params = message["params"].clone();
                    if params["threadId"].is_null() && params["conversationId"].is_string() {
                        params["threadId"] = params["conversationId"].clone();
                        message["params"] = params.clone();
                    }
                    if approval_result(&method, &params, false).is_err() {
                        let _ = write(
                            &inner,
                            &json!({"id":message["id"],"error":{"code":-32601,"message":"FloatingBar does not support this interaction. No permission granted."}}),
                        );
                        (inner.event)(
                            json!({"method":"floatingbar/unsupported","params":{"threadId":params["threadId"],"requestMethod":method,"parameterKeys":params.as_object().map(|p|p.keys().cloned().collect::<Vec<_>>()),"availableDecisions":params["availableDecisions"],"message":"Codex meminta interaksi yang belum didukung. Tidak ada izin diberikan."}}),
                        );
                        continue;
                    }
                    let id = NEXT_REMOTE.fetch_add(1, Ordering::Relaxed);
                    inner.remote.lock().unwrap().insert(
                        id,
                        Remote {
                            wire_id: message["id"].clone(),
                            method,
                            params,
                        },
                    );
                    message["floatingbarId"] = json!(id);
                    (inner.event)(message);
                } else if let Some(id) = message["id"].as_u64() {
                    if let Some(tx) = inner.pending.lock().unwrap().remove(&id) {
                        let result = if message["error"].is_object() {
                            Err(message["error"]["message"]
                                .as_str()
                                .unwrap_or("Codex RPC gagal")
                                .to_owned())
                        } else {
                            Ok(message["result"].clone())
                        };
                        let _ = tx.send(result);
                    }
                } else {
                    if message["method"] == "serverRequest/resolved"
                        || message["method"] == "turn/completed"
                    {
                        let mut remote = inner.remote.lock().unwrap();
                        let ids: Vec<_> = remote
                            .iter()
                            .filter(|(_, r)| {
                                if message["method"] == "serverRequest/resolved" {
                                    r.wire_id == message["params"]["requestId"]
                                } else {
                                    r.params["threadId"] == message["params"]["threadId"]
                                        && (r.params["turnId"].is_null()
                                            || r.params["turnId"]
                                                == message["params"]["turn"]["id"])
                                }
                            })
                            .map(|(id, _)| *id)
                            .collect();
                        for id in ids {
                            remote.remove(&id);
                            (inner.event)(
                                json!({"method":"floatingbar/permissionClosed","params":{"id":id}}),
                            );
                        }
                    }
                    (inner.event)(message);
                }
            }
            if let Some(inner) = weak.upgrade() {
                inner.closed.store(true, Ordering::Relaxed);
                for (_, tx) in inner.pending.lock().unwrap().drain() {
                    let _ = tx.send(Err("Koneksi Codex terputus".into()));
                }
                for (id, _) in inner.remote.lock().unwrap().drain() {
                    (inner.event)(
                        json!({"method":"floatingbar/permissionClosed","params":{"id":id}}),
                    );
                }
                (inner.event)(json!({"method":"floatingbar/disconnected"}));
            }
        });
        let client = Self(inner);
        client.call("initialize", json!({"clientInfo":{"name":"floatingbar","title":"FloatingBar","version":env!("CARGO_PKG_VERSION")}}))?;
        write(&client.0, &json!({"method":"initialized","params":{}}))?;
        Ok(client)
    }
    pub fn closed(&self) -> bool {
        self.0.closed.load(Ordering::Relaxed)
    }
    pub fn call(&self, method: &str, params: Value) -> Reply {
        let id = self.0.next.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = mpsc::channel();
        self.0.pending.lock().unwrap().insert(id, tx);
        let result = write(&self.0, &json!({"id":id,"method":method,"params":params}))
            .and_then(|_| rx.recv_timeout(Duration::from_secs(30)).map_err(|_| "Codex belum merespons; operasi mungkin masih berlangsung. Muat ulang sebelum mencoba lagi.".to_string())?);
        self.0.pending.lock().unwrap().remove(&id);
        result
    }
    pub fn answer(&self, id: u64, allow: bool) -> Result<(), String> {
        let mut remote = self.0.remote.lock().unwrap();
        let r = remote.get(&id).ok_or("Permintaan Codex sudah berakhir")?;
        let result = approval_result(&r.method, &r.params, allow)?;
        write(&self.0, &json!({"id":r.wire_id,"result":result}))?;
        remote.remove(&id);
        (self.0.event)(json!({"method":"floatingbar/permissionClosed","params":{"id":id}}));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn approvals_never_grant_session_access() {
        for method in [
            "item/commandExecution/requestApproval",
            "item/fileChange/requestApproval",
        ] {
            assert_eq!(
                approval_result(method, &json!({}), true).unwrap(),
                json!({"decision":"accept"})
            );
            assert_eq!(
                approval_result(method, &json!({}), false).unwrap(),
                json!({"decision":"decline"})
            );
        }
        assert_eq!(
            approval_result("execCommandApproval", &json!({}), true).unwrap()["decision"],
            "approved"
        );
        assert!(
            approval_result("applyPatchApproval", &json!({}), false).unwrap()["decision"]["denied"]
                .is_object()
        );
    }
    #[test]
    fn limited_decisions_cannot_be_widened() {
        assert!(approval_result(
            "item/commandExecution/requestApproval",
            &json!({"availableDecisions":["acceptForSession","decline"]}),
            true
        )
        .is_err());
        assert!(approval_result("unknown", &json!({}), true).is_err());
        assert_eq!(approval_result("item/commandExecution/requestApproval", &json!({"availableDecisions":["accept",{"acceptWithExecpolicyAmendment":{"execpolicy_amendment":["cmd"]}},"cancel"]}), false).unwrap(), json!({"decision":"cancel"}));
    }
    #[test]
    fn permissions_grant_only_requested_subset_for_one_turn() {
        let p = json!({"permissions":{"network":{"enabled":true}}});
        assert_eq!(
            approval_result("item/permissions/requestApproval", &p, true).unwrap(),
            json!({"permissions":p["permissions"],"scope":"turn"})
        );
        assert_eq!(
            approval_result("item/permissions/requestApproval", &p, false).unwrap()["permissions"],
            json!({})
        );
    }
    // A real stdio peer in a separate test process, without contacting a model.
    #[test]
    fn fixture_server() {
        if std::env::var_os("FLOATINGBAR_RPC_FIXTURE").is_none() {
            return;
        }
        let mut decisions = 0;
        for line in std::io::stdin().lock().lines() {
            let m: Value = serde_json::from_str(&line.unwrap()).unwrap();
            let response = match m["method"].as_str() {
                Some("initialize") => json!({"id":m["id"],"result":{}}),
                Some("initialized") => continue,
                Some("test/request") => {
                    println!(
                        "{}",
                        json!({"id":"server-string-id","method":"item/commandExecution/requestApproval","params":{"threadId":"fixture","turnId":"turn","command":"Write-Output fixture"}})
                    );
                    json!({"id":m["id"],"result":{}})
                }
                Some("test/probe") => json!({"id":m["id"],"result":{"decisions":decisions}}),
                Some("test/resolve") => {
                    println!(
                        "{}",
                        json!({"method":"serverRequest/resolved","params":{"requestId":"server-string-id","threadId":"fixture"}})
                    );
                    json!({"id":m["id"],"result":{}})
                }
                Some("test/exit") => return,
                None => {
                    assert_eq!(m["id"], "server-string-id");
                    assert_eq!(m["result"]["decision"], "decline");
                    decisions += 1;
                    continue;
                }
                _ => continue,
            };
            println!("{response}");
            std::io::stdout().flush().unwrap();
        }
    }
    #[test]
    fn wire_roundtrip_requires_explicit_answer_and_rejects_stale_ids() {
        let name = module_path!().split_once("::").unwrap().1;
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", &format!("{name}::fixture_server"), "--nocapture"])
            .env("FLOATINGBAR_RPC_FIXTURE", "1");
        let (tx, rx) = mpsc::channel();
        let client = Client::spawn(command, move |v| {
            let _ = tx.send(v);
        })
        .unwrap();
        client.call("test/request", json!({})).unwrap();
        let event = rx.recv_timeout(Duration::from_secs(3)).unwrap();
        let id = event["floatingbarId"].as_u64().unwrap();
        assert_eq!(
            client.call("test/probe", json!({})).unwrap()["decisions"],
            0
        );
        client.answer(id, false).unwrap();
        assert_eq!(
            client.call("test/probe", json!({})).unwrap()["decisions"],
            1
        );
        assert!(client.answer(id, true).is_err());
        client.call("test/request", json!({})).unwrap();
        let id = loop {
            let e = rx.recv_timeout(Duration::from_secs(3)).unwrap();
            if let Some(id) = e["floatingbarId"].as_u64() {
                break id;
            }
        };
        client.call("test/resolve", json!({})).unwrap();
        assert!(client.answer(id, true).is_err());
        assert!(client.call("test/exit", json!({})).is_err());
        assert!(client.closed());
    }
}
