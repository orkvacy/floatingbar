//! Sends a prompt from the pill to the Claude desktop app. The app registers
//! the `claude://` protocol; a `new?q=` link opens a new chat with the prompt
//! typed in. The answer appears in the app, which has no way to hand it back.

/// Opens a new chat in the Claude app with the prompt already typed in.
pub fn open_in_app(prompt: &str) -> Result<(), String> {
    let encoded: String = prompt
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect();
    crate::shortcuts::launch(&format!("claude://claude.ai/new?q={encoded}"))
}
