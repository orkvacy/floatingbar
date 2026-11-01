//! Merge our hook without touching unrelated user settings.
use serde_json::{json, Value};

pub fn sanitize_response(value: Value) -> Value {
    if value["hookSpecificOutput"]["hookEventName"] != "PermissionRequest" {
        return json!({});
    }
    let decision = match value["hookSpecificOutput"]["decision"]["behavior"].as_str() {
        Some("allow") => json!({"behavior":"allow"}),
        Some("deny") => json!({"behavior":"deny","message":"Ditolak lewat FloatingBar"}),
        _ => return json!({}),
    };
    json!({"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":decision}})
}

pub fn is_ours(hook: &Value) -> bool {
    matches!(
        hook["url"].as_str(),
        Some("http://127.0.0.1:47821/permission" | "http://localhost:47821/permission")
    ) || (hook["command"].as_str().is_some_and(|s| {
        s.replace('\\', "/")
            .to_ascii_lowercase()
            .ends_with("/floatingbar.exe")
    }) && hook["args"]
        .as_array()
        .is_some_and(|args| args.iter().any(|v| v == "--permission-hook")))
}
pub fn merge(
    mut settings: Value,
    exe: &str,
    args: Vec<String>,
    install: bool,
) -> Result<(Value, usize), String> {
    if !settings.is_object() {
        return Err("Konfigurasi Claude harus berupa objek JSON".into());
    }
    if settings.get("hooks").is_none() {
        if !install {
            return Ok((settings, 0));
        }
        settings["hooks"] = json!({});
    }
    if !settings["hooks"].is_object() {
        return Err("Bagian hooks Claude tidak valid".into());
    }
    if settings["hooks"].get("PermissionRequest").is_none() {
        if !install {
            return Ok((settings, 0));
        }
        settings["hooks"]["PermissionRequest"] = json!([]);
    }
    let groups = settings["hooks"]["PermissionRequest"]
        .as_array_mut()
        .ok_or("PermissionRequest harus berupa array")?;
    let replacement = json!({"type":"command","command":exe,"args":args,"timeout":60,"statusMessage":"Menunggu jawaban di FloatingBar…"});
    let mut count = 0;
    for group in groups.iter_mut() {
        let hooks = group["hooks"]
            .as_array_mut()
            .ok_or("Daftar hook Claude tidak valid")?;
        for hook in hooks {
            if is_ours(hook) {
                *hook = replacement.clone();
                count += 1;
            }
        }
    }
    if count == 0 && install {
        groups.push(json!({"hooks":[replacement]}));
        count = 1;
    }
    Ok((settings, count))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bridge_outputs_only_explicit_one_time_decisions() {
        let source = json!({"systemMessage":"untrusted","hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow","updatedPermissions":["all"]}}});
        let result = sanitize_response(source);
        assert_eq!(
            result["hookSpecificOutput"]["decision"],
            json!({"behavior":"allow"})
        );
        assert!(result.get("systemMessage").is_none());
        assert_eq!(sanitize_response(json!({})), json!({}));
        assert_eq!(
            sanitize_response(
                json!({"hookSpecificOutput":{"hookEventName":"Other","decision":{"behavior":"allow"}}})
            ),
            json!({})
        );
    }
    #[test]
    fn migrates_only_our_endpoint_and_preserves_other_settings() {
        let original = json!({"env":{"TEST":"unchanged"},"hooks":{"Stop":[{"hooks":[{"type":"command","command":"other"}]}],"PermissionRequest":[{"matcher":"Bash","hooks":[{"type":"http","url":"http://127.0.0.1:47821/permission"},{"type":"http","url":"https://example.invalid/permission"}]}]}});
        let (new, count) = merge(
            original.clone(),
            "C:/app/floatingbar.exe",
            vec!["--permission-hook".into()],
            false,
        )
        .unwrap();
        assert_eq!(count, 1);
        assert_eq!(new["env"], original["env"]);
        assert_eq!(new["hooks"]["Stop"], original["hooks"]["Stop"]);
        assert_eq!(new["hooks"]["PermissionRequest"][0]["matcher"], "Bash");
        assert_eq!(
            new["hooks"]["PermissionRequest"][0]["hooks"][1],
            original["hooks"]["PermissionRequest"][0]["hooks"][1]
        );
        let (again, count) = merge(
            new.clone(),
            "C:/app/floatingbar.exe",
            vec!["--permission-hook".into()],
            true,
        )
        .unwrap();
        assert_eq!(count, 1);
        assert_eq!(again, new);
    }
    #[test]
    fn does_not_install_automatically_or_replace_invalid_config() {
        assert_eq!(
            merge(json!({"theme":"dark"}), "app", vec![], false)
                .unwrap()
                .1,
            0
        );
        for value in [
            json!([]),
            json!({"hooks":[]}),
            json!({"hooks":{"PermissionRequest":"bad"}}),
        ] {
            assert!(merge(value, "app", vec![], true).is_err());
        }
    }
}
