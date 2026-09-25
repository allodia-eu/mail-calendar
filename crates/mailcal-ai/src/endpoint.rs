//! An endpoint the person runs or rents themselves: any server that speaks OpenAI's
//! chat-completions API.
//!
//! The free counterpart of Allodia's relay (`docs/pledge.md`), and the double every test of the
//! pipeline runs against. The request shaper lives here; the socket is the host's
//! ([`HttpTransport`]).

use std::{borrow::Cow, fmt, time::Instant};

use mailcal_jurisdiction::Class;

use crate::{
    AiBackend, AiError, report,
    transport::{HttpRequest, HttpResponse, HttpTransport},
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
    reasoning_effort: Option<String>,
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
            reasoning_effort: None,
        })
    }

    /// The same endpoint, asking for `effort` as `reasoning_effort`: how much a model that can
    /// think does, in OpenAI's name for it, which routers pass on. Only training mode asks; a draft
    /// otherwise gets whatever the model does by default.
    #[must_use]
    pub fn with_reasoning_effort(self, effort: &str) -> Self {
        Self {
            reasoning_effort: Some(effort.to_owned()),
            ..self
        }
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

    /// The body as sent: the request, plus the model, an explicit refusal of streaming and any
    /// reasoning effort asked for.
    fn body(&self, request: &ChatRequest) -> Result<String, AiError> {
        let mut body = serde_json::to_value(request).map_err(|_| AiError::Malformed)?;
        let object = body.as_object_mut().ok_or(AiError::Malformed)?;
        object.insert("model".to_owned(), self.endpoint.model.clone().into());
        object.insert("stream".to_owned(), false.into());
        if let Some(effort) = &self.endpoint.reasoning_effort {
            object.insert("reasoning_effort".to_owned(), effort.clone().into());
        }
        Ok(body.to_string())
    }

    /// Posts `request` once, and logs the exchange.
    fn post(&self, request: &ChatRequest) -> Result<HttpResponse, AiError> {
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
        Ok(answered)
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
        let mut request = Cow::Borrowed(request);
        let mut resent = 0;
        loop {
            let answered = self.post(&request)?;
            match answered.status {
                200..=299 => return report::read_answer(&answered.body),
                400 => {
                    let Some((fewer, dropped)) = without_unsupported(&request, &answered.body)
                    else {
                        return Err(AiError::Status(400));
                    };
                    resent += 1;
                    report::log_resent(request.purpose, dropped, resent);
                    request = Cow::Owned(fewer);
                }
                401 | 403 => return Err(AiError::Unauthorized),
                429 => return Err(AiError::RateLimited),
                status => return Err(AiError::Status(status)),
            }
        }
    }
}

/// `request` without what a refusal says the model does not support, and what that was, when the
/// refusal names the forced tool choice or tools and the request still carries it. Each is
/// dropped once at most, so a request is sent at most three times.
fn without_unsupported(
    request: &ChatRequest,
    refusal: &str,
) -> Option<(ChatRequest, &'static str)> {
    let said = refusal.to_lowercase();
    if !said.contains("support") {
        return None;
    }
    if request.tool_choice.is_some()
        && (said.contains("tool_choice") || said.contains("tool choice"))
    {
        let fewer = ChatRequest {
            tool_choice: None,
            ..request.clone()
        };
        return Some((fewer, "a forced tool"));
    }
    let names_tools = ["tools", "tool use", "tool calling", "function calling"]
        .iter()
        .any(|name| said.contains(name));
    if !request.tools.is_empty() && names_tools {
        let fewer = ChatRequest {
            tools: Vec::new(),
            tool_choice: None,
            ..request.clone()
        };
        return Some((fewer, "tools"));
    }
    None
}

#[cfg(test)]
#[path = "endpoint_tests.rs"]
mod tests;
