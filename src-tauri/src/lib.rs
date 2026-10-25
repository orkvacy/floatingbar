mod ask;
mod audio;
mod battery;
mod claude_app;
mod media;
mod notify;
mod permission;
mod shortcuts;
mod tray;
mod ui_permission;

use battery::BatteryInfo;
use media::{Action, MediaInfo};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, State, WebviewWindow, WindowEvent};
use windows::Win32::Foundation::POINT;
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

/// Interactive area of the pill, in CSS px relative to the window.
#[derive(Deserialize, Clone, Copy, Debug)]
struct HitRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
struct SavedPosition {
    x: i32,
    y: i32,
}

#[derive(Default)]
struct AppState {
    media: Mutex<Option<MediaInfo>>,
    battery: Mutex<Option<BatteryInfo>>,
    hit: Mutex<Option<HitRect>>,
    /// Why notifications can't be read, if they can't.
    notify_error: Mutex<Option<String>>,
    /// Latest window position, written to disk by the saver thread.
    position: Mutex<Option<SavedPosition>>,
}

#[tauri::command]
fn get_media(state: State<AppState>) -> Option<MediaInfo> {
    state.media.lock().unwrap().clone()
}

#[tauri::command]
fn get_battery(state: State<AppState>) -> Option<BatteryInfo> {
    state.battery.lock().unwrap().clone()
}

#[tauri::command]
fn get_notify_error(state: State<AppState>) -> Option<String> {
    state.notify_error.lock().unwrap().clone()
}

#[tauri::command]
fn set_hit_region(state: State<AppState>, rect: HitRect) {
    *state.hit.lock().unwrap() = Some(rect);
}

#[tauri::command]
fn reset_position(window: WebviewWindow) -> Result<(), String> {
    center_window(&window).map_err(|e| e.to_string())
}

// Async so waiting on the Claude app never blocks the UI thread.
#[tauri::command]
async fn claude_chats(app: AppHandle) -> Result<claude_app::ChatList, String> {
    claude_app::list(&app)
}

#[tauri::command]
fn claude_set_mode(app: AppHandle, mode: String) {
    claude_app::set_mode(&app, mode);
}

#[tauri::command]
fn claude_open(app: AppHandle, chat: Option<String>) {
    claude_app::open(&app, chat);
}

#[tauri::command]
fn claude_send(app: AppHandle, chat: Option<String>, prompt: String, paste_image: bool) {
    claude_app::send_prompt(&app, chat, prompt, paste_image);
}

#[tauri::command]
fn open_in_claude(prompt: String) -> Result<(), String> {
    ask::open_in_app(&prompt)
}

#[tauri::command]
fn answer_permission(app: AppHandle, id: u64, decision: permission::Decision) {
    if id >= ui_permission::ID_BASE {
        ui_permission::answer(&app, id, decision);
    } else {
        permission::answer(&app, id, decision);
    }
}

#[tauri::command]
fn get_shortcuts(app: AppHandle) -> Vec<shortcuts::Shortcut> {
    shortcuts::load(&app)
}

/// Takes an index rather than a target so the page can only launch what the
/// user put in shortcuts.json.
#[tauri::command]
fn open_shortcut(app: AppHandle, index: usize) -> Result<(), String> {
    let list = shortcuts::load(&app);
    let shortcut = list.get(index).ok_or("shortcut tidak ditemukan")?;
    shortcuts::launch(&shortcut.target)
}

#[tauri::command]
fn edit_shortcuts(app: AppHandle) -> Result<(), String> {
    shortcuts::edit(&app)
}

fn playing_app(state: &AppState) -> Option<String> {
    state.media.lock().unwrap().as_ref().map(|m| m.app_id.clone())
}

// Async so Core Audio runs on a worker thread instead of the UI thread.
#[tauri::command]
async fn get_volume(state: State<'_, AppState>) -> Result<audio::VolumeInfo, String> {
    audio::get(playing_app(&state).as_deref())
}

