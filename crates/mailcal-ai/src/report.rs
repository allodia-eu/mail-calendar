//! What the log says about one exchange with an AI endpoint (`docs/logging.md`): which request, how
//! much was sent, how long it took, and for a refusal the server's own error fields. Never the
//! endpoint, the key, the request or the answer.

use std::time::Duration;

use serde_json::Value;

use crate::{
    AiError,
    wire::{ChatResponse, Purpose},
};

/// The longest a server's error message is logged, in characters.
const MESSAGE_CHARS: usize = 200;

/// The longest a `type`, `code` or `param` value is logged, in characters.
const FIELD_CHARS: usize = 80;

/// Logs an exchange that ended with an HTTP answer.
pub fn log_answer(purpose: Purpose, sent: usize, elapsed: Duration, status: u16, body: &str) {
    let what = purpose.label();
    let ms = elapsed.as_millis();
    if (200..300).contains(&status) {
        log::info!(
            "ai: {what} answered in {ms} ms ({sent} bytes sent, {} received)",
            body.len()
        );
    } else {
        log::warn!(
            "ai: {what} refused with {status} after {ms} ms ({sent} bytes sent): {}",
            refusal_summary(body)
        );
    }
}

/// Logs an exchange that got no answer at all.
pub fn log_unanswered(purpose: Purpose, sent: usize, elapsed: Duration) {
    log::warn!(
        "ai: {} got no answer after {} ms ({sent} bytes sent)",
        purpose.label(),
        elapsed.as_millis()
    );
}

/// Reads a successful answer, and says in the log when it does not read as one.
///
/// # Errors
///
/// Returns [`AiError::Malformed`] when the body is not a chat-completions answer.
pub fn read_answer(body: &str) -> Result<ChatResponse, AiError> {
    serde_json::from_str(body).map_err(|_| {
        log::warn!(
            "ai: an answer of {} bytes is not a chat-completions answer",
            body.len()
        );
        AiError::Malformed
    })
}

/// A refusal's error fields as one line: `type`, `code` and `param` when each is a short word, and
/// the message on one line, cut to 200 characters. An answer that is not JSON is described by
/// its size alone, because an error page can name the host.
#[must_use]
pub fn refusal_summary(body: &str) -> String {
    let Ok(root) = serde_json::from_str::<Value>(body) else {
        return format!("a non-JSON answer of {} bytes", body.len());
    };
    let error = match root.get("error") {
        Some(Value::Object(_)) => &root["error"],
        _ => &root,
    };
    let code = root
        .get("data")
        .and_then(|data| data.get("code"))
        .or_else(|| error.get("code"));
    let mut parts = Vec::new();
    for (name, value) in [
        ("type", error.get("type")),
        ("code", code),
        ("param", error.get("param")),
    ] {
        if let Some(word) = value.and_then(short_word) {
            parts.push(format!("{name}={word}"));
        }
    }
    let message = error
        .get("message")
        .or_else(|| root.get("error").filter(|value| value.is_string()))
        .or_else(|| root.get("detail"))
        .and_then(Value::as_str);
    if let Some(message) = message {
        parts.push(format!("message=\"{}\"", one_line(message)));
    }
    if parts.is_empty() {
        format!("no error fields in {} bytes", body.len())
    } else {
        parts.join(" ")
    }
}

fn short_word(value: &Value) -> Option<String> {
    let text = match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        _ => return None,
    };
    let fits = !text.is_empty()
        && text.chars().count() <= FIELD_CHARS
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_.-:[]".contains(c));
    fits.then_some(text)
}

fn one_line(message: &str) -> String {
    let flat = message.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= MESSAGE_CHARS {
        return flat;
    }
    let mut cut: String = flat.chars().take(MESSAGE_CHARS).collect();
    cut.push('…');
    cut
}

#[cfg(test)]
#[path = "report_tests.rs"]
mod tests;
