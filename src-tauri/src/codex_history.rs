use serde::Serialize;
use serde_json::Value;

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct History {
    pub thread_id: String,
    pub messages: Vec<Message>,
    pub working: bool,
    pub error: Option<String>,
}
#[derive(Serialize, Clone)]
pub struct Message {
    pub id: String,
    pub role: String,
    pub text: String,
}

fn message(item: &Value) -> Option<Message> {
    let (role, text) = match item["type"].as_str()? {
        "userMessage" => (
            "user",
            item["content"]
                .as_array()?
                .iter()
                .filter_map(|c| c["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        "agentMessage" => ("assistant", item["text"].as_str()?.to_owned()),
        _ => return None,
    };
    Some(Message {
        id: item["id"].as_str()?.into(),
        role: role.into(),
        text,
    })
}
impl History {
    pub fn upsert_item(&mut self, item: &Value) {
        if let Some(m) = message(item) {
            if let Some(old) = self.messages.iter_mut().find(|old| old.id == m.id) {
                *old = m;
            } else {
                self.messages.push(m);
            }
        }
        self.trim();
    }
    pub fn delta(&mut self, id: &str, text: &str) {
        if let Some(m) = self.messages.iter_mut().find(|m| m.id == id) {
            m.text.push_str(text);
        } else {
            self.messages.push(Message {
                id: id.into(),
                role: "assistant".into(),
                text: text.into(),
            });
        }
        self.trim();
    }
    fn trim(&mut self) {
        if self.messages.len() > 40 {
            self.messages.drain(..self.messages.len() - 40);
        }
    }
}
pub fn parse_messages(thread: &Value) -> Vec<Message> {
    let mut h = History::default();
    for turn in thread["turns"].as_array().into_iter().flatten() {
        for item in turn["items"].as_array().into_iter().flatten() {
            h.upsert_item(item);
        }
    }
    h.messages
}

pub fn approval_summary(params: &Value, item: Option<&Value>) -> String {
    let mut parts = Vec::new();
    if let Some(reason) = params["reason"].as_str() {
        parts.push(reason.to_owned());
    }
    if let Some(command) = params["command"].as_str() {
        parts.push(command.to_owned());
    } else if params["command"].is_array() {
        parts.push(serde_json::to_string_pretty(&params["command"]).unwrap_or_default());
    }
    if let Some(cwd) = params["cwd"].as_str() {
        parts.push(format!("Folder: {cwd}"));
    }
    for (key, label) in [
        ("permissions", "Akses yang diminta"),
        ("fileChanges", "Perubahan file"),
        ("networkApprovalContext", "Akses jaringan"),
        ("grantRoot", "Folder yang diminta"),
    ] {
        if !params[key].is_null() {
            parts.push(format!(
                "{label}:\n{}",
                serde_json::to_string_pretty(&params[key]).unwrap_or_default()
            ));
        }
    }
    if let Some(changes) = item.and_then(|v| v.get("changes")) {
        parts.push(format!(
            "Perubahan:\n{}",
            serde_json::to_string_pretty(changes).unwrap_or_default()
        ));
    }
    if parts.is_empty() {
        "Codex meminta persetujuan; detail tindakan belum tersedia.".into()
    } else {
        parts.join("\n\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn history_excludes_tools_and_reasoning() {
        let messages = parse_messages(&json!({"turns":[{"items":[
            {"id":"u","type":"userMessage","content":[{"type":"text","text":"Hello"},{"type":"image","url":"ignored"}]},
            {"id":"tool","type":"commandExecution","text":"ignored"},
            {"id":"r","type":"reasoning","text":"ignored"},
            {"id":"a","type":"agentMessage","text":"Hi"}
        ]}]}));
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].text, "Hello");
        assert_eq!(messages[1].role, "assistant");
    }
    #[test]
    fn completion_replaces_streamed_item_without_duplicate() {
        let mut h = History::default();
        h.delta("a", "Hel");
        h.delta("a", "lo");
        h.upsert_item(&json!({"id":"a","type":"agentMessage","text":"Hello!"}));
        assert_eq!(h.messages.len(), 1);
        assert_eq!(h.messages[0].text, "Hello!");
    }
    #[test]
    fn visible_history_is_bounded_and_keeps_order() {
        let mut h = History::default();
        for i in 0..45 {
            h.upsert_item(&json!({"id":i.to_string(),"type":"agentMessage","text":i.to_string()}));
        }
        assert_eq!(h.messages.len(), 40);
        assert_eq!(h.messages[0].id, "5");
        assert_eq!(h.messages[39].id, "44");
    }
    #[test]
    fn approval_shows_action_without_internal_ids_or_persistent_choices() {
        let summary = approval_summary(
            &json!({"threadId":"internal","availableDecisions":["acceptForSession"],"command":"Write-Output test","cwd":"D:/example","reason":"Print test output"}),
            None,
        );
        assert!(summary.contains("Write-Output test"));
        assert!(summary.contains("D:/example"));
        assert!(!summary.contains("internal"));
        assert!(!summary.contains("acceptForSession"));
    }
}