#[tauri::command]
async fn set_volume(state: State<'_, AppState>, level: f32) -> Result<audio::VolumeInfo, String> {
    audio::set_level(playing_app(&state).as_deref(), level)
}

#[tauri::command]
async fn set_muted(state: State<'_, AppState>, muted: bool) -> Result<audio::VolumeInfo, String> {
    audio::set_muted(playing_app(&state).as_deref(), muted)
}

#[tauri::command]
fn media_play_pause() -> Result<(), String> {
    media::control(Action::PlayPause)
}

#[tauri::command]
fn media_next() -> Result<(), String> {
    media::control(Action::Next)
}

#[tauri::command]
fn media_previous() -> Result<(), String> {
    media::control(Action::Previous)
}

/// Place the window at the top-center of the monitor it is on.
pub(crate) fn center_window(window: &WebviewWindow) -> tauri::Result<()> {
    let monitor = window.current_monitor()?.or(window.primary_monitor()?);
    if let Some(m) = monitor {
        let size = window.outer_size()?;
        let x = m.position().x + (m.size().width as i32 - size.width as i32) / 2;
        window.set_position(PhysicalPosition::new(x, m.position().y))?;
    }
    Ok(())
}

fn position_file(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("position.json"))
}

/// Restore the position the user dragged the pill to, unless that spot is no
/// longer on any monitor (e.g. an external screen was unplugged).
fn restore_position(app: &AppHandle, window: &WebviewWindow) -> tauri::Result<()> {
    let saved: Option<SavedPosition> = position_file(app)
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok());
    if let Some(pos) = saved {
        let size = window.outer_size()?;
        let anchor = (pos.x + size.width as i32 / 2, pos.y + 20);
        let visible = window.available_monitors()?.iter().any(|m| {
            let (mx, my) = (m.position().x, m.position().y);
            let (mw, mh) = (m.size().width as i32, m.size().height as i32);
            anchor.0 >= mx && anchor.0 < mx + mw && anchor.1 >= my && anchor.1 < my + mh
        });
        if visible {
            return window.set_position(PhysicalPosition::new(pos.x, pos.y));
        }
    }
    center_window(window)
}

/// Window moves arrive many times per second while dragging, so persist
/// them from a slow loop instead of on every event.
fn spawn_position_saver(app: AppHandle) {
    thread::spawn(move || {
        let mut written = None;
        loop {
            thread::sleep(Duration::from_secs(1));
            let pos = *app.state::<AppState>().position.lock().unwrap();
            if pos.is_none() || pos == written {
                continue;
            }
            if let Some(path) = position_file(&app) {
                let _ = fs::create_dir_all(path.parent().unwrap());
                if fs::write(&path, serde_json::to_string(&pos).unwrap()).is_ok() {
                    written = pos;
                }
            }
        }
    });
}

fn spawn_media_loop(app: AppHandle) {
    thread::spawn(move || {
        let mut cache = media::ThumbCache::default();
        let mut manager = None;
        loop {
            if manager.is_none() {
                manager = media::manager().ok();
            }
            let info = match &manager {
                Some(m) => media::read(m, &mut cache).unwrap_or(None),
                None => None,
            };
            let state = app.state::<AppState>();
            let mut current = state.media.lock().unwrap();
            if *current != info {
                *current = info.clone();
                let _ = app.emit("media", info);
            }
            drop(current);
            thread::sleep(Duration::from_millis(700));
        }
    });
}

fn spawn_battery_loop(app: AppHandle) {
    thread::spawn(move || loop {
        let info = Some(battery::read());
        let state = app.state::<AppState>();
        let mut current = state.battery.lock().unwrap();
        if *current != info {
            *current = info.clone();
            let _ = app.emit("battery", info);
        }
        drop(current);
        thread::sleep(Duration::from_secs(3));
    });
}

