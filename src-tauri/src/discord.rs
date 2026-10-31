use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::windows::io::AsRawHandle;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};
use windows::Win32::Foundation::{ERROR_NO_DATA, HANDLE};
use windows::Win32::System::Pipes::{PeekNamedPipe, SetNamedPipeHandleState, PIPE_NOWAIT};

const MAX_FRAME_SIZE: usize = 1024 * 1024;
const WRITE_TIMEOUT: Duration = Duration::from_secs(2);

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
    pub status: String,
    pub error: Option<String>,
    pub needs_config: bool,
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
    // Only the worker touches the pipe. UI commands must never wait for Discord I/O.
    writer: Mutex<Option<SyncSender<Vec<u8>>>>,
    speaking_users: Mutex<HashSet<String>>,
    nonce_counter: AtomicU64,
    reconnect: AtomicBool,
    reset_auth: AtomicBool,
    auth_blocked: AtomicBool,
}

impl DiscordManager {
    fn next_nonce(&self) -> String {
        format!("fb_{}", self.nonce_counter.fetch_add(1, Ordering::Relaxed))
    }

    pub fn send_frame(&self, cmd: &str, args: Value) -> Result<(), String> {
        let nonce = self.next_nonce();
        let payload = json!({
            "cmd": cmd,
            "args": args,
            "nonce": nonce
        });
        self.write_packet(1, &payload)
    }

    pub fn subscribe(&self, evt: &str, args: Value) {
        let nonce = self.next_nonce();
        let payload = json!({
            "cmd": "SUBSCRIBE",
            "evt": evt,
            "args": args,
            "nonce": nonce
        });
        let _ = self.write_packet(1, &payload);
    }

    fn write_packet(&self, opcode: u32, payload: &Value) -> Result<(), String> {
        let bytes = payload.to_string().into_bytes();
        self.write_bytes(opcode, &bytes)
    }

    fn write_bytes(&self, opcode: u32, bytes: &[u8]) -> Result<(), String> {
        let mut packet = Vec::with_capacity(8 + bytes.len());
        packet.extend_from_slice(&opcode.to_le_bytes());
        packet.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        packet.extend_from_slice(&bytes);
        self.writer
            .lock()
            .unwrap()
            .as_ref()
            .ok_or("Discord belum terhubung")?
            .try_send(packet)
            .map_err(|_| "Antrean Discord penuh atau koneksi terputus".to_string())
    }
}

fn pipe_would_block(err: &io::Error) -> bool {
    err.raw_os_error() == Some(ERROR_NO_DATA.0 as i32) || err.kind() == io::ErrorKind::WouldBlock
}

