//! Answers permission prompts shown inside the Claude desktop app itself
//! (e.g. connector tools in Chat), which no hook can reach.
//!
//! The app's window exposes its buttons through UI Automation, the API screen
//! readers use. A background thread looks for an "Allow once" button, shows
//! the prompt on the pill, and presses the real button when the user answers
//! there. Everything COM stays on that one thread; answers arrive by channel.
//! Matching is by the English button labels ("Allow once", "Decline").

use crate::permission::{Decision, PermissionRequest};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use windows::core::{Result, VARIANT};
use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationInvokePattern, TreeScope_Descendants,
    UIA_ButtonControlTypeId, UIA_ControlTypePropertyId, UIA_InvokePatternId, UIA_TextControlTypeId,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowThreadProcessId, IsWindowVisible, SetForegroundWindow, ShowWindow, SW_RESTORE,
};

/// Ids above this belong to app prompts; below, to hook requests.
pub const ID_BASE: u64 = 1 << 40;
/// Walking the app's accessibility tree costs a little CPU on both sides.
const SCAN_EVERY: Duration = Duration::from_millis(1500);

pub struct UiPermissions(Mutex<Sender<(u64, Decision)>>);

pub fn answer(app: &AppHandle, id: u64, decision: Decision) {
    let _ = app.state::<UiPermissions>().0.lock().unwrap().send((id, decision));
}

struct Prompt {
    id: u64,
    title: String,
    hwnd: HWND,
    allow: IUIAutomationElement,
    deny: IUIAutomationElement,
}

pub(crate) fn claude_windows() -> Vec<HWND> {
    unsafe extern "system" fn collect(hwnd: HWND, out: LPARAM) -> BOOL {
        let out = &mut *(out.0 as *mut Vec<HWND>);
        if IsWindowVisible(hwnd).as_bool() {
            let mut pid = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if crate::audio::exe_name(pid).as_deref() == Some("claude.exe") {
                out.push(hwnd);
            }
        }
        true.into()
    }
    let mut found = Vec::new();
    unsafe {
        let _ = EnumWindows(Some(collect), LPARAM(&mut found as *mut _ as isize));
    }
    found
}

pub(crate) fn of_type(ua: &IUIAutomation, root: &IUIAutomationElement, control: i32) -> Result<Vec<IUIAutomationElement>> {
    unsafe {
        let cond = ua.CreatePropertyCondition(UIA_ControlTypePropertyId, &VARIANT::from(control))?;
        let all = root.FindAll(TreeScope_Descendants, &cond)?;
        (0..all.Length()?).map(|i| all.GetElement(i)).collect()
    }
}

pub(crate) fn name(el: &IUIAutomationElement) -> String {
    unsafe { el.CurrentName().map(|n| n.to_string()).unwrap_or_default() }
}

/// The open prompt in any Claude window, if there is one.
fn find_prompt(ua: &IUIAutomation) -> Option<(HWND, String, IUIAutomationElement, IUIAutomationElement)> {
    for hwnd in claude_windows() {
        let Ok(root) = (unsafe { ua.ElementFromHandle(hwnd) }) else { continue };
        let Ok(buttons) = of_type(ua, &root, UIA_ButtonControlTypeId.0) else { continue };
        // Button names may carry their shortcut, e.g. "Allow once Ctrl Enter".
        let find = |prefix: &str| buttons.iter().find(|b| name(b).starts_with(prefix)).cloned();
        let (Some(allow), Some(deny)) = (find("Allow once"), find("Decline")) else { continue };
        let title = of_type(ua, &root, UIA_TextControlTypeId.0)
            .ok()
            .and_then(|texts| texts.iter().map(name).find(|t| t.contains("wants to")))
            .unwrap_or_else(|| "Claude minta izin memakai tool".into());
        return Some((hwnd, title, allow, deny));
    }
    None
}

pub(crate) fn press(el: &IUIAutomationElement) {
    unsafe {
        if let Ok(p) = el.GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId) {
            let _ = p.Invoke();
        }
    }
}

fn run(app: AppHandle, rx: Receiver<(u64, Decision)>) {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
    let Ok(ua) = (unsafe { CoCreateInstance::<_, IUIAutomation>(&CUIAutomation, None, CLSCTX_INPROC_SERVER) })
    else {
        eprintln!("ui automation unavailable");
        return;
    };
    let mut next = ID_BASE;
    let mut current: Option<Prompt> = None;
    loop {
        match rx.recv_timeout(SCAN_EVERY) {
            Ok((id, decision)) => {
                if let Some(p) = current.as_ref().filter(|p| p.id == id) {
                    match decision {
                        Decision::Allow => press(&p.allow),
                        Decision::Deny => press(&p.deny),
                        Decision::Ask => unsafe {
                            let _ = ShowWindow(p.hwnd, SW_RESTORE);
                            let _ = SetForegroundWindow(p.hwnd);
                        },
                    }
                }
                // Rescan right away so the card closes as soon as the prompt does.
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }

        match (find_prompt(&ua), &current) {
            // Same prompt still open: refresh the button handles.
            (Some((hwnd, title, allow, deny)), Some(p)) if p.title == title => {
                current = Some(Prompt { id: p.id, title, hwnd, allow, deny });
            }
            (Some((hwnd, title, allow, deny)), old) => {
                if let Some(old) = old {
                    let _ = app.emit("permission-closed", old.id);
                }
                next += 1;
                let request = PermissionRequest {
                    id: next,
                    tool: "Claude".into(),
                    summary: title.clone(),
                    project: "aplikasi Claude".into(),
                };
                let _ = app.emit("permission", request);
                current = Some(Prompt { id: next, title, hwnd, allow, deny });
            }
            (None, Some(old)) => {
                let _ = app.emit("permission-closed", old.id);
                current = None;
            }
            (None, None) => {}
        }
    }
}

pub fn spawn(app: &AppHandle) {
    let (tx, rx) = mpsc::channel();
    app.manage(UiPermissions(Mutex::new(tx)));
    let app = app.clone();
    thread::spawn(move || run(app, rx));
}
