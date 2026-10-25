use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tauri::{AppHandle, Manager};
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Shortcut {
    pub name: String,
    /// A URL, an app protocol (e.g. "whatsapp:") or a path to a program/file.
    pub target: String,
    /// Optional image URL; websites fall back to their favicon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

fn defaults() -> Vec<Shortcut> {
    let s = |name: &str, target: &str| Shortcut { name: name.into(), target: target.into(), icon: None };
    vec![
        s("WhatsApp", "https://web.whatsapp.com"),
        s("GitHub", "https://github.com"),
        s("Portal ASCII", "https://ascii.web.id"),
        s("Google Drive", "https://drive.google.com"),
    ]
}

pub fn file(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("shortcuts.json"))
}

/// Read on every call so edits to the file show up without a restart.
pub fn load(app: &AppHandle) -> Vec<Shortcut> {
    let Some(path) = file(app) else {
        return defaults();
    };
    match fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_else(|_| defaults()),
        Err(_) => {
            let list = defaults();
            let _ = fs::create_dir_all(path.parent().unwrap());
            let _ = fs::write(&path, serde_json::to_string_pretty(&list).unwrap());
            list
        }
    }
}

/// Hands the target to whatever Windows associates with it: the default
/// browser for URLs, the registered app for protocols. (explorer.exe drops
/// URLs with a query string and opens Documents instead.)
pub fn launch(target: &str) -> Result<(), String> {
    let result = unsafe {
        ShellExecuteW(
            HWND(std::ptr::null_mut()),
            &HSTRING::from("open"),
            &HSTRING::from(target),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    // Values above 32 mean success.
    if result.0 as isize > 32 {
        Ok(())
    } else {
        Err(format!("tidak bisa membuka {target}"))
    }
}

/// Opens shortcuts.json in Notepad for editing.
pub fn edit(app: &AppHandle) -> Result<(), String> {
    load(app); // make sure the file exists
    let path = file(app).ok_or("no config dir")?;
    Command::new("notepad.exe").arg(path).spawn().map(|_| ()).map_err(|e| e.to_string())
}
