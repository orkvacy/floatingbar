//! Desktop permission adapters. COM stays on one worker; only explicit user
//! answers reach a verified one-time button. Unverified providers open their app.
use crate::permission::{Decision, PermissionRequest};
use crate::permission_provider::Provider;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use windows::core::{Result, PWSTR, VARIANT};
use windows::Win32::Foundation::{CloseHandle, BOOL, HWND, LPARAM};
use windows::Win32::Storage::Packaging::Appx::GetApplicationUserModelId;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationInvokePattern,
    TreeScope_Descendants, UIA_ButtonControlTypeId, UIA_ControlTypePropertyId, UIA_InvokePatternId,
    UIA_TextControlTypeId,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowThreadProcessId, IsWindowVisible, SetForegroundWindow, ShowWindow,
    SW_RESTORE,
};
pub const ID_BASE: u64 = 1 << 40;
const SCAN_EVERY: Duration = Duration::from_millis(1500);
struct Answer {
    id: u64,
    decision: Decision,
    expires: Instant,
    reply: Sender<std::result::Result<(), String>>,
}
pub struct UiPermissions(Mutex<Sender<Answer>>);
pub async fn answer(
    app: AppHandle,
    id: u64,
    decision: Decision,
) -> std::result::Result<(), String> {
    let (tx, rx) = mpsc::channel();
    app.state::<UiPermissions>()
        .0
        .lock()
        .unwrap()
        .send(Answer {
            id,
            decision,
            expires: Instant::now() + Duration::from_secs(7),
            reply: tx,
        })
        .map_err(|_| "Pemantau izin tidak tersedia")?;
    tauri::async_runtime::spawn_blocking(move || {
        rx.recv_timeout(Duration::from_secs(8))
            .map_err(|_| "Aplikasi belum merespons. Periksa prompt di aplikasi asal.".to_string())?
    })
    .await
    .map_err(|_| "Pemantau izin tidak tersedia".to_string())?
}
struct Prompt {
    id: u64,
    provider: Provider,
    title: String,
    hwnd: HWND,
    allow: IUIAutomationElement,
    deny: IUIAutomationElement,
}
// Retained for the separate Claude conversation panel.
pub(crate) fn claude_windows() -> Vec<HWND> {
    ai_windows(false)
        .into_iter()
        .filter(|(p, _)| *p == Provider::Claude)
        .map(|(_, w)| w)
        .collect()
}
fn ai_windows(experimental: bool) -> Vec<(Provider, HWND)> {
    unsafe extern "system" fn collect(hwnd: HWND, out: LPARAM) -> BOOL {
        let (out, experimental) = &mut *(out.0 as *mut (Vec<(Provider, HWND)>, bool));
        if IsWindowVisible(hwnd).as_bool() {
            let mut pid = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if let Some(exe) = crate::audio::exe_name(pid) {
                if Provider::from_exe(&exe).is_some() {
                    if let Some(provider) =
                        Provider::from_identity(&exe, process_app_id(pid).as_deref())
                    {
                        if provider == Provider::Claude || *experimental {
                            out.push((provider, hwnd));
                        }
                    }
                }
            }
        }
        true.into()
    }
    let mut found = (Vec::new(), experimental);
    unsafe {
        let _ = EnumWindows(Some(collect), LPARAM(&mut found as *mut _ as isize));
    }
    found.0
}

fn process_app_id(pid: u32) -> Option<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buffer = [0u16; 512];
        let mut len = buffer.len() as u32;
        let result = GetApplicationUserModelId(handle, &mut len, PWSTR(buffer.as_mut_ptr()));
        let _ = CloseHandle(handle);
        if result.0 != 0 || len == 0 || len as usize > buffer.len() {
            return None;
        }
        Some(String::from_utf16_lossy(&buffer[..len as usize - 1]))
    }
}

pub(crate) fn of_type(
    ua: &IUIAutomation,
    root: &IUIAutomationElement,
    control: i32,
) -> Result<Vec<IUIAutomationElement>> {
    unsafe {
        let cond =
            ua.CreatePropertyCondition(UIA_ControlTypePropertyId, &VARIANT::from(control))?;
        let all = root.FindAll(TreeScope_Descendants, &cond)?;
        (0..all.Length()?).map(|i| all.GetElement(i)).collect()
    }
}

pub(crate) fn name(el: &IUIAutomationElement) -> String {
    unsafe { el.CurrentName().map(|n| n.to_string()).unwrap_or_default() }
}

