// SPDX-FileCopyrightText: 2026 Allodia
// SPDX-License-Identifier: LicenseRef-Allodia-1.0

//! Allodia's relay for the writing assistant, as a `mailcal_ai::AiBackend` (`docs/ai.md` rule 2).
//!
//! The request is the one `mailcal-ai` built, without a model (the gateway picks it) and with the
//! `purpose` the gateway picks it by. The answer is one JSON object, the relay having collected the
//! gateway's stream, with what the request cost and what is left under `allodia`. The relay
//! decides whether a request may spend; this side reads what it was told.
//!
//! **Over `mailcal-ai`'s transport, not this crate's**: a learning request can run for minutes,
//! and only that port carries a per-request timeout and the person's Stop. The access token is the
//! host's to mint, and a token it cannot mint stops the request before anything is sent.

use std::{fmt, time::Instant};

use mailcal_ai::{
    AiBackend, AiError, HttpRequest, HttpTransport, report,
    wire::{ChatRequest, ChatResponse},
};
use serde::Deserialize;

use crate::{API_BASE_PATH, AccountService};

/// Mints an access token for the relay, or says why none can be had: `Unauthorized` when the
/// sign-in has to be made again, `Unreachable` when the service could not be asked.
pub type TokenSource = Box<dyn Fn() -> Result<String, AiError> + Send + Sync>;

/// The relay's route, relative to the account service's root.
const ROUTE: &str = "/ai/chat";

/// The relay at the Mail & Calendar service.
pub struct Relay {
    url: String,
    token: TokenSource,
    transport: Box<dyn HttpTransport>,
}

impl Relay {
    /// The relay of `service`, asked with tokens from `token` over `transport`.
    #[must_use]
    pub fn new(
        service: &AccountService,
        token: TokenSource,
        transport: Box<dyn HttpTransport>,
    ) -> Self {
        Self {
            url: format!("{}{API_BASE_PATH}{ROUTE}", service.base_url()),
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
        let body = body(request)?;
        let started = Instant::now();
        let answered = self
            .transport
            .post_json(HttpRequest {
                url: &self.url,
                bearer: Some(&token),
                body: &body,
                timeout: request.purpose.timeout(),
            })
            .map_err(|_| {
                report::log_unanswered(request.purpose, body.len(), started.elapsed());
                AiError::Unreachable
            })?;
        report::log_answer(
            request.purpose,
            body.len(),
            started.elapsed(),
            answered.status,
            &answered.body,
        );
        match answered.status {
            200..=299 => report::read_answer(&answered.body),
            402 => Err(AiError::OutOfCredits),
            403 if refusal_code(&answered.body).as_deref() == Some("not_entitled") => {
                Err(AiError::NotEntitled)
            }
            // The service answers a token it will not take, or one without `mailcal:ai:use`, with
            // 401; RFC 6750 puts a missing scope at 403, so both are a sign-in to make again.
            401 | 403 => Err(AiError::Unauthorized),
            429 => Err(AiError::RateLimited),
            status => Err(AiError::Status(status)),
        }
    }
}

/// The request as the relay takes it: the chat-completions body with `purpose` beside it.
fn body(request: &ChatRequest) -> Result<String, AiError> {
    let mut body = serde_json::to_value(request).map_err(|_| AiError::Malformed)?;
    body.as_object_mut().ok_or(AiError::Malformed)?.insert(
        "purpose".to_owned(),
        serde_json::to_value(request.purpose).map_err(|_| AiError::Malformed)?,
    );
    Ok(body.to_string())
}

/// The service's own code for a refusal, from its `data.code`.
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

#[cfg(test)]
#[path = "ai_tests.rs"]
mod tests;
