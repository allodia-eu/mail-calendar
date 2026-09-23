//! Allodia's relay for writing style and drafted replies, as a `mailcal_ai::AiBackend`
//! (`docs/ai.md`, "Where requests go").
//!
//! The request is the one `mailcal-ai` built, without a model (the gateway picks it) and with the
//! purpose the gateway picks it by. The answer is one JSON object, the relay having collected the
//! gateway's stream, with what the request cost and what is left under `allodia`. The relay gates
//! the credits; this side only draws what it is told.
//!
//! **Over `mailcal-ai`'s transport, not this crate's**: a learning request can run for minutes,
//! and only that port carries a per-request timeout. The access token is the host's to mint, and
//! a failure to mint one comes back as `Unauthorized`, which a client answers with "sign in again".

use std::fmt;

use mailcal_ai::{
    AiBackend, AiError, HttpRequest, HttpTransport,
    wire::{ChatRequest, ChatResponse},
};
use serde::Deserialize;

use crate::{API_BASE_PATH, AccountService, Error, Request, Transport};

/// Mints an access token for the relay, refreshing it when it has to.
pub type TokenSource = Box<dyn Fn() -> Result<String, AiError> + Send + Sync>;

/// The relay at the Mail & Calendar service.
pub struct Relay {
    url: String,
    token: TokenSource,
    transport: Box<dyn HttpTransport>,
}

impl Relay {
    /// The relay of the service at `service`, asked with tokens from `token` over `transport`.
    #[must_use]
    pub fn new(
        service: &AccountService,
        token: TokenSource,
        transport: Box<dyn HttpTransport>,
    ) -> Self {
        Self {
            url: format!("{}{API_BASE_PATH}/ai/chat", service.base_url()),
            token,
            transport,
        }
    }
}

impl fmt::Debug for Relay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Relay").finish_non_exhaustive()
    }
}

impl AiBackend for Relay {
    fn chat(&self, request: &ChatRequest) -> Result<ChatResponse, AiError> {
        let token = (self.token)()?;
        let mut body = serde_json::to_value(request).map_err(|_| AiError::Malformed)?;
        body.as_object_mut().ok_or(AiError::Malformed)?.insert(
            "purpose".to_owned(),
            serde_json::to_value(request.purpose).map_err(|_| AiError::Malformed)?,
        );
        let body = body.to_string();
        let answered = self
            .transport
            .post_json(HttpRequest {
                url: &self.url,
                bearer: Some(&token),
                body: &body,
                timeout: request.purpose.timeout(),
            })
            .map_err(|_| AiError::Unreachable)?;
        match answered.status {
            200..=299 => serde_json::from_str(&answered.body).map_err(|_| AiError::Malformed),
            402 => Err(AiError::OutOfCredits),
            403 if refusal_code(&answered.body).as_deref() == Some("not_entitled") => {
                Err(AiError::NotEntitled)
            }
            401 | 403 => Err(AiError::Unauthorized),
            429 => Err(AiError::RateLimited),
            status => Err(AiError::Status(status)),
        }
    }
}

/// The service's own code for a refusal, from its `data.code` envelope.
fn refusal_code(body: &str) -> Option<String> {
    #[derive(Deserialize)]
    struct Envelope {
        data: Option<Code>,
    }
    #[derive(Deserialize)]
    struct Code {
        code: String,
    }
    serde_json::from_str::<Envelope>(body)
        .ok()?
        .data
        .map(|data| data.code)
}

/// The person's AI credits, as the service last said. For display: the relay decides.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Balance {
    /// Credits left.
    pub balance_credits: f64,
    /// Whether the one-time starting grant has been applied.
    #[serde(default)]
    pub starting_grant_applied: bool,
}

impl AccountService {
    /// Ask how many AI credits the signed-in account has left.
    ///
    /// # Errors
    /// [`Error::Unauthorized`] when the token needs refreshing or lacks the scope;
    /// [`Error::Transport`] when the request never arrived; [`Error::Malformed`] when the answer
    /// cannot be read.
    pub fn ai_balance(
        &self,
        transport: &dyn Transport,
        access_token: &str,
    ) -> Result<Balance, Error> {
        let response = transport
            .send(&Request::get(
                format!("{}{API_BASE_PATH}/ai/balance", self.base_url()),
                access_token,
            ))
            .map_err(Error::Transport)?;
        crate::subscription::answered(&response)
    }
}

#[cfg(test)]
#[path = "ai_tests.rs"]
mod tests;