fn write_packet_to<W: Write>(writer: &mut W, mut bytes: &[u8]) -> io::Result<()> {
    let deadline = Instant::now() + WRITE_TIMEOUT;
    while !bytes.is_empty() {
        if Instant::now() >= deadline {
            return Err(io::ErrorKind::TimedOut.into());
        }
        match writer.write(bytes) {
            Ok(0) => thread::sleep(Duration::from_millis(10)),
            Ok(n) => bytes = &bytes[n..],
            Err(e) if pipe_would_block(&e) => thread::sleep(Duration::from_millis(10)),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    // FlushFileBuffers on a named pipe waits for the peer to consume everything.
    // No flush is needed: writes already send the framed bytes to the pipe.
    Ok(())
}

fn read_pipe_exact(
    file: &mut File,
    mut bytes: &mut [u8],
    failed: &AtomicBool,
    reconnect: &AtomicBool,
    deadline: Option<Instant>,
) -> io::Result<()> {
    while !bytes.is_empty() {
        if failed.load(Ordering::Relaxed) || reconnect.load(Ordering::Relaxed) {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        if deadline.is_some_and(|d| Instant::now() >= d) {
            return Err(io::ErrorKind::TimedOut.into());
        }
        // Rust maps ERROR_NO_DATA on nonblocking Windows pipes to Ok(0).
        // Peek preserves the distinction between an idle pipe and a closed peer.
        let mut available = 0u32;
        unsafe {
            PeekNamedPipe(
                HANDLE(file.as_raw_handle()),
                None,
                0,
                None,
                Some(&mut available),
                None,
            )
        }
        .map_err(|e| io::Error::from_raw_os_error((e.code().0 as u32 & 0xffff) as i32))?;
        if available == 0 {
            thread::sleep(Duration::from_millis(20));
            continue;
        }
        let count = bytes.len().min(available as usize);
        match file.read(&mut bytes[..count]) {
            Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(n) => bytes = &mut bytes[n..],
            Err(e) if pipe_would_block(&e) => thread::sleep(Duration::from_millis(20)),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

fn connection_status(app: &AppHandle, mgr: &DiscordManager, status: &str, error: Option<String>) {
    let mut state = mgr.state.lock().unwrap();
    if state.status == status && state.error == error {
        return;
    }
    state.status = status.into();
    state.error = error;
    let snapshot = state.clone();
    drop(state);
    let _ = app.emit("discord-state", snapshot);
}

fn update_channel_subscriptions(
    mgr: &DiscordManager,
    current: &mut Option<String>,
    next: Option<&str>,
) {
    if current.as_deref() == next {
        return;
    }
    for evt in [
        "SPEAKING_START",
        "SPEAKING_STOP",
        "VOICE_STATE_CREATE",
        "VOICE_STATE_DELETE",
        "VOICE_STATE_UPDATE",
    ] {
        if let Some(old) = current.as_ref() {
            let _ = mgr.write_packet(
                1,
                &json!({
                    "cmd": "UNSUBSCRIBE", "evt": evt,
                    "args": { "channel_id": old }, "nonce": mgr.next_nonce()
                }),
            );
        }
        if let Some(id) = next {
            mgr.subscribe(evt, json!({ "channel_id": id }));
        }
    }
    mgr.speaking_users.lock().unwrap().clear();
    *current = next.map(str::to_owned);
}

#[derive(Serialize, Deserialize)]
struct OAuthCredentials {
    client_id: String,
    client_secret: String,
}

impl OAuthCredentials {
    fn validate(&self) -> Result<(), String> {
        if !(17..=22).contains(&self.client_id.len())
            || !self.client_id.bytes().all(|c| c.is_ascii_digit())
        {
            return Err("Client ID Discord tidak valid".into());
        }
        if self.client_secret.is_empty() || self.client_secret.len() > 256 {
            return Err("Client secret Discord tidak valid".into());
        }
        Ok(())
    }
}

fn private_file(app: &AppHandle, name: &str) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|dir| dir.join(name))
}

fn load_credentials(app: &AppHandle) -> Result<OAuthCredentials, String> {
    let credentials = match (std::env::var("FLOATINGBAR_DISCORD_CLIENT_ID"), std::env::var("FLOATINGBAR_DISCORD_CLIENT_SECRET")) {
        (Ok(client_id), Ok(client_secret)) => OAuthCredentials { client_id, client_secret },
        _ => crate::secure_store::read(&private_file(app, "discord-client.dat").ok_or("Folder pengaturan tidak tersedia")?)
            .map_err(|_| "Isi Client ID dan client secret melalui Pengaturan integrasi Discord. Kredensial tidak disertakan dalam aplikasi.")?,
    };
    credentials.validate()?;
    Ok(credentials)
}

fn load_token(app: &AppHandle) -> Option<String> {
    let path = private_file(app, "discord-token.dat")?;
    if path.exists() {
        let parsed: Value = crate::secure_store::read(&path).ok()?;
        return parsed["access_token"].as_str().map(String::from);
    }
    // One-time migration for existing installs: remove plaintext only after
    // its encrypted replacement has been written successfully.
    let legacy = private_file(app, "discord_token.json")?;
    let parsed: Value = serde_json::from_str(&fs::read_to_string(&legacy).ok()?).ok()?;
    let token = parsed["access_token"].as_str()?.to_owned();
    if crate::secure_store::write(&path, &json!({ "access_token": token })).is_ok() {
        let _ = fs::remove_file(legacy);
    }
    Some(token)
}

fn save_token(app: &AppHandle, token: &str) {
    if let Some(path) = private_file(app, "discord-token.dat") {
        let _ = crate::secure_store::write(&path, &json!({ "access_token": token }));
    }
}

fn clear_token(app: &AppHandle) {
    for name in ["discord-token.dat", "discord_token.json"] {
        if let Some(path) = private_file(app, name) {
            let _ = fs::remove_file(path);
        }
    }
}

fn exchange_code_for_token(code: &str, credentials: &OAuthCredentials) -> Result<String, String> {
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Tidak bisa membuat koneksi autentikasi Discord")?;
    let response = client
        .post("https://discord.com/api/v10/oauth2/token")
        .form(&[
            ("client_id", credentials.client_id.as_str()),
            ("client_secret", credentials.client_secret.as_str()),
            ("grant_type", "authorization_code"),
            ("code", code),
        ])
        .send()
        .map_err(|_| "Tidak bisa menghubungi Discord untuk login. Periksa koneksi internet.")?;
    let status = response.status();
    let parsed: Value = response
        .json()
        .map_err(|_| "Respons login Discord tidak valid")?;
    if !status.is_success() {
        return Err(if parsed["error"] == "invalid_client" {
            "Discord menolak kredensial aplikasi. Periksa Client ID/client secret di konfigurasi developer.".into()
        } else {
            format!("Pertukaran token Discord gagal (HTTP {}). Klik Autentikasi ulang untuk mencoba lagi.", status.as_u16())
        });
    }
    parsed["access_token"]
        .as_str()
        .map(str::to_owned)
        .ok_or("Token login Discord tidak ditemukan".into())
}

fn try_connect_pipe() -> Option<File> {
    for i in 0..10 {
        let path = format!(r"\\.\pipe\discord-ipc-{i}");
        if let Ok(file) = OpenOptions::new().read(true).write(true).open(&path) {
            // Poll on the background reader; writes have a bounded deadline.
            if unsafe {
                SetNamedPipeHandleState(
                    HANDLE(file.as_raw_handle()),
                    Some(&PIPE_NOWAIT),
                    None,
                    None,
                )
            }
            .is_ok()
            {
                return Some(file);
            }
        }
    }
    None
}

pub fn spawn(app: AppHandle) {
    thread::spawn(move || {
        let mgr = app.state::<DiscordManager>();

        loop {
            if mgr.auth_blocked.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(200));
                continue;
            }
            mgr.reconnect.store(false, Ordering::Relaxed);
            if mgr.reset_auth.swap(false, Ordering::Relaxed) {
                clear_token(&app);
            }
            let credentials = match load_credentials(&app) {
                Ok(value) => {
                    mgr.state.lock().unwrap().needs_config = false;
                    value
                }
                Err(error) => {
                    mgr.state.lock().unwrap().needs_config = true;
                    connection_status(&app, &mgr, "Pengaturan Discord diperlukan", Some(error));
                    mgr.auth_blocked.store(true, Ordering::Relaxed);
                    continue;
                }
            };
            let mut file = match try_connect_pipe() {
                Some(f) => f,
                None => {
                    connection_status(&app, &mgr, "Discord desktop tidak ditemukan", Some("Buka aplikasi Discord desktop. Koneksi voice di browser tidak menyediakan IPC lokal.".into()));
                    for _ in 0..30 {
                        if mgr.reconnect.load(Ordering::Relaxed) {
                            break;
                        }
                        thread::sleep(Duration::from_millis(100));
                    }
                    continue;
                }
            };

            connection_status(&app, &mgr, "Menghubungkan ke Discord...", None);
            let Ok(mut writer) = file.try_clone() else {
                continue;
            };
            let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(64);
            let failed = Arc::new(AtomicBool::new(false));
            let writer_failed = failed.clone();
            let writer_thread = thread::spawn(move || {
                while let Ok(packet) = rx.recv() {
                    if writer_failed.load(Ordering::Relaxed) {
                        break;
                    }
                    if write_packet_to(&mut writer, &packet).is_err() {
                        break;
                    }
                }
                writer_failed.store(true, Ordering::Relaxed);
            });
            *mgr.writer.lock().unwrap() = Some(tx);

            // Send handshake
            let handshake = json!({ "v": 1, "client_id": &credentials.client_id });
            let _ = mgr.write_packet(0, &handshake);

            let mut cached_token = load_token(&app);
            let mut current_subscribed_channel: Option<String> = None;
            let mut stage_deadline = Some(Instant::now() + Duration::from_secs(15));
            let connection_error;
            let mut reauthorized = false;

            loop {
                // Read 8-byte header
                let mut header = [0u8; 8];
                if let Err(error) = read_pipe_exact(
                    &mut file,
                    &mut header,
                    &failed,
                    &mgr.reconnect,
                    stage_deadline,
                ) {
                    connection_error = Some(if error.kind() == io::ErrorKind::TimedOut {
                        "Discord tidak membalas handshake/login tepat waktu. Coba Sambungkan ulang; jika tetap gagal, restart Discord desktop.".into()
                    } else {
                        "Koneksi IPC Discord terputus. Mencoba menyambungkan ulang.".into()
                    });
                    break;
                }

                let opcode = u32::from_le_bytes(header[0..4].try_into().unwrap());
                let length = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;

                if length > MAX_FRAME_SIZE {
                    connection_error = Some("Ukuran paket Discord tidak valid".into());
                    break;
                }

                let mut body = vec![0u8; length];
                if read_pipe_exact(
                    &mut file,
                    &mut body,
                    &failed,
                    &mgr.reconnect,
                    Some(Instant::now() + Duration::from_secs(5)),
                )
                .is_err()
                {
                    connection_error = Some("Paket Discord terputus sebelum selesai dibaca".into());
                    break;
                }

                if opcode == 3 {
                    let _ = mgr.write_bytes(4, &body);
                    continue;
                }
                if opcode == 2 {
                    let close: Value = serde_json::from_slice(&body).unwrap_or_default();
                    let code = close["code"].as_i64().unwrap_or(0);
                    connection_error = Some(format!("Discord menutup koneksi RPC (kode {code}). Coba Sambungkan ulang atau periksa konfigurasi aplikasi Discord."));
                    break;
                }
                if opcode != 1 {
                    continue;
                }
                let msg: Value = match serde_json::from_slice(&body) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                let cmd = msg["cmd"].as_str().unwrap_or_default();
                let evt = msg["evt"].as_str().unwrap_or_default();

                if evt == "ERROR" && cmd != "AUTHENTICATE" {
                    let code = msg["data"]["code"].as_i64().unwrap_or(0);
                    let error = format!(
                        "Discord menolak {cmd} (kode {code}). Periksa izin aplikasi RPC Discord."
                    );
                    connection_status(
                        &app,
                        &mgr,
                        if cmd == "AUTHORIZE" {
                            "Izin Discord belum diberikan"
                        } else {
                            "Discord terhubung, sebagian kontrol tidak tersedia"
                        },
                        Some(error.clone()),
                    );
                    if cmd == "AUTHORIZE" {
                        mgr.auth_blocked.store(true, Ordering::Relaxed);
                        connection_error = Some(error);
                        break;
                    }
                    continue;
                }

                if cmd == "DISPATCH" && evt == "READY" {
                    {
                        let mut st = mgr.state.lock().unwrap();
                        st.connected = false; // READY is not authenticated yet.
                        st.status = "Menunggu autentikasi Discord...".into();
                        let clone = st.clone();
                        drop(st);
                        let _ = app.emit("discord-state", clone);
                    }

                    stage_deadline = Some(Instant::now() + Duration::from_secs(180));
                    if let Some(token) = &cached_token {
                        let _ = mgr.send_frame("AUTHENTICATE", json!({ "access_token": token }));
                    } else {
                        let _ = mgr.send_frame(
                            "AUTHORIZE",
                            json!({ "client_id": &credentials.client_id, "scopes": ["rpc", "identify"] }),
                        );
                    }
                } else if cmd == "AUTHORIZE" {
                    if let Some(code) = msg["data"]["code"].as_str() {
                        match exchange_code_for_token(code, &credentials) {
                            Ok(token) => {
                                save_token(&app, &token);
                                cached_token = Some(token.clone());
                                let _ = mgr
                                    .send_frame("AUTHENTICATE", json!({ "access_token": token }));
                                stage_deadline = Some(Instant::now() + Duration::from_secs(15));
                            }
                            Err(error) => {
                                mgr.auth_blocked.store(true, Ordering::Relaxed);
                                connection_error = Some(error);
                                break;
                            }
                        }
                    }
                } else if cmd == "AUTHENTICATE" {
                    if msg.get("evt").and_then(Value::as_str) == Some("ERROR") {
                        if reauthorized {
                            mgr.auth_blocked.store(true, Ordering::Relaxed);
                            connection_error = Some("Discord menolak token baru. Klik Autentikasi ulang dan periksa izin aplikasi RPC.".into());
                            break;
                        }
                        reauthorized = true;
                        // Token might have expired, clear and re-authorize
                        clear_token(&app);
                        cached_token = None;
                        stage_deadline = Some(Instant::now() + Duration::from_secs(180));
                        let _ = mgr.send_frame(
                            "AUTHORIZE",
                            json!({ "client_id": &credentials.client_id, "scopes": ["rpc", "identify"] }),
                        );
                    } else {
                        // Successfully authenticated!
                        stage_deadline = None;
                        {
                            let mut st = mgr.state.lock().unwrap();
                            st.connected = true;
                            st.status = "Discord terhubung".into();
                            st.error = None;
                            let _ = app.emit("discord-state", st.clone());
                        }
                        mgr.subscribe("VOICE_CHANNEL_SELECT", json!({}));
                        mgr.subscribe("VOICE_SETTINGS_UPDATE", json!({}));
                        let _ = mgr.send_frame("GET_VOICE_SETTINGS", json!({}));
                        let _ = mgr.send_frame("GET_SELECTED_VOICE_CHANNEL", json!({}));
                    }
                } else if (cmd == "GET_VOICE_SETTINGS" || cmd == "SET_VOICE_SETTINGS")
                    || evt == "VOICE_SETTINGS_UPDATE"
                {
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
                        update_channel_subscriptions(
                            &mgr,
                            &mut current_subscribed_channel,
                            Some(&cid),
                        );
                        let _ = mgr.send_frame("GET_SELECTED_VOICE_CHANNEL", json!({}));
                    } else {
                        update_channel_subscriptions(&mgr, &mut current_subscribed_channel, None);
                        mgr.speaking_users.lock().unwrap().clear();
                        let mut st = mgr.state.lock().unwrap();
                        st.in_voice = false;
                        st.channel_id = None;
                        st.channel_name.clear();
                        st.guild_id = None;
                        st.users.clear();
                        let clone = st.clone();
                        drop(st);
                        let _ = app.emit("discord-state", clone);
                    }
                } else if cmd == "GET_SELECTED_VOICE_CHANNEL" {
                    let data = &msg["data"];
                    let mut st = mgr.state.lock().unwrap();

                    if data.is_null() || data["id"].is_null() {
                        update_channel_subscriptions(&mgr, &mut current_subscribed_channel, None);
                        st.in_voice = false;
                        st.channel_id = None;
                        st.channel_name.clear();
                        st.guild_id = None;
                        st.users.clear();
                        mgr.speaking_users.lock().unwrap().clear();
                    } else {
                        let id = data["id"].as_str().unwrap_or_default().to_string();
                        st.in_voice = true;
                        st.channel_id = Some(id.clone());
                        st.channel_name =
                            data["name"].as_str().unwrap_or("Voice Channel").to_string();
                        st.guild_id = data["guild_id"].as_str().map(String::from);

                        update_channel_subscriptions(
                            &mgr,
                            &mut current_subscribed_channel,
                            Some(&id),
                        );

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
                                    format!(
                                        "https://cdn.discordapp.com/avatars/{uid}/{a}.png?size=64"
                                    )
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
                    let _ = mgr.send_frame("GET_SELECTED_VOICE_CHANNEL", json!({}));
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
            failed.store(true, Ordering::Relaxed);
            let _ = writer_thread.join();
            mgr.speaking_users.lock().unwrap().clear();
            {
                let mut st = mgr.state.lock().unwrap();
                *st = DiscordState {
                    status: "Koneksi Discord terputus".into(),
                    error: connection_error,
                    ..DiscordState::default()
                };
                let clone = st.clone();
                drop(st);
                let _ = app.emit("discord-state", clone);
            }

            for _ in 0..30 {
                if mgr.reconnect.load(Ordering::Relaxed) {
                    break;
                }
                thread::sleep(Duration::from_millis(100));
            }
        }
    });
}

// ---- Tauri Commands ----

#[tauri::command]
pub fn get_discord_state(mgr: State<'_, DiscordManager>) -> DiscordState {
    mgr.state.lock().unwrap().clone()
}

#[tauri::command]
pub async fn discord_configure(
    app: AppHandle,
    client_id: String,
    client_secret: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let credentials = OAuthCredentials {
            client_id: client_id.trim().into(),
            client_secret: client_secret.trim().into(),
        };
        credentials.validate()?;
        crate::secure_store::write(
            &private_file(&app, "discord-client.dat").ok_or("Folder pengaturan tidak tersedia")?,
            &credentials,
        )?;
        let mgr = app.state::<DiscordManager>();
        mgr.state.lock().unwrap().needs_config = false;
        mgr.reset_auth.store(true, Ordering::Relaxed);
        mgr.auth_blocked.store(false, Ordering::Relaxed);
        mgr.reconnect.store(true, Ordering::Relaxed);
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn discord_reconnect(mgr: State<'_, DiscordManager>, reset_auth: bool) {
    if reset_auth {
        mgr.reset_auth.store(true, Ordering::Relaxed);
    }
    mgr.auth_blocked.store(false, Ordering::Relaxed);
    mgr.reconnect.store(true, Ordering::Relaxed);
}

#[tauri::command]
pub fn discord_set_mute(mgr: State<'_, DiscordManager>, mute: bool) -> Result<(), String> {
    mgr.send_frame("SET_VOICE_SETTINGS", json!({ "mute": mute }))
}

#[tauri::command]
pub fn discord_set_deaf(mgr: State<'_, DiscordManager>, deaf: bool) -> Result<(), String> {
    mgr.send_frame("SET_VOICE_SETTINGS", json!({ "deaf": deaf }))
}

#[tauri::command]
pub fn discord_set_user_volume(
    mgr: State<'_, DiscordManager>,
    user_id: String,
    volume: f64,
) -> Result<(), String> {
    if !volume.is_finite() {
        return Err("Volume tidak valid".into());
    }
    let vol_int = (volume.clamp(0.0, 1.0) * 100.0).round() as i64;
    mgr.send_frame(
        "SET_USER_VOICE_SETTINGS",
        json!({ "user_id": user_id, "volume": vol_int }),
    )
}

#[tauri::command]
pub fn discord_toggle_user_mute(
    mgr: State<'_, DiscordManager>,
    user_id: String,
    mute: bool,
) -> Result<(), String> {
    mgr.send_frame(
        "SET_USER_VOICE_SETTINGS",
        json!({ "user_id": user_id, "mute": mute }),
    )
}

#[tauri::command]
pub fn discord_disconnect(mgr: State<'_, DiscordManager>) -> Result<(), String> {
    mgr.send_frame("SELECT_VOICE_CHANNEL", json!({ "channel_id": null }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::windows::io::FromRawHandle;
    use windows::core::PCSTR;
    use windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES;
    use windows::Win32::System::Pipes::{ConnectNamedPipe, CreateNamedPipeA};

    fn local_pipe_pair() -> (File, File) {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let name = format!(
            r"\\.\pipe\floatingbar-regression-{}-{unique}",
            std::process::id()
        );
        let c_name = std::ffi::CString::new(name.clone()).unwrap();
        let handle = unsafe {
            CreateNamedPipeA(
                PCSTR(c_name.as_ptr().cast()),
                FILE_FLAGS_AND_ATTRIBUTES(3),
                PIPE_NOWAIT,
                1,
                4096,
                4096,
                0,
                None,
            )
        }
        .unwrap();
        let server = unsafe { File::from_raw_handle(handle.0) };
        // Nonblocking connect starts listening without holding up the test thread.
        let _ = unsafe { ConnectNamedPipe(HANDLE(server.as_raw_handle()), None) };
        let client = OpenOptions::new()
            .read(true)
            .write(true)
            .open(name)
            .unwrap();
        unsafe {
            SetNamedPipeHandleState(
                HANDLE(client.as_raw_handle()),
                Some(&PIPE_NOWAIT),
                None,
                None,
            )
        }
        .unwrap();
        (server, client)
    }

    #[test]
    fn idle_windows_pipe_survives_until_delayed_fragmented_reply() {
        let (mut server, mut client) = local_pipe_pair();
        let writer = thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            server.write_all(b"re").unwrap();
            thread::sleep(Duration::from_millis(30));
            server.write_all(b"ady").unwrap();
            thread::sleep(Duration::from_millis(100));
        });
        let mut reply = [0u8; 5];
        read_pipe_exact(
            &mut client,
            &mut reply,
            &AtomicBool::new(false),
            &AtomicBool::new(false),
            Some(Instant::now() + Duration::from_secs(2)),
        )
        .unwrap();
        assert_eq!(&reply, b"ready");
        writer.join().unwrap();
    }

    #[test]
    fn silent_windows_pipe_reaches_handshake_deadline_instead_of_false_eof() {
        let (_server, mut client) = local_pipe_pair();
        let error = read_pipe_exact(
            &mut client,
            &mut [0u8; 8],
            &AtomicBool::new(false),
            &AtomicBool::new(false),
            Some(Instant::now() + Duration::from_millis(50)),
        )
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    }

    #[test]
    fn reconnect_interrupts_idle_windows_pipe() {
        let (_server, mut client) = local_pipe_pair();
        let error = read_pipe_exact(
            &mut client,
            &mut [0u8; 8],
            &AtomicBool::new(false),
            &AtomicBool::new(true),
            None,
        )
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
    }

    #[test]
    fn full_or_disconnected_queue_returns_error_without_waiting_for_io() {
        let mgr = DiscordManager::default();
        assert!(mgr.send_frame("TEST", json!({})).is_err());
        let (tx, rx) = mpsc::sync_channel(1);
        *mgr.writer.lock().unwrap() = Some(tx);
        assert!(mgr.send_frame("FIRST", json!({})).is_ok());
        assert!(mgr.send_frame("FULL", json!({})).is_err());
        let packet = rx.recv().unwrap();
        assert_eq!(u32::from_le_bytes(packet[..4].try_into().unwrap()), 1);
        assert_eq!(
            u32::from_le_bytes(packet[4..8].try_into().unwrap()) as usize,
            packet.len() - 8
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&packet[8..]).unwrap()["cmd"],
            "FIRST"
        );
        drop(rx);
        assert!(mgr.send_frame("CLOSED", json!({})).is_err());
    }

    #[test]
    fn keepalive_reply_preserves_non_json_payload() {
        let mgr = DiscordManager::default();
        let (tx, rx) = mpsc::sync_channel(1);
        *mgr.writer.lock().unwrap() = Some(tx);
        let ping = [0, 255, 7, 42];
        mgr.write_bytes(4, &ping).unwrap();
        let packet = rx.recv().unwrap();
        assert_eq!(u32::from_le_bytes(packet[..4].try_into().unwrap()), 4);
        assert_eq!(&packet[8..], &ping);
    }

    struct SlowPipe {
        bytes: Vec<u8>,
        calls: usize,
    }
    impl Write for SlowPipe {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.calls += 1;
            if self.calls == 1 {
                return Err(io::ErrorKind::WouldBlock.into());
            }
            let n = bytes.len().min(2);
            self.bytes.extend_from_slice(&bytes[..n]);
            Ok(n)
        }
        fn flush(&mut self) -> io::Result<()> {
            panic!("pipe writes must not flush")
        }
    }

    #[test]
    fn backpressure_and_partial_writes_preserve_the_complete_frame() {
        let mut writer = SlowPipe {
            bytes: Vec::new(),
            calls: 0,
        };
        write_packet_to(&mut writer, b"complete frame").unwrap();
        assert_eq!(writer.bytes, b"complete frame");
    }

    #[test]
    fn stalled_pipe_times_out_instead_of_waiting_forever() {
        struct Stalled;
        impl Write for Stalled {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Ok(0)
            }
            fn flush(&mut self) -> io::Result<()> {
                unreachable!()
            }
        }
        assert_eq!(
            write_packet_to(&mut Stalled, b"frame").unwrap_err().kind(),
            io::ErrorKind::TimedOut
        );
    }

    #[test]
    fn channel_change_removes_old_subscriptions_and_deduplicates_new_ones() {
        let mgr = DiscordManager::default();
        let (tx, rx) = mpsc::sync_channel(32);
        *mgr.writer.lock().unwrap() = Some(tx);
        let mut current = None;
        update_channel_subscriptions(&mgr, &mut current, Some("a"));
        assert_eq!(rx.try_iter().count(), 5);
        update_channel_subscriptions(&mgr, &mut current, Some("a"));
        assert_eq!(rx.try_iter().count(), 0);
        update_channel_subscriptions(&mgr, &mut current, Some("b"));
        let packets: Vec<Value> = rx
            .try_iter()
            .map(|p| serde_json::from_slice(&p[8..]).unwrap())
            .collect();
        assert_eq!(packets.len(), 10);
        assert_eq!(packets[0]["cmd"], "UNSUBSCRIBE");
        assert_eq!(packets[0]["args"]["channel_id"], "a");
        assert_eq!(packets[1]["args"]["channel_id"], "b");
        update_channel_subscriptions(&mgr, &mut current, None);
        assert_eq!(rx.try_iter().count(), 5);
        assert_eq!(current, None);
    }
}
