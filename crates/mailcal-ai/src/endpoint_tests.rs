use std::sync::{Arc, Mutex};

use mailcal_jurisdiction::{Class, Mode};

use super::{EndpointError, OwnEndpoint};
use crate::{
    AiError, GatedBackend, LanguageStyle,
    test_support::CannedTransport,
    tool,
    transport::TransportFailed,
    wire::{ChatMessage, ChatRequest, Purpose},
};

fn endpoint(base: &str) -> Result<OwnEndpoint, EndpointError> {
    OwnEndpoint::new(
        base,
        Some("sk-secret".to_owned()),
        "mistral-small",
        Some(Class::EuNative),
    )
}

fn request() -> ChatRequest {
    ChatRequest {
        purpose: Purpose::Draft,
        messages: vec![ChatMessage::system("be brief"), ChatMessage::user("hello")],
        tools: Vec::new(),
        tool_choice: None,
        temperature: Some(0.4),
        max_tokens: Some(800),
    }
}

fn gated(transport: CannedTransport) -> GatedBackend {
    GatedBackend::own_endpoint(
        endpoint("https://api.example.eu/v1/").unwrap(),
        Box::new(transport),
        Arc::new(|| Mode::EuNative),
    )
}

#[test]
fn https_is_required_except_to_this_device() {
    assert!(endpoint("https://api.example.eu/v1").is_ok());
    assert!(endpoint("http://localhost:11434/v1").is_ok());
    assert!(endpoint("http://127.0.0.1:8080/v1").is_ok());
    assert!(endpoint("http://[::1]:8080/v1").is_ok());
    assert_eq!(
        endpoint("http://192.168.1.20:11434/v1"),
        Err(EndpointError::NotHttps)
    );
    assert_eq!(
        endpoint("http://api.example.eu/v1"),
        Err(EndpointError::NotHttps)
    );
    assert_eq!(
        endpoint("ftp://api.example.eu/v1"),
        Err(EndpointError::InvalidUrl)
    );
}

#[test]
fn an_address_carrying_credentials_or_a_query_is_refused() {
    for base in [
        "https://user:pass@api.example.eu/v1",
        "https://api.example.eu/v1?key=secret",
        "https://api.example.eu/v1#part",
        "not a url",
    ] {
        assert_eq!(endpoint(base), Err(EndpointError::InvalidUrl), "{base}");
    }
}

#[test]
fn a_model_is_required_and_a_blank_key_is_no_key() {
    assert_eq!(
        OwnEndpoint::new("https://api.example.eu/v1", None, "  ", None),
        Err(EndpointError::NoModel)
    );
    let keyless = OwnEndpoint::new(
        "https://api.example.eu/v1",
        Some("  ".to_owned()),
        "m",
        None,
    )
    .unwrap();
    assert!(!format!("{keyless:?}").contains("has_key: true"));
}

#[test]
fn the_key_never_reaches_debug_output() {
    let printed = format!("{:?}", endpoint("https://api.example.eu/v1").unwrap());
    assert!(!printed.contains("sk-secret"));
    assert!(!printed.contains("example"));
}

#[test]
fn a_request_carries_the_model_the_key_and_no_streaming() {
    let transport = CannedTransport::new(
        200,
        r#"{"choices":[{"message":{"role":"assistant","content":"Hi"}}]}"#,
    );
    let sent = Arc::clone(&transport.sent);

    let answer = gated(transport).chat(&request()).unwrap();

    assert_eq!(answer.answer().unwrap().content.as_deref(), Some("Hi"));
    let sent = sent.lock().unwrap();
    assert_eq!(sent[0].url, "https://api.example.eu/v1/chat/completions");
    assert_eq!(sent[0].bearer.as_deref(), Some("sk-secret"));
    let body = &sent[0].body;
    assert_eq!(body["model"], "mistral-small");
    assert_eq!(body["stream"], false);
    assert_eq!(body["max_tokens"], 800);
    assert_eq!(body["messages"][0]["role"], "system");
    // The purpose is the relay's business: an own endpoint is never sent a field it may reject.
    assert!(body.get("purpose").is_none());
}

#[test]
fn a_reasoning_effort_is_sent_only_when_one_is_asked_for() {
    let answer = r#"{"choices":[{"message":{"role":"assistant","content":"Hi"}}]}"#;
    let plain = CannedTransport::new(200, answer);
    let plain_sent = Arc::clone(&plain.sent);
    gated(plain).chat(&request()).unwrap();
    assert!(
        plain_sent.lock().unwrap()[0]
            .body
            .get("reasoning_effort")
            .is_none()
    );

    let thinking = CannedTransport::new(200, answer);
    let thinking_sent = Arc::clone(&thinking.sent);
    let endpoint = endpoint("https://api.example.eu/v1")
        .unwrap()
        .with_reasoning_effort("low");
    GatedBackend::own_endpoint(endpoint, Box::new(thinking), Arc::new(|| Mode::EuNative))
        .chat(&request())
        .unwrap();
    assert_eq!(
        thinking_sent.lock().unwrap()[0].body["reasoning_effort"],
        "low"
    );
}