/// The NotificationChanged event only works for packaged apps, so poll the
/// notification center and emit the toasts we haven't seen yet.
fn spawn_notification_loop(app: AppHandle) {
    thread::spawn(move || {
        let logos = notify::LogoCache::default();
        let set_error = |err: Option<String>| {
            let state = app.state::<AppState>();
            let mut current = state.notify_error.lock().unwrap();
            if *current != err {
                *current = err.clone();
                let _ = app.emit("notify-error", err);
            }
        };
        loop {
            let listener = match notify::listener() {
                Ok(l) => l,
                Err(e) => {
                    set_error(Some(e));
                    thread::sleep(Duration::from_secs(10));
                    continue;
                }
            };
            set_error(None);
            // Toasts already in the notification center at startup are old news.
            let mut seen: Option<HashSet<u32>> = None;
            loop {
                let Ok(list) = notify::current(&listener) else {
                    break;
                };
                let ids: HashSet<u32> = list.iter().filter_map(|n| n.Id().ok()).collect();
                if let Some(seen) = &seen {
                    for n in list.iter().filter(|n| n.Id().is_ok_and(|id| !seen.contains(&id))) {
                        if let Ok(info) = notify::parse(n, &logos) {
                            let _ = app.emit("notification", info);
                        }
                    }
                }
                seen = Some(ids);
                thread::sleep(Duration::from_millis(800));
            }
        }
    });
}

/// The window is transparent and larger than the pill, so clicks must pass
/// through everywhere except the pill itself. Once the window ignores the
/// cursor the webview receives no mouse events, so poll the global cursor.
fn spawn_hover_loop(app: AppHandle, window: WebviewWindow) {
    thread::spawn(move || {
        let mut hovering = false;
        let _ = window.set_ignore_cursor_events(true);
        loop {
            thread::sleep(Duration::from_millis(30));
            if !window.is_visible().unwrap_or(true) {
                continue;
            }
            let rect = *app.state::<AppState>().hit.lock().unwrap();
            let (Some(rect), Ok(origin), Ok(scale)) =
                (rect, window.outer_position(), window.scale_factor())
            else {
                continue;
            };
            let mut p = POINT::default();
            if unsafe { GetCursorPos(&mut p) }.is_err() {
                continue;
            }
            let x = (p.x - origin.x) as f64 / scale;
            let y = (p.y - origin.y) as f64 / scale;
            let inside = x >= rect.x
                && x <= rect.x + rect.width
                && y >= rect.y
                && y <= rect.y + rect.height;
            if inside != hovering {
                hovering = inside;
                let _ = window.set_ignore_cursor_events(!inside);
                let _ = app.emit("hover", inside);
            }
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Launching again (e.g. from the Start menu while autostart already
        // ran it) just brings the pill back instead of opening a second one.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::set_visible(app, true);
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(AppState::default())
        .manage(permission::Pending::default())        .invoke_handler(tauri::generate_handler![
            get_media,
            get_battery,
            get_notify_error,
            set_hit_region,
            reset_position,
            answer_permission,
            open_in_claude,
            claude_chats,
            claude_set_mode,
            claude_open,
            claude_send,
            get_shortcuts,
            open_shortcut,
            edit_shortcuts,
            get_volume,
            set_volume,
            set_muted,
            media_play_pause,
            media_next,
            media_previous
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::Moved(pos) = event {
                *window.state::<AppState>().position.lock().unwrap() =
                    Some(SavedPosition { x: pos.x, y: pos.y });
            }
        })
        .setup(|app| {
            let window = app.get_webview_window("main").expect("main window");
            restore_position(app.handle(), &window)?;
            window.show()?;
            tray::create(app.handle())?;
            spawn_position_saver(app.handle().clone());
            spawn_media_loop(app.handle().clone());
            spawn_battery_loop(app.handle().clone());
            spawn_notification_loop(app.handle().clone());
            permission::spawn_server(app.handle().clone());
            ui_permission::spawn(app.handle());
            claude_app::spawn(app.handle());
            spawn_hover_loop(app.handle().clone(), window);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running FloatingBar");
}
