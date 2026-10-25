use crate::media::image_data_url;
use base64::Engine;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use windows::core::{Interface, HSTRING};
use windows::ApplicationModel::{AppInfo, Package};
use windows::Foundation::Size;
use windows::UI::Notifications::Management::{
    UserNotificationListener as Listener, UserNotificationListenerAccessStatus as Access,
};
use windows::UI::Notifications::{KnownNotificationBindings, NotificationKinds, UserNotification};

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NotificationInfo {
    pub id: u32,
    pub app: String,
    pub title: String,
    pub body: String,
    /// App logo as a data URL.
    pub icon: Option<String>,
}

/// Needs "Notification access" allowed for desktop apps in Windows privacy settings.
pub fn listener() -> Result<Listener, String> {
    let open = || -> windows::core::Result<(Listener, Access)> {
        let l = Listener::Current()?;
        let mut status = l.GetAccessStatus()?;
        if status != Access::Allowed {
            status = l.RequestAccessAsync()?.get()?;
        }
        Ok((l, status))
    };
    match open() {
        Ok((l, Access::Allowed)) => Ok(l),
        Ok(_) => Err("Akses notifikasi belum diizinkan".into()),
        Err(e) => Err(format!("Listener notifikasi gagal: {}", e.message())),
    }
}

pub fn current(l: &Listener) -> windows::core::Result<Vec<UserNotification>> {
    let list = l.GetNotificationsAsync(NotificationKinds::Toast)?.get()?;
    (0..list.Size()?).map(|i| list.GetAt(i)).collect()
}

/// App logos by AppUserModelId. Reading a packaged app's logo (e.g. Claude)
/// can block forever, which froze the whole notification loop, so each logo
/// is read once on its own thread and the loop only ever uses what's cached.
#[derive(Default)]
pub struct LogoCache(Arc<Mutex<HashMap<String, Option<String>>>>);

impl LogoCache {
    fn get(&self, aumid: &str) -> Option<String> {
        let mut map = self.0.lock().unwrap();
        if let Some(logo) = map.get(aumid) {
            return logo.clone();
        }
        // Mark as attempted so a hung read is never retried.
        map.insert(aumid.to_string(), None);
        let (map, aumid) = (self.0.clone(), aumid.to_string());
        thread::spawn(move || {
            let logo = read_logo(&aumid).ok().flatten();
            map.lock().unwrap().insert(aumid, logo);
        });
        None
    }
}

fn read_logo(aumid: &str) -> windows::core::Result<Option<String>> {
    let info = AppInfo::GetFromAppUserModelId(&HSTRING::from(aumid))?;
    // Packaged apps (Claude, Store apps): read the logo PNG straight from the
    // install folder; their logo stream is the read that never returns.
    if let Some(bytes) = info.Package().ok().and_then(|p| package_logo(&p)) {
        let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
        return Ok(Some(format!("data:image/png;base64,{b64}")));
    }
    let logo = info.DisplayInfo()?.GetLogo(Size { Width: 64.0, Height: 64.0 })?;
    image_data_url(&logo.cast()?)
}

fn package_logo(package: &Package) -> Option<Vec<u8>> {
    // The manifest logo, e.g. file:///C:/Program%20Files/WindowsApps/…/assets/StoreLogo.png.
    // Only scale- or size-qualified variants of it may exist on disk.
    let uri = package.Logo().ok()?.AbsoluteUri().ok()?.to_string();
    let path = PathBuf::from(uri.strip_prefix("file:///")?.replace("%20", " "));
    let dir = path.parent()?;
    let stem = path.file_stem()?.to_string_lossy();
    // Unplated (transparent) variants look best on the black pill.
    let candidates = [
        "Square44x44Logo.targetsize-48_altform-unplated.png".to_string(),
        "Square44x44Logo.targetsize-48.png".to_string(),
        "Square44x44Logo.scale-200.png".to_string(),
        format!("{stem}.scale-200.png"),
        format!("{stem}.scale-100.png"),
        format!("{stem}.png"),
    ];
    candidates.iter().find_map(|name| fs::read(dir.join(name)).ok())
}

pub fn parse(n: &UserNotification, logos: &LogoCache) -> windows::core::Result<NotificationInfo> {
    let (app, icon) = match n.AppInfo() {
        Ok(info) => (
            info.DisplayInfo().and_then(|d| d.DisplayName()).map(|s| s.to_string()).unwrap_or_default(),
            info.AppUserModelId().ok().and_then(|id| logos.get(&id.to_string())),
        ),
        Err(_) => (String::new(), None),
    };

    // Toast text: first line is the title, the rest is the body.
    let binding = n
        .Notification()?
        .Visual()?
        .GetBinding(&KnownNotificationBindings::ToastGeneric()?)?;
    let texts = binding.GetTextElements()?;
    let mut lines = Vec::new();
    for i in 0..texts.Size()? {
        let t = texts.GetAt(i)?.Text()?.to_string();
        if !t.trim().is_empty() {
            lines.push(t);
        }
    }
    let title = if lines.is_empty() { String::new() } else { lines.remove(0) };

    Ok(NotificationInfo { id: n.Id()?, app, title, body: lines.join("\n"), icon })
}
