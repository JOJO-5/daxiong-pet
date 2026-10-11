use serde_json::{json, Value};
pub const MAX_BODY: usize = 64 * 1024;

pub struct Chat {
    pub system: String,
    pub history: String,
    pub message: String,
    pub output: i32,
    pub temperature: f32,
}

pub fn chat(value: Value) -> Result<Chat, String> {
    if value.get("model").and_then(Value::as_str) != Some("daxiong-litert") {
        return Err("Unknown model; use daxiong-litert".into());
    }
    if value.get("stream").and_then(Value::as_bool) != Some(true) {
        return Err("This research worker requires stream=true".into());
    }
    if value.get("tools").is_some() {
        return Err("Tool execution is not supported".into());
    }
    let output = match value
        .get("max_tokens")
        .or_else(|| value.get("max_completion_tokens"))
    {
        None => 32,
        Some(v) => v.as_i64().ok_or("Output token limit must be an integer")?,
    };
    if !(1..=256).contains(&output) {
        return Err("Output must be 1..256 tokens".into());
    }
    let temperature = match value.get("temperature") {
        None => 0.6,
        Some(v) => v.as_f64().ok_or("temperature must be numeric")?,
    };
    if !(0.0..=2.0).contains(&temperature) {
        return Err("temperature must be 0..2".into());
    }
    let messages = value
        .get("messages")
        .and_then(Value::as_array)
        .ok_or("messages must be an array")?;
    if messages.is_empty() || messages.len() > 64 {
        return Err("Expected 1..64 text messages".into());
    }
    let mut system = Vec::new();
    let mut history = Vec::new();
    for message in messages {
        let role = message
            .get("role")
            .and_then(Value::as_str)
            .ok_or("Missing role")?;
        if !matches!(role, "system" | "user" | "assistant") {
            return Err("Only system/user/assistant roles are supported".into());
        }
        let content = match message.get("content") {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Array(parts)) => {
                let mut s = String::new();
                for part in parts {
                    if part.get("type").and_then(Value::as_str) != Some("text") {
                        return Err("This worker supports text only".into());
                    }
                    s.push_str(
                        part.get("text")
                            .and_then(Value::as_str)
                            .ok_or("Missing text")?,
                    );
                }
                s
            }
            _ => return Err("Text content is required".into()),
        };
        if content.contains('\0') {
            return Err("Embedded NUL is not accepted".into());
        }
        if role == "system" {
            if !history.is_empty() {
                return Err("System messages must precede dialogue".into());
            }
            system.push(content);
        } else {
            history.push(json!({"role":role,"content":[{"type":"text","text":content}]}));
        }
    }
    if history
        .last()
        .and_then(|v| v.get("role"))
        .and_then(Value::as_str)
        != Some("user")
    {
        return Err("Last message must be user".into());
    }
    let message = history.pop().unwrap();
    Ok(Chat {
        system: system.join("\n"),
        history: Value::Array(history).to_string(),
        message: message.to_string(),
        output: output as i32,
        temperature: temperature as f32,
    })
}

pub fn delta(raw: &str) -> Result<String, String> {
    if raw.is_empty() {
        return Ok(String::new());
    }
    let value: Value = serde_json::from_str(raw).map_err(|_| "Malformed LiteRT stream JSON")?;
    if let Some(s) = value.as_str() {
        return Ok(s.into());
    }
    if value.get("channels").is_some() || value.get("tool_calls").is_some() {
        return Err("Unexpected thinking/tool stream".into());
    }
    match value.get("content") {
        Some(Value::String(s)) => Ok(s.clone()),
        Some(Value::Array(parts)) => {
            let mut text = String::new();
            for part in parts {
                if part.get("type").and_then(Value::as_str) != Some("text") {
                    return Err("Unexpected non-text stream".into());
                }
                text.push_str(
                    part.get("text")
                        .and_then(Value::as_str)
                        .ok_or("Missing delta text")?,
                );
            }
            Ok(text)
        }
        None => Ok(String::new()),
        _ => Err("Unexpected delta content".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_is_rebuilt_and_last_user_is_not_prefilled_twice() {
        let c=chat(json!({"model":"daxiong-litert","stream":true,"messages":[{"role":"system","content":"dog"},{"role":"user","content":"name"},{"role":"assistant","content":"ok"},{"role":"user","content":"recall"}]})).unwrap();
        assert_eq!(c.system, "dog");
        let history: Value = serde_json::from_str(&c.history).unwrap();
        assert_eq!(history.as_array().unwrap().len(), 2);
        assert!(!c.history.contains("recall"));
        assert!(c.message.contains("recall"));
    }
    #[test]
    fn media_tools_and_ambiguous_history_are_rejected() {
        let base = json!({"model":"daxiong-litert","stream":true,"messages":[{"role":"user","content":"hello"}]});
        for bad in [
            json!({"role":"user","content":[{"type":"image_url","image_url":{"url":"data:image/png;base64,x"}}]}),
            json!({"role":"tool","content":"x"}),
            json!({"role":"assistant","content":"x"}),
            json!({"role":"user","content":"x\u{0}"}),
        ] {
            let mut v = base.clone();
            v["messages"] = json!([bad]);
            assert!(chat(v).is_err());
        }
        let mut v = base;
        v["max_tokens"] = json!(1000000);
        assert!(chat(v).is_err());
    }
    #[test]
    fn reasoning_and_tools_are_never_rendered_as_pet_text() {
        assert_eq!(
            delta(r#"{"role":"assistant","content":[{"type":"text","text":"你好"}]}"#).unwrap(),
            "你好"
        );
        assert!(delta(r#"{"channels":{"thought":"private"},"content":[]}"#).is_err());
        assert!(delta(r#"{"tool_calls":[{}]}"#).is_err());
    }
}
