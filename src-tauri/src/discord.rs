use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};

const CLIENT_ID: &str = "1563832553541075005";
const CLIENT_SECRET: &str = "yzHU5CDNpkdWdDeiUDynBTEHnsNual82";

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DiscordUser {
    pub id: String,
    pub name: String,
    pub avatar: Option<String>,
    pub volume: f64,
    pub is_muted: bool,
    pub is_speaking: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DiscordState {
    pub connected: bool,
    pub in_voice: bool,
    pub channel_id: Option<String>,
    pub channel_name: String,
    pub guild_id: Option<String>,
    pub self_muted: bool,
    pub self_deafened: bool,
    pub users: Vec<DiscordUser>,
}

#[derive(Default)]
pub struct DiscordManager {
    pub state: Mutex<DiscordState>,
    writer: Mutex<Option<File>>,
    speaking_users: Mutex<HashSet<String>>,
    nonce_counter: AtomicU64,
}

impl DiscordManager {
    fn next_nonce(&self) -> String {
        format!("fb_{}", self.nonce_counter.fetch_add(1, Ordering::Relaxed))
    }

    pub fn send_frame(&self, cmd: &str, args: Value) {
        let nonce = self.next_nonce();
        let payload = json!({
            "cmd": cmd,
            "args": args,
            "nonce": nonce
        });
        self.write_packet(1, &payload);
    }

    pub fn subscribe(&self, evt: &str, args: Value) {
        let nonce = self.next_nonce();
        let payload = json!({
            "cmd": "SUBSCRIBE",
            "evt": evt,
            "args": args,
            "nonce": nonce
        });
        self.write_packet(1, &payload);
    }

    fn write_packet(&self, opcode: u32, payload: &Value) {
        let mut guard = self.writer.lock().unwrap();
        if let Some(file) = guard.as_mut() {
            let json_str = payload.to_string();
            let bytes = json_str.as_bytes();
            let mut header = [0u8; 8];
            header[0..4].copy_from_slice(&opcode.to_le_bytes());
            header[4..8].copy_from_slice(&(bytes.len() as u32).to_le_bytes());
            let _ = file.write_all(&header);
            let _ = file.write_all(bytes);
            let _ = file.flush();
        }
    }
}

fn token_file(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("discord_token.json"))
}

fn load_token(app: &AppHandle) -> Option<String> {
    let path = token_file(app)?;
    let content = fs::read_to_string(path).ok()?;
    let parsed: Value = serde_json::from_str(&content).ok()?;
    parsed["access_token"].as_str().map(String::from)
}

fn save_token(app: &AppHandle, token: &str) {
    if let Some(path) = token_file(app) {
        let _ = fs::create_dir_all(path.parent().unwrap());
        let val = json!({ "access_token": token });
        let _ = fs::write(path, val.to_string());
    }
}

fn clear_token(app: &AppHandle) {
    if let Some(path) = token_file(app) {
        let _ = fs::remove_file(path);
    }
}

