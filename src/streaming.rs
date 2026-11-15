//! Bounded SSE decoding. Only public answer content is consumed, never reasoning fields.
use anyhow::{Context, Result, ensure};
use serde_json::Value;
use std::io::{BufRead, BufReader, Read};
const MAX_WIRE: u64 = 1024 * 1024;
const MAX_CONTENT: usize = 128 * 1024;

pub fn read(reader: impl Read, is_sse: bool, mut changed: impl FnMut(&str)) -> Result<String> {
    let mut reader = BufReader::new(reader.take(MAX_WIRE + 1));
    let first = reader.fill_buf().context("AI response could not be read")?;
    if !is_sse && first.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'{') {
        let mut bytes = Vec::new();
        reader
            .read_to_end(&mut bytes)
            .context("AI response ended unexpectedly")?;
        ensure!(bytes.len() as u64 <= MAX_WIRE, "AI response is too large");
        let value: Value = serde_json::from_slice(&bytes).context("Invalid AI response")?;
        ensure!(
            value
                .pointer("/choices/0/finish_reason")
                .and_then(Value::as_str)
                != Some("length"),
            "AI response was cut short"
        );
        let content = value
            .pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .context("AI response has no message content")?;
        ensure!(content.len() <= MAX_CONTENT, "AI response is too large");
        changed(content);
        return Ok(content.to_owned());
    }
    let mut content = String::new();
    let mut data = String::new();
    let mut total = 0;
    let mut complete = false;
    loop {
        let mut line = String::new();
        let count = reader
            .read_line(&mut line)
            .context("AI stream interrupted. Try again; the current preview is preserved.")?;
        if count == 0 {
            break;
        }
        total += count;
        ensure!(total as u64 <= MAX_WIRE, "AI stream is too large");
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            if data.is_empty() {
                continue;
            }
            if data.trim() == "[DONE]" {
                complete = true;
                break;
            }
            let value: Value =
                serde_json::from_str(data.trim()).context("Invalid AI stream event")?;
            ensure!(
                value.get("error").is_none(),
                "AI provider reported a streaming error"
            );
            if let Some(reason) = value
                .pointer("/choices/0/finish_reason")
                .and_then(Value::as_str)
            {
                ensure!(
                    reason != "length" && reason != "content_filter",
                    "AI response was incomplete"
                );
                complete = reason == "stop";
            }
            if let Some(delta) = value
                .pointer("/choices/0/delta/content")
                .and_then(Value::as_str)
            {
                ensure!(
                    content.len() + delta.len() <= MAX_CONTENT,
                    "AI response is too large"
                );
                content.push_str(delta);
                changed(&content);
            }
            data.clear();
        } else if let Some(value) = line.strip_prefix("data:") {
            data.push_str(value.strip_prefix(' ').unwrap_or(value));
            data.push('\n');
        }
    }
    ensure!(
        complete && !content.is_empty(),
        "AI stream ended before the answer was complete. Try again; the current preview is preserved."
    );
    Ok(content)
}

/// Decode just the public summary while its surrounding JSON is still arriving.
pub fn summary_prefix(content: &str) -> Option<String> {
    let rest = content
        .trim_start()
        .strip_prefix('{')?
        .trim_start()
        .strip_prefix("\"summary\"")?
        .trim_start()
        .strip_prefix(':')?
        .trim_start();
    if !rest.starts_with('"') {
        return None;
    }
    let mut escaped = false;
    for (index, ch) in rest.char_indices().skip(1) {
        if ch == '"' && !escaped {
            return serde_json::from_str::<String>(&rest[..=index]).ok();
        }
        if ch == '\\' {
            escaped = !escaped;
        } else {
            escaped = false;
        }
    }
    // A chunk may end inside a JSON escape or UTF-16 surrogate pair. Retain the
    // complete prefix until the next chunk arrives, without displaying broken text.
    let mut end = rest.len();
    for _ in 0..14 {
        if let Ok(text) = serde_json::from_str::<String>(&format!("{}\"", &rest[..end])) {
            return Some(text);
        }
        end = rest[..end].char_indices().last()?.0;
    }
    None
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn streams_public_answer_incrementally_and_ignores_private_reasoning() {
        let pieces = ["{\"summary\":\"Игры ", "и 3D отдельно.\",\"groups\":[]}"];
        let mut wire = String::from(": heartbeat\n\n");
        wire.push_str(
            "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"private-marker\"}}]}\n\n",
        );
        for piece in pieces {
            wire.push_str(&format!(
                "data: {}\n\n",
                serde_json::json!({"choices":[{"delta":{"content":piece}}]})
            ));
        }
        wire.push_str("data: [DONE]\n\n");
        let mut seen = vec![];
        let content = read(wire.as_bytes(), true, |text| {
            seen.push(summary_prefix(text))
        })
        .unwrap();
        assert_eq!(seen[0].as_deref(), Some("Игры "));
        assert_eq!(seen[1].as_deref(), Some("Игры и 3D отдельно."));
        assert!(!content.contains("private-marker"));
    }
    #[test]
    fn handles_partial_escapes_and_rejects_truncated_or_oversized_streams() {
        assert_eq!(
            summary_prefix("{\"summary\":\"Hello\\nworld\\u04"),
            Some("Hello\nworld".into())
        );
        assert_eq!(summary_prefix("{\"groups\":[]}"), None);
        assert!(
            read(
                b"data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n".as_slice(),
                true,
                |_| {}
            )
            .is_err()
        );
        let wire = format!(
            "data: {}\n\n",
            serde_json::json!({"choices":[{"delta":{"content":"a".repeat(MAX_CONTENT+1)}}]})
        );
        assert!(read(wire.as_bytes(), true, |_| {}).is_err());
    }
}
