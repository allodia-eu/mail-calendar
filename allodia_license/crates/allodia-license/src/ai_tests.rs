use std::sync::{Arc, Mutex};

use mailcal_ai::{
    AiBackend, AiError, HttpRequest, HttpResponse, HttpTransport, TransportFailed,
    wire::{ChatMessage, ChatRequest, Purpose},
};

use super::{Balance, Relay};
use crate::{AccountService, Error, Request, Response, Transport};

/// What the transport was sent: the URL, the bearer, the body.
type Sent = Arc<Mutex<Vec<(String, Option<String>, serde_json::Value)>>>;

/// Answers every request with one status and body, and keeps what it was sent.
struct Canned {
    status: u16,
    body: &'static str,
    sent: Sent,
}

impl HttpTransport for Canned {
    fn post_json(&self, request: HttpRequest<'_>) -> Result<HttpResponse, TransportFailed> {
        self.sent.lock().unwrap().push((
            request.url.to_owned(),
            request.bearer.map(str::to_owned),
            serde_json::from_str(request.body).unwrap(),
        ));
        Ok(HttpResponse {
            status: self.status,
            body: self.body.to_owned(),
        })
    }
}

fn relay(status: u16, body: &'static str) -> (Relay, Sent) {
    let sent = Arc::default();
    let relay = Relay::new(
        &AccountService::new("https://mailcal.example.eu/"),
        Box::new(|| Ok("access-token".to_owned())),
        Box::new(Canned {
            status,
            body,
            sent: Arc::clone(&sent),
        }),
    );
    (relay, sent)
}

fn request(purpose: Purpose) -> ChatRequest {
    ChatRequest {
        purpose,
        messages: vec![ChatMessage::user("hello")],
        tools: Vec::new(),
        tool_choice: None,
        temperature: Some(0.5),
        max_tokens: Some(100),
    }
}

#[test]
fn a_request_names_its_purpose_and_no_model_and_reads_the_charge() {
    let (relay, sent) = relay(
        200,
        r#"{"choices":[{"message":{"content":"Hi"}}],
            "allodia":{"credits_charged":1.25,"balance_credits":498.75}}"#,
    );

    let answer = relay.chat(&request(Purpose::Style)).unwrap();

    let metering = answer.allodia.unwrap();
    assert!((metering.balance_credits - 498.75).abs() < f64::EPSILON);
    let sent = sent.lock().unwrap();
    let (url, bearer, body) = &sent[0];
    assert_eq!(url, "https://mailcal.example.eu/api/v1/ai/chat");
    assert_eq!(bearer.as_deref(), Some("access-token"));
    assert_eq!(body["purpose"], "style");
    assert!(body.get("model").is_none());
    assert!(body.get("models").is_none());
    assert_eq!(body["max_tokens"], 100);
}

#[test]
fn each_refusal_has_its_own_error_and_never_the_service_s_words() {
    for (status, body, expected) in [
        (
            402,
            r#"{"data":{"code":"insufficient_credits"}}"#,
            AiError::OutOfCredits,
        ),
        (
            403,
            r#"{"data":{"code":"not_entitled","message":"Upgrade!"}}"#,
            AiError::NotEntitled,
        ),
        (
            403,
            r#"{"data":{"code":"insufficient_scope"}}"#,
            AiError::Unauthorized,
        ),
        (401, "", AiError::Unauthorized),
        (429, "", AiError::RateLimited),
        (502, "<html>", AiError::Status(502)),
    ] {
        let (relay, _) = relay(status, body);
        assert_eq!(
            relay.chat(&request(Purpose::Draft)).unwrap_err(),
            expected,
            "{status}"
        );
    }
}

/// A token that cannot be minted stops the request before it is sent.
#[test]
fn no_token_means_nothing_is_sent() {
    let sent = Arc::default();
    let relay = Relay::new(
        &AccountService::new("https://mailcal.example.eu"),
        Box::new(|| Err(AiError::Unauthorized)),
        Box::new(Canned {
            status: 200,
            body: "{}",
            sent: Arc::clone(&sent),
        }),
    );
    assert_eq!(
        relay.chat(&request(Purpose::Draft)).unwrap_err(),
        AiError::Unauthorized
    );
    assert!(sent.lock().unwrap().is_empty());
}

struct Answering(u16, &'static str);

impl Transport for Answering {
    fn send(&self, request: &Request) -> Result<Response, String> {
        assert_eq!(request.url, "https://mailcal.example.eu/api/v1/ai/balance");
        Ok(Response {
            status: self.0,
            body: self.1.to_owned(),
        })
    }
}

#[test]
fn the_balance_is_read_from_the_service() {
    let service = AccountService::new("https://mailcal.example.eu");
    let balance = service
        .ai_balance(
            &Answering(
                200,
                r#"{"balanceCredits":12.5,"startingGrantApplied":true}"#,
            ),
            "token",
        )
        .unwrap();
    assert_eq!(
        balance,
        Balance {
            balance_credits: 12.5,
            starting_grant_applied: true,
        }
    );
    assert_eq!(
        service
            .ai_balance(&Answering(401, ""), "token")
            .unwrap_err(),
        Error::Unauthorized
    );
}