fn exchange_code_for_token(code: &str) -> Option<String> {
    let post_data = format!(
        "client_id={CLIENT_ID}&client_secret={CLIENT_SECRET}&grant_type=authorization_code&code={code}"
    );

    let output = Command::new("curl.exe")
        .args([
            "-s",
            "-X", "POST",
            "https://discord.com/api/v10/oauth2/token",
            "-H", "Content-Type: application/x-www-form-urlencoded",
            "-d", &post_data,
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let parsed: Value = serde_json::from_slice(&output.stdout).ok()?;
    parsed["access_token"].as_str().map(String::from)
}

fn try_connect_pipe() -> Option<File> {
    for i in 0..10 {
        let path = format!(r"\\.\pipe\discord-ipc-{i}");
        if let Ok(file) = OpenOptions::new().read(true).write(true).open(&path) {
            return Some(file);
        }
    }
    None
}

pub fn spawn(app: AppHandle) {
    thread::spawn(move || {
        let mgr = app.state::<DiscordManager>();

        loop {
            let mut file = match try_connect_pipe() {
                Some(f) => f,
                None => {
                    // Update state to disconnected
                    let mut st = mgr.state.lock().unwrap();
                    if st.connected {
                        st.connected = false;
                        st.in_voice = false;
                        st.channel_id = None;
                        st.channel_name.clear();
                        st.users.clear();
                        let clone = st.clone();
                        drop(st);
                        let _ = app.emit("discord-state", clone);
                    }
                    thread::sleep(Duration::from_secs(3));
                    continue;
                }
            };

            // Set writer
            if let Ok(writer_clone) = file.try_clone() {
                *mgr.writer.lock().unwrap() = Some(writer_clone);
            }

            // Send handshake
            let handshake = json!({ "v": 1, "client_id": CLIENT_ID });
            mgr.write_packet(0, &handshake);

            let mut cached_token = load_token(&app);
            let mut current_subscribed_channel: Option<String> = None;

            loop {
                // Read 8-byte header
                let mut header = [0u8; 8];
                if file.read_exact(&mut header).is_err() {
                    break; // connection closed
                }

                let _opcode = u32::from_le_bytes(header[0..4].try_into().unwrap());
                let length = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;

                let mut body = vec![0u8; length];
                if file.read_exact(&mut body).is_err() {
                    break; // connection closed
                }

                let msg: Value = match serde_json::from_slice(&body) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                let cmd = msg["cmd"].as_str().unwrap_or_default();
                let evt = msg["evt"].as_str().unwrap_or_default();

                if cmd == "DISPATCH" && evt == "READY" {
                    {
                        let mut st = mgr.state.lock().unwrap();
                        st.connected = true;
                        let clone = st.clone();
                        drop(st);
                        let _ = app.emit("discord-state", clone);
                    }

                    if let Some(token) = &cached_token {
                        mgr.send_frame("AUTHENTICATE", json!({ "access_token": token }));
                    } else {
                        mgr.send_frame(
                            "AUTHORIZE",
                            json!({ "client_id": CLIENT_ID, "scopes": ["rpc", "identify"] }),
                        );
                    }
                } else if cmd == "AUTHORIZE" {
                    if let Some(code) = msg["data"]["code"].as_str() {
                        if let Some(token) = exchange_code_for_token(code) {
                            save_token(&app, &token);
                            cached_token = Some(token.clone());
                            mgr.send_frame("AUTHENTICATE", json!({ "access_token": token }));
                        }
                    }
                } else if cmd == "AUTHENTICATE" {
                    if msg.get("evt").and_then(Value::as_str) == Some("ERROR") {
                        // Token might have expired, clear and re-authorize
                        clear_token(&app);
                        cached_token = None;
                        mgr.send_frame(
                            "AUTHORIZE",
                            json!({ "client_id": CLIENT_ID, "scopes": ["rpc", "identify"] }),
                        );
                    } else {
                        // Successfully authenticated!
                        mgr.subscribe("VOICE_CHANNEL_SELECT", json!({}));
                        mgr.subscribe("VOICE_SETTINGS_UPDATE", json!({}));
                        mgr.send_frame("GET_VOICE_SETTINGS", json!({}));
                        mgr.send_frame("GET_SELECTED_VOICE_CHANNEL", json!({}));
                    }
                } else if cmd == "GET_VOICE_SETTINGS" || evt == "VOICE_SETTINGS_UPDATE" {
                    if let Some(data) = msg.get("data") {
                        let mut st = mgr.state.lock().unwrap();
                        if let Some(mute) = data["mute"].as_bool() {
                            st.self_muted = mute;
                        }
                        if let Some(deaf) = data["deaf"].as_bool() {
                            st.self_deafened = deaf;
                        }
                        let clone = st.clone();
                        drop(st);
                        let _ = app.emit("discord-state", clone);
                    }
                } else if evt == "VOICE_CHANNEL_SELECT" {
                    if let Some(channel_id) = msg["data"]["channel_id"].as_str() {
                        let cid = channel_id.to_string();
                        if current_subscribed_channel.as_deref() != Some(&cid) {
                            current_subscribed_channel = Some(cid.clone());
                            mgr.subscribe("SPEAKING_START", json!({ "channel_id": cid }));
                            mgr.subscribe("SPEAKING_STOP", json!({ "channel_id": cid }));
                            mgr.subscribe("VOICE_STATE_CREATE", json!({ "channel_id": cid }));
                            mgr.subscribe("VOICE_STATE_DELETE", json!({ "channel_id": cid }));
                            mgr.subscribe("VOICE_STATE_UPDATE", json!({ "channel_id": cid }));
                        }
                        mgr.send_frame("GET_SELECTED_VOICE_CHANNEL", json!({}));
                    } else {
                        current_subscribed_channel = None;
                        mgr.speaking_users.lock().unwrap().clear();
                        let mut st = mgr.state.lock().unwrap();
                        st.in_voice = false;
                        st.channel_id = None;
                        st.channel_name.clear();
                        st.users.clear();
                        let clone = st.clone();
                        drop(st);
                        let _ = app.emit("discord-state", clone);
                    }
                } else if cmd == "GET_SELECTED_VOICE_CHANNEL" {
                    let data = &msg["data"];
                    let mut st = mgr.state.lock().unwrap();

                    if data.is_null() || data["id"].is_null() {
                        st.in_voice = false;
                        st.channel_id = None;
                        st.channel_name.clear();
                        st.users.clear();
                        mgr.speaking_users.lock().unwrap().clear();
                    } else {
                        let id = data["id"].as_str().unwrap_or_default().to_string();
                        st.in_voice = true;
                        st.channel_id = Some(id.clone());
                        st.channel_name = data["name"]
                            .as_str()
                            .unwrap_or("Voice Channel")
                            .to_string();
                        st.guild_id = data["guild_id"].as_str().map(String::from);

                        if current_subscribed_channel.as_deref() != Some(&id) {
                            current_subscribed_channel = Some(id.clone());
                            mgr.subscribe("SPEAKING_START", json!({ "channel_id": id }));
                            mgr.subscribe("SPEAKING_STOP", json!({ "channel_id": id }));
                            mgr.subscribe("VOICE_STATE_CREATE", json!({ "channel_id": id }));
                            mgr.subscribe("VOICE_STATE_DELETE", json!({ "channel_id": id }));
                            mgr.subscribe("VOICE_STATE_UPDATE", json!({ "channel_id": id }));
                        }

                        let speaking = mgr.speaking_users.lock().unwrap().clone();
                        let mut users = Vec::new();

                        if let Some(states) = data["voice_states"].as_array() {
                            for vs in states {
                                let user = &vs["user"];
                                let uid = user["id"].as_str().unwrap_or_default().to_string();
                                let name = vs["nick"]
                                    .as_str()
                                    .or_else(|| user["global_name"].as_str())
                                    .or_else(|| user["username"].as_str())
                                    .unwrap_or("User")
                                    .to_string();

                                let avatar = user["avatar"].as_str().map(|a| {
                                    format!("https://cdn.discordapp.com/avatars/{uid}/{a}.png?size=64")
                                });

                                let vol = vs["volume"].as_f64().unwrap_or(100.0) / 100.0;
                                let is_muted = vs["mute"].as_bool().unwrap_or(false)
                                    || vs["voice_state"]["self_mute"].as_bool().unwrap_or(false)
                                    || vs["voice_state"]["mute"].as_bool().unwrap_or(false);

                                users.push(DiscordUser {
                                    id: uid.clone(),
                                    name,
                                    avatar,
                                    volume: vol.clamp(0.0, 1.0),
                                    is_muted,
                                    is_speaking: speaking.contains(&uid),
                                });
                            }
                        }

                        st.users = users;
                    }

                    let clone = st.clone();
                    drop(st);
                    let _ = app.emit("discord-state", clone);
                } else if evt == "VOICE_STATE_CREATE"
                    || evt == "VOICE_STATE_DELETE"
                    || evt == "VOICE_STATE_UPDATE"
                {
                    mgr.send_frame("GET_SELECTED_VOICE_CHANNEL", json!({}));
                } else if evt == "SPEAKING_START" {
                    if let Some(uid) = msg["data"]["user_id"].as_str() {
                        mgr.speaking_users.lock().unwrap().insert(uid.to_string());
                        let mut st = mgr.state.lock().unwrap();
                        let mut changed = false;
                        for u in &mut st.users {
                            if u.id == uid && !u.is_speaking {
                                u.is_speaking = true;
                                changed = true;
                            }
                        }
                        if changed {
                            let clone = st.clone();
                            drop(st);
                            let _ = app.emit("discord-state", clone);
                        }
                    }
                } else if evt == "SPEAKING_STOP" {
                    if let Some(uid) = msg["data"]["user_id"].as_str() {
                        mgr.speaking_users.lock().unwrap().remove(uid);
                        let mut st = mgr.state.lock().unwrap();
                        let mut changed = false;
                        for u in &mut st.users {
                            if u.id == uid && u.is_speaking {
                                u.is_speaking = false;
                                changed = true;
                            }
                        }
                        if changed {
                            let clone = st.clone();
                            drop(st);
                            let _ = app.emit("discord-state", clone);
                        }
                    }
                }
            }

            // Connection broke, clear writer
            *mgr.writer.lock().unwrap() = None;
            {
                let mut st = mgr.state.lock().unwrap();
                st.connected = false;
                st.in_voice = false;
                st.channel_id = None;
                st.channel_name.clear();
                st.users.clear();
                let clone = st.clone();
                drop(st);
                let _ = app.emit("discord-state", clone);
            }

            thread::sleep(Duration::from_secs(3));
        }
    });
}

// ---- Tauri Commands ----

#[tauri::command]
pub fn get_discord_state(mgr: State<'_, DiscordManager>) -> DiscordState {
    mgr.state.lock().unwrap().clone()
}

#[tauri::command]
pub fn discord_set_mute(mgr: State<'_, DiscordManager>, mute: bool) {
    mgr.send_frame("SET_VOICE_SETTINGS", json!({ "mute": mute }));
}

#[tauri::command]
pub fn discord_set_deaf(mgr: State<'_, DiscordManager>, deaf: bool) {
    mgr.send_frame("SET_VOICE_SETTINGS", json!({ "deaf": deaf }));
}

#[tauri::command]
pub fn discord_set_user_volume(mgr: State<'_, DiscordManager>, user_id: String, volume: f64) {
    let vol_int = (volume * 100.0).round() as i64;
    mgr.send_frame(
        "SET_USER_VOICE_SETTINGS",
        json!({ "user_id": user_id, "volume": vol_int }),
    );
}

#[tauri::command]
pub fn discord_toggle_user_mute(mgr: State<'_, DiscordManager>, user_id: String, mute: bool) {
    mgr.send_frame(
        "SET_USER_VOICE_SETTINGS",
        json!({ "user_id": user_id, "mute": mute }),
    );
}

#[tauri::command]
pub fn discord_disconnect(mgr: State<'_, DiscordManager>) {
    mgr.send_frame("SELECT_VOICE_CHANNEL", json!({ "channel_id": null }));
}
