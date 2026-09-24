//! An endpoint the person runs or rents themselves: any server that speaks OpenAI's
//! chat-completions API.
//!
//! The free counterpart of Allodia's relay (`docs/pledge.md`), and the double every test of the
//! pipeline runs against. The request shaper lives here; the socket is the host's
//! ([`HttpTransport`]).

use std::{fmt, time::Instant};

use mailcal_jurisdiction::Class;

use crate::{
    AiBackend, AiError, report,
    transport::{HttpRequest, HttpTransport},
    wire::{ChatRequest, ChatResponse},
};

/// A validated own endpoint: where it is, the key it wants, the model to ask for, and where the
/// person says it runs.
#[derive(Clone, PartialEq, Eq)]
pub struct OwnEndpoint {
    base_url: String,
    api_key: Option<String>,
    model: String,
    declared: Option<Class>,
}

/// Why an own endpoint was not accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum EndpointError {
    /// The address is not a URL, or carries a user name, a query or a fragment.
    #[error("the address is not a usable URL")]
    InvalidUrl,
    /// The address is plain HTTP to somewhere other than this device, which would send the key
    /// and the mail in the clear.
    #[error("the address must use HTTPS unless it is on this device")]
    NotHttps,
    /// No model name was given.
    #[error("a model name is required")]
    NoModel,
}

impl OwnEndpoint {
    /// Validates the four settings.
    ///
    /// The base URL is what the provider documents as its API root, `https://api.example.eu/v1`;
    /// `/chat/completions` is appended. Plain HTTP is accepted only to this device (`localhost`,
    /// a loopback address), where a local model server usually listens.
    ///
    /// # Errors
    ///
    /// Returns [`EndpointError`] naming the first setting that is not usable.
    pub fn new(
        base_url: &str,
        api_key: Option<String>,
        model: &str,
        declared: Option<Class>,
    ) -> Result<Self, EndpointError> {
        let parsed = url::Url::parse(base_url.trim()).map_err(|_| EndpointError::InvalidUrl)?;
        if !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
        {
            return Err(EndpointError::InvalidUrl);
        }
        match parsed.scheme() {
            "https" => {}
            "http" if is_this_device(&parsed) => {}
            "http" => return Err(EndpointError::NotHttps),
            _ => return Err(EndpointError::InvalidUrl),
        }
        let model = model.trim();
        if model.is_empty() {
            return Err(EndpointError::NoModel);
        }
        Ok(Self {
            base_url: parsed.as_str().trim_end_matches('/').to_owned(),
            api_key: api_key
                .map(|key| key.trim().to_owned())
                .filter(|key| !key.is_empty()),
            model: model.to_owned(),
            declared,
        })
    }

    /// The base URL as validated.
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// The model asked for.
    #[must_use]
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Where the person says it runs.
    #[must_use]
    pub fn declared(&self) -> Option<Class> {
        self.declared
    }

    fn completions_url(&self) -> String {
        format!("{}/chat/completions", self.base_url)
    }
}

impl fmt::Debug for OwnEndpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OwnEndpoint")
            .field("has_key", &self.api_key.is_some())
            .field("declared", &self.declared)
            .finish_non_exhaustive()
    }
}

/// Whether a URL's host is this device.
fn is_this_device(url: &url::Url) -> bool {
    match url.host() {
        Some(url::Host::Domain(name)) => name.eq_ignore_ascii_case("localhost"),
        Some(url::Host::Ipv4(address)) => address.is_loopback(),
        Some(url::Host::Ipv6(address)) => address.is_loopback(),
        None => false,
    }
}

/// Shapes a request for an own endpoint and reads its answer.
pub(crate) struct OpenAiCompatibleBackend {
    endpoint: OwnEndpoint,
    transport: Box<dyn HttpTransport>,
}

impl OpenAiCompatibleBackend {
    pub(crate) fn new(endpoint: OwnEndpoint, transport: Box<dyn HttpTransport>) -> Self {
        Self {
            endpoint,
            transport,
        }
    }

    /// The body as sent: the request, plus the model and an explicit refusal of streaming.
    fn body(&self, request: &ChatRequest) -> Result<String, AiError> {
        let mut body = serde_json::to_value(request).map_err(|_| AiError::Malformed)?;
        let object = body.as_object_mut().ok_or(AiError::Malformed)?;
        object.insert("model".to_owned(), self.endpoint.model.clone().into());
        object.insert("stream".to_owned(), false.into());
        Ok(body.to_string())
    }
}

impl fmt::Debug for OpenAiCompatibleBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenAiCompatibleBackend")
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}

impl AiBackend for OpenAiCompatibleBackend {
    fn chat(&self, request: &ChatRequest) -> Result<ChatResponse, AiError> {
        let body = self.body(request)?;
        let url = self.endpoint.completions_url();
        let started = Instant::now();
        let answered = self
            .transport
            .post_json(HttpRequest {
                url: &url,
                bearer: self.endpoint.api_key.as_deref(),
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
            401 | 403 => Err(AiError::Unauthorized),
            429 => Err(AiError::RateLimited),
            status => Err(AiError::Status(status)),
        }
    }
}

#[cfg(test)]
#[path = "endpoint_tests.rs"]
mod tests;
