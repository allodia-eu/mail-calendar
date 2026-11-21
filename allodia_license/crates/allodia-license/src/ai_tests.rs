// SPDX-FileCopyrightText: 2026 Allodia
// SPDX-License-Identifier: LicenseRef-Allodia-1.0

use std::sync::{Arc, Mutex};

use mailcal_ai::{
    AiBackend, AiError, HttpRequest, HttpResponse, HttpTransport, TransportFailed,
    wire::{ChatMessage, ChatRequest, Purpose},
};

use super::Relay;
use crate::AccountService;

/// What the transport was sent: the URL, the bearer, the body and the timeout in seconds.
type Sent = Arc<Mutex<Vec<(String, Option<String>, serde_json::Value, u64)>>>;

/// Answers every request with one status and body, or with no answer, and keeps what it was sent.
struct Canned {
    answer: Option<(u16, &'static str)>,
    sent: Sent,
}

impl HttpTransport for Canned {
    fn post_json(&self, request: HttpRequest<'_>) -> Result<HttpResponse, TransportFailed> {
        self.sent.lock().unwrap().push((
            request.url.to_owned(),
            request.bearer.map(str::to_owned),
            serde_json::from_str(request.body).unwrap(),
            request.timeout.as_secs(),
        ));
        let (status, body) = self.answer.ok_or(TransportFailed)?;
        Ok(HttpResponse {
            status,
            body: body.to_owned(),
        })
    }
}

fn relay_with(
    answer: Option<(u16, &'static str)>,
    token: Result<&'static str, AiError>,
) -> (Relay, Sent) {
    let sent = Arc::default();
    let relay = Relay::new(
        &AccountService::new("https://mailcal.example.eu/"),
        Box::new(move || token.clone().map(str::to_owned)),
        Box::new(Canned {
            answer,
            sent: Arc::clone(&sent),
        }),
    );
    (relay, sent)
}

fn relay(status: u16, body: &'static str) -> (Relay, Sent) {
    relay_with(Some((status, body)), Ok("access-token"))
}

fn request(purpose: Purpose) -> ChatRequest {
    ChatRequest {
        purpose,
        messages: vec![ChatMessage::system("rules"), ChatMessage::user("hello")],
        tools: Vec::new(),
        tool_choice: None,
        temperature: Some(0.5),
        max_tokens: Some(100),
    }
}

/// The shape the service's `RelayChatRequest` takes: `purpose` and the chat-completions fields,
/// and never `model`, `models` or `provider`, each of which it refuses with 400.
#[test]
fn a_request_names_its_purpose_and_no_model_and_reads_the_charge() {
    let (relay, sent) = relay(
        200,
        r#"{"choices":[{"index":0,"message":{"role":"assistant","content":"Hi","tool_calls":[]},
            "finish_reason":"stop"}],"usage":null,
            "allodia":{"credits_charged":1.25,"balance_credits":498.75}}"#,
    );

    let answer = relay.chat(&request(Purpose::Style)).unwrap();

    assert_eq!(
        answer
            .answer()
            .and_then(|message| message.content.as_deref()),
        Some("Hi")
    );
    let metering = answer.allodia.unwrap();
    assert!((metering.balance_credits - 498.75).abs() < f64::EPSILON);
    let sent = sent.lock().unwrap();
    let (url, bearer, body, timeout) = &sent[0];
    assert_eq!(url, "https://mailcal.example.eu/api/v1/ai/chat");
    assert_eq!(bearer.as_deref(), Some("access-token"));
    assert_eq!(body["purpose"], "style");
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(body["messages"][1]["content"], "hello");
    assert_eq!(body["max_tokens"], 100);
    for chosen_by_the_gateway in ["model", "models", "provider"] {
        assert!(body.get(chosen_by_the_gateway).is_none());
    }
    assert_eq!(*timeout, Purpose::Style.timeout().as_secs());
}

#[test]
fn a_draft_waits_as_long_as_a_draft_may() {
    let (relay, sent) = relay(200, r#"{"choices":[]}"#);
    relay.chat(&request(Purpose::Draft)).unwrap();
    let sent = sent.lock().unwrap();
    assert_eq!(sent[0].2["purpose"], "draft");
    assert_eq!(sent[0].3, Purpose::Draft.timeout().as_secs());
}

/// Every refusal the service names (`data.code`), and what the core makes of each: 401 is a
/// sign-in, 402 no credits, 403 `not_entitled` the entitlement, 429 too many, 413 a sample too
/// large, and the rest a server error. Never the service's own words.
#[test]
fn each_refusal_has_its_own_error_and_never_the_service_s_words() {
    for (status, body, expected) in [
        (401, "", AiError::Unauthorized),
        (
            402,
            r#"{"code":"PAYMENT_REQUIRED","data":{"code":"insufficient_credits"}}"#,
            AiError::OutOfCredits,
        ),
        (
            403,
            r#"{"code":"FORBIDDEN","message":"Upgrade!","data":{"code":"not_entitled"}}"#,
            AiError::NotEntitled,
        ),
        (
            403,
            r#"{"error":"insufficient_scope"}"#,
            AiError::Unauthorized,
        ),
        (
            429,
            r#"{"data":{"code":"rate_limited","retryAfterSeconds":60}}"#,
            AiError::RateLimited,
        ),
        (
            413,
            r#"{"data":{"code":"too_large"}}"#,
            AiError::Status(413),
        ),
        (
            502,
            r#"{"data":{"code":"upstream_failed"}}"#,
            AiError::Status(502),
        ),
        (
            503,
            r#"{"data":{"code":"unavailable"}}"#,
            AiError::Status(503),
        ),
        (400, "<html>", AiError::Status(400)),
    ] {
        let (relay, _) = relay(status, body);
        assert_eq!(
            relay.chat(&request(Purpose::Draft)).unwrap_err(),
            expected,
            "{status} {body}"
        );
    }
}

#[test]
fn a_success_that_is_not_an_answer_is_unreadable() {
    let (relay, _) = relay(200, "<html>");
    assert_eq!(
        relay.chat(&request(Purpose::Draft)).unwrap_err(),
        AiError::Malformed
    );
}

#[test]
fn no_answer_is_unreachable() {
    let (relay, sent) = relay_with(None, Ok("access-token"));
    assert_eq!(
        relay.chat(&request(Purpose::Draft)).unwrap_err(),
        AiError::Unreachable
    );
    assert_eq!(sent.lock().unwrap().len(), 1);
}

/// A token that cannot be minted stops the request before it is sent, and says why.
#[test]
fn no_token_means_nothing_is_sent() {
    for refusal in [AiError::Unauthorized, AiError::Unreachable] {
        let (relay, sent) = relay_with(Some((200, "{}")), Err(refusal.clone()));
        assert_eq!(relay.chat(&request(Purpose::Draft)).unwrap_err(), refusal);
        assert!(sent.lock().unwrap().is_empty());
    }
}