#[test]
fn each_failure_status_has_its_own_error() {
    for (status, expected) in [
        (401, AiError::Unauthorized),
        (403, AiError::Unauthorized),
        (429, AiError::RateLimited),
        (404, AiError::Status(404)),
        (500, AiError::Status(500)),
    ] {
        let error = gated(CannedTransport::new(status, "{}"))
            .chat(&request())
            .unwrap_err();
        assert_eq!(error, expected, "{status}");
    }
    let unreadable = gated(CannedTransport::new(200, "<html>"))
        .chat(&request())
        .unwrap_err();
    assert_eq!(unreadable, AiError::Malformed);
}

#[test]
fn no_answer_at_all_is_unreachable() {
    let transport = CannedTransport {
        first: Mutex::default(),
        answer: Err(TransportFailed),
        sent: Arc::default(),
    };
    assert_eq!(
        gated(transport).chat(&request()).unwrap_err(),
        AiError::Unreachable
    );
}

#[test]
fn an_undeclared_endpoint_is_refused_before_the_transport_is_touched() {
    let transport = CannedTransport::new(200, "{}");
    let sent = Arc::clone(&transport.sent);
    let gated = GatedBackend::own_endpoint(
        OwnEndpoint::new("https://api.example.com/v1", None, "m", None).unwrap(),
        Box::new(transport),
        Arc::new(|| Mode::EuNative),
    );

    assert!(matches!(
        gated.chat(&request()),
        Err(AiError::Refused(refused)) if refused.class == Class::Unknown
    ));
    assert!(sent.lock().unwrap().is_empty());
}

/// A request forcing `emit_json`, as every draft and learning request does.
fn forced() -> ChatRequest {
    let (tool, choice) = tool::forced::<LanguageStyle>("describe");
    ChatRequest {
        tools: vec![tool],
        tool_choice: Some(choice),
        ..request()
    }
}

const NO_TOOL_CHOICE: &str = r#"{"error":{"message":"Model 'deepseek-v4.1-flash' does not support tool_choice.","type":"invalid_request_error"}}"#;
const NO_TOOLS: &str = r#"{"error":"registry.ollama.ai/library/gemma:2b does not support tools"}"#;
const ANSWERED: &str = r#"{"choices":[{"message":{"content":"{\"register\":\"u\"}"}}]}"#;

#[test]
fn a_model_that_refuses_the_forced_choice_is_asked_again_without_it() {
    let transport = CannedTransport::after(&[(400, NO_TOOL_CHOICE)], 200, ANSWERED);
    let sent = Arc::clone(&transport.sent);

    let answer = gated(transport).chat(&forced()).unwrap();

    assert!(answer.answer().unwrap().content.is_some());
    let sent = sent.lock().unwrap();
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[0].body["tool_choice"]["function"]["name"], "emit_json");
    assert!(sent[1].body.get("tool_choice").is_none());
    assert_eq!(sent[1].body["tools"][0]["function"]["name"], "emit_json");
}

#[test]
fn a_model_without_tools_is_asked_again_with_none() {
    let transport = CannedTransport::after(&[(400, NO_TOOLS)], 200, ANSWERED);
    let sent = Arc::clone(&transport.sent);

    gated(transport).chat(&forced()).unwrap();

    let sent = sent.lock().unwrap();
    assert_eq!(sent.len(), 2);
    assert!(sent[1].body.get("tools").is_none());
    assert!(sent[1].body.get("tool_choice").is_none());
    assert_eq!(sent[1].body["messages"], sent[0].body["messages"]);
}

#[test]
fn each_refusal_is_answered_once_and_then_the_status_stands() {
    let transport = CannedTransport::after(
        &[(400, NO_TOOL_CHOICE), (400, NO_TOOLS)],
        400,
        NO_TOOL_CHOICE,
    );
    let sent = Arc::clone(&transport.sent);

    assert_eq!(
        gated(transport).chat(&forced()).unwrap_err(),
        AiError::Status(400)
    );
    assert_eq!(sent.lock().unwrap().len(), 3);
}

#[test]
fn any_other_refusal_is_not_repeated() {
    for (body, request) in [
        (
            r#"{"error":{"message":"max_tokens is too large"}}"#,
            forced(),
        ),
        (NO_TOOLS, request()),
    ] {
        let transport = CannedTransport::new(400, body);
        let sent = Arc::clone(&transport.sent);
        assert_eq!(
            gated(transport).chat(&request).unwrap_err(),
            AiError::Status(400)
        );
        assert_eq!(sent.lock().unwrap().len(), 1, "{body}");
    }
}
