//! Drives the Claude desktop app from the pill through UI Automation: pick
//! Chat or Code mode, open a conversation from the sidebar, send a prompt
//! (optionally with a pasted image), and mirror the conversation back to the
//! pill as it updates.
//!
//! This leans on the app's current accessibility tree (English labels: the
//! "Claude"/"Code" mode radios, "More options for <chat>" sidebar buttons,
//! the prompt box and Send/Stop buttons, "Message N" groups and the
//! "You said:" prefix on user messages), so an app redesign can break it.
//! Like ui_permission, all COM work stays on one thread fed by a channel.

use crate::ui_permission::{claude_windows, name, of_type, press};
use serde::Serialize;
use std::collections::HashSet;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use windows::core::{Result, BSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationSelectionItemPattern,
    IUIAutomationTreeWalker, IUIAutomationValuePattern, UIA_ButtonControlTypeId, UIA_DocumentControlTypeId,
    UIA_EditControlTypeId, UIA_ExpandCollapsePatternId, UIA_GroupControlTypeId, UIA_ListControlTypeId,
    UIA_ListItemControlTypeId, UIA_RadioButtonControlTypeId, UIA_SelectionItemPatternId,
    UIA_StatusBarControlTypeId, UIA_TextControlTypeId, UIA_ToolBarControlTypeId, UIA_ValuePatternId,
    UIA_PATTERN_ID,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{keybd_event, KEYEVENTF_KEYUP, VK_CONTROL};
use windows::Win32::UI::WindowsAndMessaging::{SetForegroundWindow, ShowWindow, SW_RESTORE};

const POLL: Duration = Duration::from_millis(700);
/// Stop following a conversation after this long.
const WATCH_TIMEOUT: Duration = Duration::from_secs(300);
/// How many recent messages the pill shows.
const HISTORY_LEN: usize = 12;

// Chat and Code mode label the same controls differently.
const PROMPT_LABELS: [&str; 2] = ["Prompt", "Write your prompt to Claude"];
const SEND_LABELS: [&str; 2] = ["Send", "Send message"];
// Exact names only: Code mode also has "Stop this task" for background tasks.
const STOP_LABELS: [&str; 3] = ["Stop", "Stop response", "Stop generating"];

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ChatList {
    /// "chat", "code", or "" when the mode switch isn't visible.
    pub mode: String,
    /// Conversation titles from the sidebar, top to bottom.
    pub chats: Vec<String>,
    /// The conversation open in the app right now, if it's one of `chats`.
    pub current: Option<String>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Message {
    /// "user" or "claude".
    pub role: &'static str,
    pub text: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct History {
    messages: Vec<Message>,
    /// Claude is still writing.
    working: bool,
    error: Option<String>,
}

enum Cmd {
    List(Sender<std::result::Result<ChatList, String>>),
    SetMode(String),
    /// Open a conversation (None = a new one) and show its history.
    Open(Option<String>),
    Send { chat: Option<String>, prompt: String, paste_image: bool },
}

pub struct ClaudeApp(Mutex<Sender<Cmd>>);

fn send(app: &AppHandle, cmd: Cmd) {
    let _ = app.state::<ClaudeApp>().0.lock().unwrap().send(cmd);
}

pub fn list(app: &AppHandle) -> std::result::Result<ChatList, String> {
    let (tx, rx) = mpsc::channel();
    send(app, Cmd::List(tx));
    rx.recv_timeout(Duration::from_secs(8))
        .map_err(|_| "Aplikasi Claude tidak merespons".to_string())?
}

pub fn set_mode(app: &AppHandle, mode: String) {
    send(app, Cmd::SetMode(mode));
}

pub fn open(app: &AppHandle, chat: Option<String>) {
    send(app, Cmd::Open(chat));
}

pub fn send_prompt(app: &AppHandle, chat: Option<String>, prompt: String, paste_image: bool) {
    send(app, Cmd::Send { chat, prompt, paste_image });
}

/// The main Claude window: the one showing a page (chat or session).
fn main_window(ua: &IUIAutomation) -> Option<(HWND, IUIAutomationElement)> {
    claude_windows().into_iter().find_map(|hwnd| {
        let root = unsafe { ua.ElementFromHandle(hwnd) }.ok()?;
        let has_page = !of_type(ua, &root, UIA_DocumentControlTypeId.0).ok()?.is_empty();
        has_page.then_some((hwnd, root))
    })
}

fn prompt_box(ua: &IUIAutomation, root: &IUIAutomationElement) -> Option<IUIAutomationElement> {
    of_type(ua, root, UIA_EditControlTypeId.0)
        .ok()?
        .into_iter()
        .find(|e| PROMPT_LABELS.contains(&name(e).as_str()))
}

fn button(ua: &IUIAutomation, root: &IUIAutomationElement, labels: &[&str]) -> Option<IUIAutomationElement> {
    of_type(ua, root, UIA_ButtonControlTypeId.0)
        .ok()?
        .into_iter()
        .find(|b| labels.contains(&name(b).as_str()))
}

/// Which mode a radio stands for. Names carry a status while a session
/// runs, e.g. "Code, working".
fn radio_mode(label: &str) -> Option<&'static str> {
    match label.split(',').next().unwrap_or(label).trim() {
        "Claude" => Some("chat"),
        "Code" => Some("code"),
        _ => None,
    }
}

fn current_mode(ua: &IUIAutomation, root: &IUIAutomationElement) -> String {
    for r in of_type(ua, root, UIA_RadioButtonControlTypeId.0).unwrap_or_default() {
        let selected = unsafe {
            r.GetCurrentPatternAs::<IUIAutomationSelectionItemPattern>(UIA_SelectionItemPatternId)
                .and_then(|p| p.CurrentIsSelected())
                .map(|b| b.as_bool())
                .unwrap_or(false)
        };
        if let (true, Some(mode)) = (selected, radio_mode(&name(&r))) {
            return mode.into();
        }
    }
    String::new()
}

fn select_mode(ua: &IUIAutomation, root: &IUIAutomationElement, mode: &str) {
    let Some(r) = of_type(ua, root, UIA_RadioButtonControlTypeId.0)
        .unwrap_or_default()
        .into_iter()
        .find(|r| radio_mode(&name(r)) == Some(mode))
    else {
        return;
    };
    // Invoke on these radios doesn't always move the selection; Select does.
    let selected = unsafe {
        r.GetCurrentPatternAs::<IUIAutomationSelectionItemPattern>(UIA_SelectionItemPatternId)
            .and_then(|p| p.Select())
            .is_ok()
    };
    if !selected {
        press(&r);
    }
}

/// Sidebar conversations are the buttons that come with a "More options for …" button.
fn chats(ua: &IUIAutomation, root: &IUIAutomationElement) -> Vec<String> {
    let names: Vec<String> = of_type(ua, root, UIA_ButtonControlTypeId.0)
        .unwrap_or_default()
        .iter()
        .map(name)
        .collect();
    let with_menu: HashSet<&str> = names.iter().filter_map(|n| n.strip_prefix("More options for ")).collect();
    let mut seen = HashSet::new();
    names
        .iter()
        .filter(|n| with_menu.contains(n.as_str()) && seen.insert(n.as_str()))
        .cloned()
        .collect()
}

/// The open conversation's title, from the page title, e.g.
/// "Fix login bug - Claude Code" or "Trip planning - Claude".
fn open_title(ua: &IUIAutomation, root: &IUIAutomationElement) -> Option<String> {
    of_type(ua, root, UIA_DocumentControlTypeId.0).ok()?.iter().find_map(|d| {
        let title = name(d);
        title
            .strip_suffix(" - Claude Code")
            .or_else(|| title.strip_suffix(" - Claude"))
            .map(str::to_string)
    })
}

fn has_pattern(el: &IUIAutomationElement, id: UIA_PATTERN_ID) -> bool {
    unsafe { el.GetCurrentPattern(id).is_ok() }
}

/// Status lines the app shows around tool calls, not part of any answer.
fn is_noise(text: &str) -> bool {
    let t = text.trim();
    matches!(t, "running" | "Starting session" | "Working…" | "Thinking" | "Thinking…" | "just now")
        || (t.ends_with(" tokens") && t.starts_with(|c: char| c.is_ascii_digit()))
        || (t.contains(" added, ") && t.ends_with(" removed"))
}

/// Visible text of the conversation, in reading order, minus controls and
/// collapsed tool-call rows (a group holding an expand/collapse button).
fn transcript(ua: &IUIAutomation, root: &IUIAutomationElement) -> Result<Vec<String>> {
    let groups = of_type(ua, root, UIA_GroupControlTypeId.0)?;
    let Some(message) = groups.iter().rev().find(|g| name(g).starts_with("Message ")) else {
        return Ok(Vec::new());
    };
    let walker = unsafe { ua.ControlViewWalker()? };
    let container = unsafe { walker.GetParentElement(message)? };
    let mut out = Vec::new();
    collect(&walker, &container, &mut out);
    Ok(out)
}

fn children(walker: &IUIAutomationTreeWalker, el: &IUIAutomationElement) -> Vec<IUIAutomationElement> {
    let mut list = Vec::new();
    let mut next = unsafe { walker.GetFirstChildElement(el) };
    while let Ok(child) = next {
        next = unsafe { walker.GetNextSiblingElement(&child) };
        list.push(child);
    }
    list
}

fn collect(walker: &IUIAutomationTreeWalker, el: &IUIAutomationElement, out: &mut Vec<String>) {
    let skip = [
        UIA_ToolBarControlTypeId.0,
        UIA_ButtonControlTypeId.0,
        UIA_StatusBarControlTypeId.0,
        UIA_ListControlTypeId.0,
        UIA_ListItemControlTypeId.0,
    ];
    let kids = children(walker, el);
    let tool_row = kids.iter().any(|k| {
        unsafe { k.CurrentControlType() }.map(|t| t.0) == Ok(UIA_ButtonControlTypeId.0)
            && has_pattern(k, UIA_ExpandCollapsePatternId)
    });
    if tool_row {
        return;
    }
    for kid in kids {
        let Ok(kind) = (unsafe { kid.CurrentControlType() }) else { continue };
        if skip.contains(&kind.0) {
            continue;
        }
        if kind.0 == UIA_TextControlTypeId.0 {
            let text = name(&kid);
            if !text.trim().is_empty() && !is_noise(&text) {
                out.push(text);
            }
        }
        collect(walker, &kid, out);
    }
}

/// Turns the flat transcript into messages. Each user message appears as a
/// screen-reader label ("You said: …") followed by its visible copy.
fn messages(texts: &[String]) -> Vec<Message> {
    let mut out: Vec<Message> = Vec::new();
    let mut skip_copy: Option<String> = None;
    for t in texts {
        if let Some(said) = t.strip_prefix("You said:") {
            let said = said.trim().to_string();
            skip_copy = Some(said.clone());
            out.push(Message { role: "user", text: said });
            continue;
        }
        if let Some(copy) = skip_copy.take() {
            // The visible copy may be cut short; it starts like the label.
            let head: String = t.chars().take(30).collect();
            if copy.starts_with(&head) {
                continue;
            }
        }
        match out.last_mut() {
            Some(last) if last.role == "claude" => {
                last.text.push_str("\n\n");
                last.text.push_str(t);
            }
            // Anything before the first user message (e.g. a greeting) is Claude's.
            _ => out.push(Message { role: "claude", text: t.clone() }),
        }
    }
    let skip = out.len().saturating_sub(HISTORY_LEN);
    out.into_iter().skip(skip).collect()
}

fn same_start(a: &str, b: &str) -> bool {
    let words = |s: &str| s.split_whitespace().take(6).collect::<Vec<_>>().join(" ");
    let (a, b) = (words(a), words(b));
    a.starts_with(&b) || b.starts_with(&a)
}

struct Watch {
    /// The prompt just sent, until it shows up in the conversation.
    awaiting: Option<String>,
    started: Instant,
    last: Vec<Message>,
    stable: u8,
}

impl Watch {
    fn new(awaiting: Option<String>) -> Self {
        Watch { awaiting, started: Instant::now(), last: Vec::new(), stable: 0 }
    }
}

/// Press the sidebar entry for `chat`, or New, and wait for its prompt box.
fn open_chat(ua: &IUIAutomation, chat: Option<&str>) -> std::result::Result<IUIAutomationElement, String> {
    let (_, root) = main_window(ua).ok_or("Aplikasi Claude tidak terbuka")?;
    let already_open = chat.is_some() && open_title(ua, &root).as_deref() == chat;
    if !already_open {
        let target = match chat {
            Some(c) => button(ua, &root, &[c]).ok_or_else(|| format!("Chat \"{c}\" tidak ditemukan di sidebar"))?,
            None => button(ua, &root, &["New"]).ok_or("Tombol New tidak ditemukan")?,
        };
        press(&target);
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        thread::sleep(Duration::from_millis(if already_open { 0 } else { 400 }));
        if let Some(input) = main_window(ua).and_then(|(_, r)| prompt_box(ua, &r)) {
            return Ok(input);
        }
        if Instant::now() > deadline {
            return Err("Kotak prompt Claude tidak ditemukan".into());
        }
    }
}

/// Puts whatever image is on the clipboard into Claude's prompt box with a
/// real Ctrl+V. Needs the app in front, so this one briefly takes focus.
fn paste_clipboard(hwnd: HWND, input: &IUIAutomationElement) {
    unsafe {
        let _ = ShowWindow(hwnd, SW_RESTORE);
        let _ = SetForegroundWindow(hwnd);
        thread::sleep(Duration::from_millis(250));
        let _ = input.SetFocus();
        thread::sleep(Duration::from_millis(150));
        let v = b'V';
        keybd_event(VK_CONTROL.0 as u8, 0, Default::default(), 0);
        keybd_event(v, 0, Default::default(), 0);
        keybd_event(v, 0, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_CONTROL.0 as u8, 0, KEYEVENTF_KEYUP, 0);
    }
    // Let the attachment upload before sending.
    thread::sleep(Duration::from_millis(1800));
}

fn deliver(ua: &IUIAutomation, chat: Option<&str>, prompt: &str, paste_image: bool) -> std::result::Result<(), String> {
    let input = open_chat(ua, chat)?;
    if paste_image {
        let (hwnd, _) = main_window(ua).ok_or("Aplikasi Claude tertutup")?;
        paste_clipboard(hwnd, &input);
    }
    unsafe {
        let value = input
            .GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)
            .map_err(|_| "Kotak prompt tidak bisa diisi")?;
        value.SetValue(&BSTR::from(prompt)).map_err(|e| e.message().to_string())?;
    }
    thread::sleep(Duration::from_millis(400));
    let (_, root) = main_window(ua).ok_or("Aplikasi Claude tertutup")?;
    let send = button(ua, &root, &SEND_LABELS).ok_or("Tombol Send tidak ditemukan")?;
    press(&send);
    Ok(())
}

fn run(app: AppHandle, rx: Receiver<Cmd>) {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
    let Ok(ua) = (unsafe { CoCreateInstance::<_, IUIAutomation>(&CUIAutomation, None, CLSCTX_INPROC_SERVER) })
    else {
        return;
    };
    let emit = |messages: Vec<Message>, working: bool, error: Option<String>| {
        let _ = app.emit("claude-history", History { messages, working, error });
    };
    let mut watch: Option<Watch> = None;
    loop {
        match rx.recv_timeout(POLL) {
            Ok(Cmd::List(reply)) => {
                let result = main_window(&ua)
                    .map(|(_, root)| {
                        let chats = chats(&ua, &root);
                        let current = open_title(&ua, &root).filter(|t| chats.contains(t));
                        ChatList { mode: current_mode(&ua, &root), chats, current }
                    })
                    .ok_or_else(|| "Aplikasi Claude tidak terbuka".to_string());
                let _ = reply.send(result);
            }
            Ok(Cmd::SetMode(mode)) => {
                if let Some((_, root)) = main_window(&ua) {
                    select_mode(&ua, &root, &mode);
                }
            }
            Ok(Cmd::Open(chat)) => match open_chat(&ua, chat.as_deref()) {
                Ok(_) => watch = Some(Watch::new(None)),
                Err(e) => emit(Vec::new(), false, Some(e)),
            },
            Ok(Cmd::Send { chat, prompt, paste_image }) => {
                match deliver(&ua, chat.as_deref(), &prompt, paste_image) {
                    Ok(()) => watch = Some(Watch::new(Some(prompt))),
                    Err(e) => {
                        watch = None;
                        emit(Vec::new(), false, Some(e));
                    }
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }

        let Some(w) = watch.as_mut() else { continue };
        let Some((_, root)) = main_window(&ua) else {
            emit(w.last.clone(), false, Some("Aplikasi Claude tertutup".into()));
            watch = None;
            continue;
        };
        let history = transcript(&ua, &root).map(|t| messages(&t)).unwrap_or_default();
        let working = button(&ua, &root, &STOP_LABELS).is_some();
        if let Some(prompt) = &w.awaiting {
            // Keep showing "sending" until our prompt is in the conversation.
            let landed = history.iter().rev().find(|m| m.role == "user").is_some_and(|m| same_start(&m.text, prompt));
            if !landed {
                if w.started.elapsed() > Duration::from_secs(20) {
                    emit(history, false, Some("Prompt tidak terlihat terkirim di aplikasi Claude".into()));
                    watch = None;
                }
                continue;
            }
            w.awaiting = None;
        }
        w.stable = if history == w.last { w.stable.saturating_add(1) } else { 0 };
        // Done once Claude stops working and the conversation has settled.
        let done = !working && w.stable >= 2;
        if history != w.last || done {
            w.last = history.clone();
            emit(history, !done, None);
        }
        if done || w.started.elapsed() > WATCH_TIMEOUT {
            watch = None;
        }
    }
}

pub fn spawn(app: &AppHandle) {
    let (tx, rx) = mpsc::channel();
    app.manage(ClaudeApp(Mutex::new(tx)));
    let app = app.clone();
    thread::spawn(move || run(app, rx));
}
