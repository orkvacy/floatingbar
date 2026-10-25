use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_autostart::ManagerExt;

/// Show or hide the pill, keeping the tray label in sync.
pub fn set_visible(app: &AppHandle, visible: bool) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let _ = if visible { window.show() } else { window.hide() };
    if let Some(item) = app.try_state::<MenuItem<Wry>>() {
        let _ = item.set_text(if visible { "Sembunyikan pill" } else { "Tampilkan pill" });
    }
}

fn toggle(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        set_visible(app, !window.is_visible().unwrap_or(true));
    }
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let autostart_on = app.autolaunch().is_enabled().unwrap_or(false);

    let toggle_item = MenuItem::with_id(app, "toggle", "Sembunyikan pill", true, None::<&str>)?;
    let reset = MenuItem::with_id(app, "reset", "Kembalikan ke tengah atas", true, None::<&str>)?;
    let shortcuts = MenuItem::with_id(app, "shortcuts", "Atur shortcut…", true, None::<&str>)?;
    let autostart = CheckMenuItem::with_id(
        app,
        "autostart",
        "Jalan saat Windows start",
        true,
        autostart_on,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", "Keluar", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &toggle_item,
            &reset,
            &shortcuts,
            &PredefinedMenuItem::separator(app)?,
            &autostart,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;
    app.manage(toggle_item);

    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().cloned().expect("app icon"))
        .tooltip("FloatingBar")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle(tray.app_handle());
            }
        })
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "toggle" => toggle(app),
            "reset" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = crate::center_window(&window);
                }
                set_visible(app, true);
            }
            "shortcuts" => {
                if let Err(e) = crate::shortcuts::edit(app) {
                    eprintln!("shortcuts: {e}");
                }
            }
            "autostart" => {
                let launcher = app.autolaunch();
                let result = if launcher.is_enabled().unwrap_or(false) {
                    launcher.disable()
                } else {
                    launcher.enable()
                };
                if let Err(e) = result {
                    eprintln!("autostart: {e}");
                }
                // The menu flips the check on click; show the real state instead.
                let _ = autostart.set_checked(launcher.is_enabled().unwrap_or(false));
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}
