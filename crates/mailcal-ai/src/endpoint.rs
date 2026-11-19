//! An endpoint the person runs or rents themselves: any server that speaks OpenAI's
//! chat-completions API.
//!
//! Also the double every test of the pipeline runs against. The request shaper lives here; the
//! socket is the host's ([`HttpTransport`]).

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
}

/// Why an own endpoint was not accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum EndpointError {
    /// No address was given.
    #[error("an address is required")]
    NoAddress,
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
    /// a loopback address), where a local model server usually listens. Every field is used
    /// without the spaces around it, the key included.
    ///
    /// # Errors
    ///
    /// Returns the first of [`endpoint_errors`].
    pub fn new(
        base_url: &str,
        api_key: Option<String>,
        model: &str,
        declared: Option<Class>,
    ) -> Result<Self, EndpointError> {
        if let Some(first) = endpoint_errors(base_url, model).first() {
            return Err(*first);
        }
        let parsed = url::Url::parse(base_url.trim()).map_err(|_| EndpointError::InvalidUrl)?;
        Ok(Self {
            base_url: parsed.as_str().trim_end_matches('/').to_owned(),
            api_key: api_key
                .map(|key| key.trim().to_owned())
                .filter(|key| !key.is_empty()),
            model: model.trim().to_owned(),
            declared,
        })
    }

    /// The name of the host requests go to, which is what a consent names.
    #[must_use]
    pub fn host(&self) -> String {
        url::Url::parse(&self.base_url)
            .ok()
            .and_then(|url| url.host_str().map(str::to_owned))
            .unwrap_or_default()
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

/// Every reason `base_url` and `model` would not make an endpoint, in the order the fields are
/// shown: what is wrong with the address, then a missing model. Empty when both are usable.
///
/// [`OwnEndpoint::new`] refuses by the first of these, so a form that shows them all before a
/// save and the save itself cannot disagree.
#[must_use]
pub fn endpoint_errors(base_url: &str, model: &str) -> Vec<EndpointError> {
    let mut errors = Vec::new();
    if let Err(error) = address_error(base_url.trim()) {
        errors.push(error);
    }
    if model.trim().is_empty() {
        errors.push(EndpointError::NoModel);
    }
    errors
}

fn address_error(base_url: &str) -> Result<(), EndpointError> {
    if base_url.is_empty() {
        return Err(EndpointError::NoAddress);
    }
    let parsed = url::Url::parse(base_url).map_err(|_| EndpointError::InvalidUrl)?;
    if !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(EndpointError::InvalidUrl);
    }
    match parsed.scheme() {
        "https" => Ok(()),
        "http" if is_this_device(&parsed) => Ok(()),
        "http" => Err(EndpointError::NotHttps),
        _ => Err(EndpointError::InvalidUrl),
    }
}

/// Whether two addresses reach the same server: the same host and port as the URL parser
/// normalises them, a scheme's default port counting as none given, so a change of path or of
/// scheme alone keeps the server. An address that is not a URL reaches none.
#[must_use]
pub fn same_server(a: &str, b: &str) -> bool {
    let server = |address: &str| {
        let url = url::Url::parse(address.trim()).ok()?;
        Some((url.host()?.to_owned(), url.port()))
    };
    matches!((server(a), server(b)), (Some(a), Some(b)) if a == b)
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