fn visible(el: &IUIAutomationElement) -> bool {
    unsafe {
        !el.CurrentIsOffscreen().map(|b| b.as_bool()).unwrap_or(true)
            && el.CurrentIsEnabled().map(|b| b.as_bool()).unwrap_or(false)
    }
}
fn same(ua: &IUIAutomation, a: &Prompt, b: &Prompt) -> bool {
    a.provider == b.provider
        && a.hwnd == b.hwnd
        && a.title == b.title
        && unsafe {
            ua.CompareElements(&a.allow, &b.allow)
                .map(|b| b.as_bool())
                .unwrap_or(false)
                && ua
                    .CompareElements(&a.deny, &b.deny)
                    .map(|b| b.as_bool())
                    .unwrap_or(false)
        }
}
fn find_prompts(ua: &IUIAutomation, experimental: bool) -> Vec<Prompt> {
    let mut found = Vec::new();
    let Ok(walker) = (unsafe { ua.ControlViewWalker() }) else {
        return found;
    };
    for (provider, hwnd) in ai_windows(experimental) {
        let Ok(root) = (unsafe { ua.ElementFromHandle(hwnd) }) else {
            continue;
        };
        let Ok(buttons) = of_type(ua, &root, UIA_ButtonControlTypeId.0) else {
            continue;
        };
        for allow in buttons
            .iter()
            .filter(|b| visible(b) && provider.allow_label(&name(b)))
        {
            // Pair buttons in a nearby container, never across the whole window.
            let mut parent = allow.clone();
            for _ in 0..5 {
                let Ok(next) = (unsafe { walker.GetParentElement(&parent) }) else {
                    break;
                };
                parent = next;
                if unsafe {
                    ua.CompareElements(&parent, &root)
                        .map(|b| b.as_bool())
                        .unwrap_or(true)
                } {
                    break;
                }
                let Ok(local) = of_type(ua, &parent, UIA_ButtonControlTypeId.0) else {
                    continue;
                };
                let Some(deny) = local
                    .iter()
                    .find(|b| visible(b) && provider.deny_label(&name(b)))
                else {
                    continue;
                };
                if local
                    .iter()
                    .filter(|b| visible(b) && provider.allow_label(&name(b)))
                    .count()
                    != 1
                {
                    continue;
                }
                let texts = of_type(ua, &parent, UIA_TextControlTypeId.0).unwrap_or_default();
                let parts: Vec<_> = texts
                    .iter()
                    .filter(|e| visible(e))
                    .map(name)
                    .filter(|s| !s.trim().is_empty())
                    .collect();
                let title = parts.join("\n");
                if title.len() > 6000 || !provider.prompt_context(&title) {
                    continue;
                }
                let prompt = Prompt {
                    id: 0,
                    provider,
                    title,
                    hwnd,
                    allow: allow.clone(),
                    deny: deny.clone(),
                };
                if !found.iter().any(|p| same(ua, p, &prompt)) {
                    found.push(prompt);
                }
                break;
            }
        }
    }
    found
}
// Also used by the existing Claude chat panel.
pub(crate) fn press(el: &IUIAutomationElement) {
    let _ = invoke(el);
}
fn invoke(el: &IUIAutomationElement) -> std::result::Result<(), String> {
    if !visible(el) {
        return Err("Prompt sudah berubah atau tidak aktif".into());
    }
    unsafe {
        el.GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId)
            .and_then(|p| p.Invoke())
            .map_err(|_| {
                "Tombol aplikasi tidak bisa ditekan. Buka aplikasi asal untuk menjawab.".into()
            })
    }
}
fn run(app: AppHandle, rx: Receiver<Answer>) {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
    let Ok(ua) = (unsafe {
        CoCreateInstance::<_, IUIAutomation>(&CUIAutomation, None, CLSCTX_INPROC_SERVER)
    }) else {
        return;
    };
    let mut next = ID_BASE;
    let mut current: Vec<Prompt> = Vec::new();
    loop {
        match rx.recv_timeout(SCAN_EVERY) {
            Ok(answer) => {
                // Fresh handles and identity check: never apply a queued click to a replacement prompt.
                let result = current.iter().find(|p| p.id == answer.id)
                    .and_then(|old| find_prompts(&ua, crate::preferences::experimental(&app)).into_iter().find(|new| same(&ua, old, new)))
                    .ok_or_else(|| "Prompt sudah ditutup atau berubah. Tunggu permintaan terbaru.".to_string())
                    .and_then(|p| {
                        if Instant::now() >= answer.expires { return Err("Jawaban kedaluwarsa. Periksa prompt di aplikasi asal.".into()); }
                        match answer.decision {
                        Decision::Ask => unsafe {
                            let _ = ShowWindow(p.hwnd, SW_RESTORE);
                            if SetForegroundWindow(p.hwnd).as_bool() { Ok(()) }
                            else { Err("Windows membatasi perpindahan fokus. Buka aplikasi lewat taskbar.".into()) }
                        },
                        _ if !p.provider.can_answer() => Err("Jawab permintaan ini di aplikasi asal.".into()),
                        Decision::Allow => invoke(&p.allow),
                        Decision::Deny => invoke(&p.deny),
                    }});
                let _ = answer.reply.send(result);
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        let mut fresh = find_prompts(&ua, crate::preferences::experimental(&app));
        for p in &mut fresh {
            if let Some(old) = current.iter().find(|old| same(&ua, old, p)) {
                p.id = old.id;
            } else {
                next += 1;
                p.id = next;
                crate::permission::publish(
                    &app,
                    PermissionRequest {
                        id: p.id,
                        provider: p.provider.id().into(),
                        provider_name: p.provider.name().into(),
                        can_answer: p.provider.can_answer(),
                        can_allow: None,
                        tool: "Permintaan izin".into(),
                        summary: p.title.clone(),
                        project: format!("aplikasi {}", p.provider.name()),
                    },
                );
            }
        }
        for old in &current {
            if !fresh.iter().any(|p| p.id == old.id) {
                crate::permission::close(&app, old.id);
            }
        }
        current = fresh;
    }
}
pub fn spawn(app: &AppHandle) {
    let (tx, rx) = mpsc::channel();
    app.manage(UiPermissions(Mutex::new(tx)));
    let app = app.clone();
    thread::spawn(move || run(app, rx));
}
