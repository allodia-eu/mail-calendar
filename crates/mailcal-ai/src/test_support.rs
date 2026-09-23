//! Doubles shared by this crate's tests: a backend that records what reached it and answers from
//! a script, and a transport that does the same one layer down.

use std::sync::{Arc, Mutex};

use mailcal_jurisdiction::{Destination, Mode};

use crate::{
    AiBackend, AiError, GatedBackend,
    transport::{HttpRequest, HttpResponse, HttpTransport, TransportFailed},
    wire::{ChatRequest, ChatResponse},
};

/// Records every request that reached it and answers each from `answers` in turn, repeating the
/// last one when the script runs out.
#[derive(Default)]
pub(crate) struct RecordingBackend {
    pub(crate) seen: Arc<Mutex<Vec<ChatRequest>>>,
    pub(crate) answers: Mutex<Vec<Result<ChatResponse, AiError>>>,
}

impl RecordingBackend {
    pub(crate) fn answering(answers: Vec<Result<ChatResponse, AiError>>) -> Self {
        let mut answers = answers;
        answers.reverse();
        Self {
            seen: Arc::default(),
            answers: Mutex::new(answers),
        }
    }
}

impl AiBackend for RecordingBackend {
    fn chat(&self, request: &ChatRequest) -> Result<ChatResponse, AiError> {
        self.seen.lock().unwrap().push(request.clone());
        let mut answers = self.answers.lock().unwrap();
        if answers.len() > 1 {
            answers.pop().unwrap()
        } else {
            answers.last().cloned().unwrap_or(Err(AiError::Malformed))
        }
    }
}

/// A gated backend over a [`RecordingBackend`] bound for the relay under `eu-native`, plus the
/// handle to what it saw.
pub(crate) fn gated(
    answers: Vec<Result<ChatResponse, AiError>>,
) -> (GatedBackend, Arc<Mutex<Vec<ChatRequest>>>) {
    let backend = RecordingBackend::answering(answers);
    let seen = Arc::clone(&backend.seen);
    (
        GatedBackend::new(
            Box::new(backend),
            Destination::AllodiaRelay,
            Arc::new(|| Mode::EuNative),
        ),
        seen,
    )
}

/// An answer carrying `content` as its text.
pub(crate) fn text_answer(content: &str) -> ChatResponse {
    serde_json::from_value(serde_json::json!({
        "choices": [{ "message": { "role": "assistant", "content": content } }],
    }))
    .unwrap()
}

/// An answer calling `emit_json` with `arguments`.
pub(crate) fn tool_answer(arguments: &serde_json::Value) -> ChatResponse {
    serde_json::from_value(serde_json::json!({
        "choices": [{ "message": {
            "role": "assistant",
            "tool_calls": [{ "id": "call-1", "type": "function", "function": {
                "name": "emit_json",
                "arguments": arguments.to_string(),
            }}],
        }}],
        "allodia": { "credits_charged": 1.5, "balance_credits": 98.5 },
    }))
    .unwrap()
}

/// A request as the transport received it, owned.
#[derive(Debug, Clone)]
pub(crate) struct SentRequest {
    pub(crate) url: String,
    pub(crate) bearer: Option<String>,
    pub(crate) body: serde_json::Value,
}

/// Answers every request with `answer` and records what was sent.
pub(crate) struct CannedTransport {
    pub(crate) answer: Result<HttpResponse, TransportFailed>,
    pub(crate) sent: Arc<Mutex<Vec<SentRequest>>>,
}

impl CannedTransport {
    pub(crate) fn new(status: u16, body: &str) -> Self {
        Self {
            answer: Ok(HttpResponse {
                status,
                body: body.to_owned(),
            }),
            sent: Arc::default(),
        }
    }
}

impl HttpTransport for CannedTransport {
    fn post_json(&self, request: HttpRequest<'_>) -> Result<HttpResponse, TransportFailed> {
        self.sent.lock().unwrap().push(SentRequest {
            url: request.url.to_owned(),
            bearer: request.bearer.map(str::to_owned),
            body: serde_json::from_str(request.body).unwrap(),
        });
        self.answer.clone()
    }
}
