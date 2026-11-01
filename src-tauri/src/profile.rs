use std::path::PathBuf;
use tauri::{AppHandle, Manager};

pub fn test_dir() -> Option<PathBuf> {
    if std::env::var_os("FLOATINGBAR_TEST_MODE").as_deref() != Some(std::ffi::OsStr::new("1")) {
        return None;
    }
    std::env::var_os("FLOATINGBAR_TEST_PROFILE")
        .map(PathBuf::from)
        .filter(|p| {
            p.is_absolute()
                && p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("floatingbar-test-"))
        })
}
pub fn config(app: &AppHandle) -> Result<PathBuf, String> {
    test_dir()
        .map(Ok)
        .unwrap_or_else(|| app.path().app_config_dir().map_err(|e| e.to_string()))
}
pub fn bridge_config() -> Result<PathBuf, String> {
    test_dir().map(Ok).unwrap_or_else(|| {
        std::env::var_os("APPDATA")
            .map(|p| PathBuf::from(p).join("id.orkvacy.floatingbar"))
            .ok_or("Folder APPDATA tidak tersedia".into())
    })
}
pub fn claude_settings() -> Result<PathBuf, String> {
    if let Some(dir) = test_dir() {
        return Ok(dir.join("claude/settings.json"));
    }
    if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR") {
        return Ok(PathBuf::from(dir).join("settings.json"));
    }
    std::env::var_os("USERPROFILE")
        .map(|p| PathBuf::from(p).join(".claude/settings.json"))
        .ok_or("Folder pengguna tidak tersedia".into())
}
