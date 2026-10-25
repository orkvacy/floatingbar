use serde::Serialize;
use windows::core::{Interface, Result, PWSTR};
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{
    eMultimedia, eRender, IAudioSessionControl2, IAudioSessionManager2, IMMDevice,
    IMMDeviceEnumerator, ISimpleAudioVolume, MMDeviceEnumerator,
};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct VolumeInfo {
    /// 0.0 to 1.0.
    pub level: f32,
    pub muted: bool,
    /// "app" when controlling the playing app only, "system" for the speakers.
    pub scope: &'static str,
}

/// Audio sessions are per process, so map the media app to its executable.
/// Packaged apps (e.g. Media Player) have no simple mapping and fall back to
/// the system volume.
fn target_exe(app_id: &str) -> Option<String> {
    let id = app_id.to_lowercase();
    let known = [
        ("spotify", "spotify.exe"),
        ("msedge", "msedge.exe"),
        ("chrome", "chrome.exe"),
        ("308046b0af4a39cb", "firefox.exe"),
        ("firefox", "firefox.exe"),
        ("opera", "opera.exe"),
        ("brave", "brave.exe"),
        ("vlc", "vlc.exe"),
    ];
    if let Some((_, exe)) = known.iter().find(|(k, _)| id.contains(k)) {
        return Some(exe.to_string());
    }
    id.ends_with(".exe").then(|| id.rsplit('\\').next().unwrap_or(&id).to_string())
}

pub(crate) fn exe_name(pid: u32) -> Option<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 512];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = CloseHandle(handle);
        ok.ok()?;
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        path.rsplit('\\').next().map(|s| s.to_lowercase())
    }
}

fn device() -> Result<IMMDevice> {
    unsafe {
        // Already-initialized threads report a harmless mode mismatch.
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia)
    }
}

/// Every audio session of the app's executable; browsers have several.
fn app_sessions(device: &IMMDevice, exe: &str) -> Result<Vec<ISimpleAudioVolume>> {
    unsafe {
        let manager: IAudioSessionManager2 = device.Activate(CLSCTX_ALL, None)?;
        let sessions = manager.GetSessionEnumerator()?;
        let mut found = Vec::new();
        for i in 0..sessions.GetCount()? {
            let control = sessions.GetSession(i)?;
            let Ok(control2) = control.cast::<IAudioSessionControl2>() else { continue };
            let Ok(pid) = control2.GetProcessId() else { continue };
            if exe_name(pid).as_deref() == Some(exe) {
                found.push(control.cast::<ISimpleAudioVolume>()?);
            }
        }
        Ok(found)
    }
}

enum Change {
    None,
    Level(f32),
    Muted(bool),
}

fn apply(app_id: Option<&str>, change: Change) -> Result<VolumeInfo> {
    let device = device()?;
    unsafe {
        if let Some(exe) = app_id.and_then(target_exe) {
            let sessions = app_sessions(&device, &exe)?;
            if let Some(first) = sessions.first() {
                for s in &sessions {
                    match change {
                        Change::Level(l) => s.SetMasterVolume(l, std::ptr::null())?,
                        Change::Muted(m) => s.SetMute(m, std::ptr::null())?,
                        Change::None => {}
                    }
                }
                return Ok(VolumeInfo {
                    level: first.GetMasterVolume()?,
                    muted: first.GetMute()?.as_bool(),
                    scope: "app",
                });
            }
        }
        let endpoint: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None)?;
        match change {
            Change::Level(l) => endpoint.SetMasterVolumeLevelScalar(l, std::ptr::null())?,
            Change::Muted(m) => endpoint.SetMute(m, std::ptr::null())?,
            Change::None => {}
        }
        Ok(VolumeInfo {
            level: endpoint.GetMasterVolumeLevelScalar()?,
            muted: endpoint.GetMute()?.as_bool(),
            scope: "system",
        })
    }
}

fn to_string_err(r: Result<VolumeInfo>) -> std::result::Result<VolumeInfo, String> {
    r.map_err(|e| e.message().to_string())
}

pub fn get(app_id: Option<&str>) -> std::result::Result<VolumeInfo, String> {
    to_string_err(apply(app_id, Change::None))
}

pub fn set_level(app_id: Option<&str>, level: f32) -> std::result::Result<VolumeInfo, String> {
    to_string_err(apply(app_id, Change::Level(level.clamp(0.0, 1.0))))
}

pub fn set_muted(app_id: Option<&str>, muted: bool) -> std::result::Result<VolumeInfo, String> {
    to_string_err(apply(app_id, Change::Muted(muted)))
}
