//! Conservative desktop adapters. A process name alone is never a prompt.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Provider {
    Claude,
    Codex,
    ChatGpt,
    Antigravity,
}

impl Provider {
    pub fn from_identity(exe: &str, app_id: Option<&str>) -> Option<Self> {
        // The packaged Codex desktop app can still use ChatGPT.exe.
        let candidate = Self::from_exe(exe)?;
        if matches!(candidate, Self::ChatGpt | Self::Codex)
            && app_id.is_some_and(|id| id.to_ascii_lowercase().starts_with("openai.codex_"))
        {
            return Some(Self::Codex);
        }
        Some(candidate)
    }
    pub fn from_exe(exe: &str) -> Option<Self> {
        match exe.to_ascii_lowercase().as_str() {
            "claude.exe" => Some(Self::Claude),
            "codex.exe" => Some(Self::Codex),
            "chatgpt.exe" => Some(Self::ChatGpt),
            "antigravity.exe" => Some(Self::Antigravity),
            _ => None,
        }
    }
    pub fn id(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::ChatGpt => "chatgpt",
            Self::Antigravity => "antigravity",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Claude => "Claude",
            Self::Codex | Self::ChatGpt => "ChatGPT / Codex",
            Self::Antigravity => "Antigravity",
        }
    }
    // Only Claude's existing UI adapter has been verified for direct answers.
    pub fn can_answer(self) -> bool {
        self == Self::Claude
    }
    pub fn allow_label(self, label: &str) -> bool {
        let label = label.trim().to_ascii_lowercase();
        match self {
            Self::Claude => label == "allow once" || label.starts_with("allow once ctrl"),
            _ => matches!(
                label.as_str(),
                "allow once"
                    | "allow"
                    | "approve"
                    | "approve once"
                    | "accept"
                    | "run"
                    | "run command"
                    | "yes, proceed"
                    | "izinkan"
                    | "izinkan sekali"
                    | "setujui"
            ),
        }
    }
    pub fn deny_label(self, label: &str) -> bool {
        let label = label.trim().to_ascii_lowercase();
        match self {
            Self::Claude => label == "decline" || label.starts_with("decline ctrl"),
            _ => {
                matches!(
                    label.as_str(),
                    "deny" | "decline" | "reject" | "cancel" | "tolak" | "batalkan"
                ) || label.starts_with("no, and tell codex")
            }
        }
    }
    pub fn prompt_context(self, text: &str) -> bool {
        let text = text.to_lowercase();
        let common = [
            "permission required",
            "approval required",
            "requires approval",
            "needs your approval",
            "requesting permission",
            "request for approval",
            "allow this",
            "izin diperlukan",
            "meminta izin",
            "minta izin",
            "butuh persetujuan",
        ];
        common.iter().any(|s| text.contains(s))
            || match self {
                Self::Claude => text.contains("wants to"),
                Self::Codex => {
                    text.contains("would you like to run")
                        || text.contains("do you want to run")
                        || text.contains("run this command")
                }
                Self::ChatGpt => text.contains("confirm you want") || text.contains("allow access"),
                Self::Antigravity => {
                    text.contains("run command?")
                        || text.contains("run this command")
                        || text.contains("review required")
                }
            }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn packaged_codex_is_not_misidentified_as_chatgpt() {
        assert_eq!(
            Provider::from_identity("ChatGPT.exe", Some("OpenAI.Codex_example!App")),
            Some(Provider::Codex)
        );
        assert_eq!(
            Provider::from_identity("ChatGPT.exe", None),
            Some(Provider::ChatGpt)
        );
        assert_eq!(
            Provider::from_identity("chrome.exe", Some("OpenAI.Codex_example!App")),
            None
        );
    }
    #[test]
    fn only_known_desktop_processes_are_sources() {
        assert_eq!(Provider::from_exe("Codex.exe"), Some(Provider::Codex));
        for exe in [
            "chrome.exe",
            "Code.exe",
            "my-claude.exe",
            "codex-helper.exe",
        ] {
            assert_eq!(Provider::from_exe(exe), None);
        }
    }
    #[test]
    fn persistent_or_generic_actions_are_not_one_time_approvals() {
        for p in [
            Provider::Claude,
            Provider::Codex,
            Provider::ChatGpt,
            Provider::Antigravity,
        ] {
            for label in [
                "Always allow",
                "Allow for this session",
                "Allow all",
                "Yes",
                "Continue",
                "Run all",
            ] {
                assert!(!p.allow_label(label), "{p:?}: {label}");
            }
        }
    }
    #[test]
    fn normal_conversation_is_not_a_permission() {
        for p in [
            Provider::Claude,
            Provider::Codex,
            Provider::ChatGpt,
            Provider::Antigravity,
        ] {
            assert!(!p.prompt_context("How do I configure my project?"));
            assert!(p.prompt_context("Approval required to read this file"));
        }
    }
    #[test]
    fn unverified_adapters_never_answer_directly() {
        assert!(Provider::Claude.can_answer());
        for p in [Provider::Codex, Provider::ChatGpt, Provider::Antigravity] {
            assert!(!p.can_answer());
        }
    }
}
