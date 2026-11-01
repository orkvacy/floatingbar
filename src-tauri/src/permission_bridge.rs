use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Serialize, Deserialize)]
pub struct Credentials {
    pub token: String,
    pub port: u16,
    pub owner_pid: u32,
    pub owner_exe: String,
}
pub fn path() -> Result<PathBuf, String> {
    Ok(crate::profile::bridge_config()?.join("permission-hook.dat"))
}
pub fn create(port: u16, token: String) -> Result<(), String> {
    let owner_exe = std::env::current_exe()
        .map_err(|_| "Lokasi aplikasi tidak tersedia")?
        .to_string_lossy()
        .into_owned();
    crate::secure_store::write(
        &path()?,
        &Credentials {
            token,
            port,
            owner_pid: std::process::id(),
            owner_exe,
        },
    )
}
fn owner_alive(c: &Credentials) -> bool {
    use windows::{
        core::PWSTR,
        Win32::{
            Foundation::CloseHandle,
            System::Threading::{
                OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
                PROCESS_QUERY_LIMITED_INFORMATION,
            },
        },
    };
    unsafe {
        let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, c.owner_pid) else {
            return false;
        };
        let mut bytes = [0u16; 32768];
        let mut len = bytes.len() as u32;
        let result = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(bytes.as_mut_ptr()),
            &mut len,
        );
        let _ = CloseHandle(handle);
        result.is_ok()
            && String::from_utf16_lossy(&bytes[..len as usize]).eq_ignore_ascii_case(&c.owner_exe)
    }
}
// Always falls back to Claude's own prompt. No secret goes to stdout or argv.
pub fn run() {
    let result = (|| -> Result<Value, String> {
        let mut body = Vec::new();
        std::io::stdin()
            .take((crate::hook_http::MAX_BODY + 1) as u64)
            .read_to_end(&mut body)
            .map_err(|_| "Input tidak terbaca")?;
        if body.len() > crate::hook_http::MAX_BODY {
            return Err("Input terlalu besar".into());
        }
        let c: Credentials = crate::secure_store::read(&path()?)?;
        if !owner_alive(&c) {
            return Err("Aplikasi tidak berjalan".into());
        }
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(1))
            .timeout(Duration::from_secs(58))
            .build()
            .map_err(|_| "HTTP tidak tersedia")?;
        let mut response = client
            .post(format!("http://127.0.0.1:{}/permission", c.port))
            .bearer_auth(c.token)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .map_err(|_| "Aplikasi tidak merespons")?;
        if !response.status().is_success() {
            return Err("Permintaan ditolak".into());
        }
        let mut bytes = Vec::new();
        response
            .by_ref()
            .take(64 * 1024)
            .read_to_end(&mut bytes)
            .map_err(|_| "Respons tidak terbaca")?;
        serde_json::from_slice(&bytes).map_err(|_| "Respons tidak valid".into())
    })()
    .unwrap_or_else(|_| json!({}));
    let result = crate::hook_config::sanitize_response(result);
    let _ = writeln!(std::io::stdout(), "{result}");
}
pub fn configure(install: bool) -> Result<usize, String> {
    let path = crate::profile::claude_settings()?;
    let before = match std::fs::read(&path) {
        Ok(b) => Some(b),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(_) => return Err("Konfigurasi Claude tidak bisa dibaca".into()),
    };
    let settings = before
        .as_deref()
        .map(serde_json::from_slice)
        .transpose()
        .map_err(|_| "JSON konfigurasi Claude tidak valid; file tidak diubah")?
        .unwrap_or_else(|| json!({}));
    let exe = std::env::current_exe().map_err(|_| "Lokasi aplikasi tidak tersedia")?;
    let mut args = vec!["--permission-hook".to_owned()];
    if let Some(dir) = crate::profile::test_dir() {
        args.extend(["--test-profile".into(), dir.to_string_lossy().into_owned()]);
    }
    let (settings, count) =
        crate::hook_config::merge(settings, &exe.to_string_lossy(), args, install)?;
    if count == 0 {
        return Ok(0);
    }
    let bytes = serde_json::to_vec_pretty(&settings).map_err(|_| "Konfigurasi tidak valid")?;
    if before.as_deref() == Some(bytes.as_slice()) {
        return Ok(count);
    }
    std::fs::create_dir_all(path.parent().ok_or("Folder konfigurasi tidak tersedia")?)
        .map_err(|_| "Folder konfigurasi tidak bisa dibuat")?;
    if let Some(original) = before.as_ref() {
        let backup = path.with_extension("json.floatingbar-backup");
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(backup)
        {
            Ok(mut f) => f
                .write_all(original)
                .map_err(|_| "Backup konfigurasi tidak bisa disimpan")?,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err("Backup konfigurasi tidak bisa dibuat".into()),
        }
    }
    let temp = path.with_extension("floatingbar.tmp");
    std::fs::write(&temp, bytes).map_err(|_| "Konfigurasi tidak bisa disimpan")?;
    if std::fs::rename(&temp, &path).is_err() {
        let _ = std::fs::remove_file(temp);
        return Err("Konfigurasi tidak bisa diganti; file lama tetap tersedia".into());
    }
    Ok(count)
}
