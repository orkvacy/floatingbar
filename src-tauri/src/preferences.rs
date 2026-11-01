use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Preferences {
    #[serde(default)]
    pub experimental_desktop_adapters: bool,
}
#[derive(Default)]
pub struct AppPreferences(pub Mutex<Preferences>);
pub fn load(app: &AppHandle) {
    let value = crate::profile::config(app)
        .ok()
        .and_then(|p| std::fs::read(p.join("preferences.json")).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    *app.state::<AppPreferences>().0.lock().unwrap() = value;
}
pub fn experimental(app: &AppHandle) -> bool {
    app.state::<AppPreferences>()
        .0
        .lock()
        .unwrap()
        .experimental_desktop_adapters
}
#[tauri::command]
pub fn get_app_preferences(state: State<AppPreferences>) -> Preferences {
    state.0.lock().unwrap().clone()
}
#[tauri::command]
pub async fn set_experimental_adapters(app: AppHandle, enabled: bool) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppPreferences>();
        let mut prefs = state.0.lock().unwrap();
        let next = Preferences {
            experimental_desktop_adapters: enabled,
        };
        let path = crate::profile::config(&app)?.join("preferences.json");
        std::fs::create_dir_all(path.parent().ok_or("Folder pengaturan tidak tersedia")?)
            .map_err(|_| "Folder pengaturan tidak bisa dibuat")?;
        let temp = path.with_extension("tmp");
        std::fs::write(
            &temp,
            serde_json::to_vec(&next).map_err(|_| "Pengaturan tidak valid")?,
        )
        .map_err(|_| "Pengaturan tidak bisa disimpan")?;
        if std::fs::rename(&temp, path).is_err() {
            let _ = std::fs::remove_file(temp);
            return Err("Pengaturan tidak bisa diganti".into());
        }
        *prefs = next;
        Ok(())
    })
    .await
    .map_err(|_| "Pekerja pengaturan berhenti".to_string())?
}
