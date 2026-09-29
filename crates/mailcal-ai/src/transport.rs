//! The one socket-shaped port: posting a JSON body and reading the answer.
//!
//! This crate opens no connection itself. The host implements [`HttpTransport`] over the app's
//! own TLS policy (`mailcal-bindings`), which keeps the choice of TLS stack in one place and keeps
//! this crate testable against a canned answer.

use std::{fmt, time::Duration};

/// One JSON `POST`.
#[derive(Clone, Copy)]
pub struct HttpRequest<'a> {
    /// Where to.
    pub url: &'a str,
    /// The bearer token, when there is one.
    pub bearer: Option<&'a str>,
    /// The JSON body.
    pub body: &'a str,
    /// How long to wait for the whole answer.
    pub timeout: Duration,
}

impl fmt::Debug for HttpRequest<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Neither the address nor the key nor the body: the first two are the person's own
        // configuration and the third is their mail.
        f.debug_struct("HttpRequest")
            .field("bearer", &self.bearer.is_some())
            .field("body_len", &self.body.len())
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

/// The answer: a status and a body, whatever the status.
#[derive(Clone, PartialEq, Eq)]
pub struct HttpResponse {
    /// The HTTP status.
    pub status: u16,
    /// The body.
    pub body: String,
}

impl fmt::Debug for HttpResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpResponse")
            .field("status", &self.status)
            .field("body_len", &self.body.len())
            .finish()
    }
}

/// Makes a JSON `POST`, blocking until the answer or the timeout.
pub trait HttpTransport: Send + Sync {
    /// Sends `request`.
    ///
    /// # Errors
    ///
    /// Returns `Err` only when no answer arrived: a refused connection, a name that would not
    /// resolve, the timeout. An HTTP error status is an answer, and comes back as `Ok`.
    fn post_json(&self, request: HttpRequest<'_>) -> Result<HttpResponse, TransportFailed>;
}

/// No answer arrived. Carries nothing: the host's own message can name the address, and the
/// address is the person's configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("no answer arrived")]
pub struct TransportFailed;
