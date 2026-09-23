//! The socket under `mailcal-ai`: its [`HttpTransport`] port over the app's own TLS policy and
//! runtime.
//!
//! Built from [`mailcal_oauth::discovery_client`] for the reason the Allodia account transport is:
//! `reqwest` is pinned to `rustls-no-provider` workspace-wide, so a client built any other way has
//! no crypto provider and fails at its first request. The port is synchronous and the client is
//! not; [`block_on`] bridges them on the app's runtime, as it does for the account service.
//!
//! Each request carries the timeout its purpose allows, so a model that never answers ends as
//! `Unreachable` rather than as a composer that waits for ever.

use mailcal_ai::{HttpRequest, HttpResponse, HttpTransport, TransportFailed};

use crate::blocking::block_on;

/// AI requests over the app's own TLS policy and runtime.
pub(crate) struct AiTransport {
    http: reqwest::Client,
    handle: tokio::runtime::Handle,
}

impl std::fmt::Debug for AiTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AiTransport")
    }
}

impl AiTransport {
    /// Build one on `handle`'s runtime.
    ///
    /// # Errors
    ///
    /// Returns the shared TLS policy's message when a client cannot be built.
    pub(crate) fn new(handle: tokio::runtime::Handle) -> Result<Self, String> {
        let http = mailcal_oauth::discovery_client().map_err(|error| error.to_string())?;
        Ok(Self { http, handle })
    }

    fn build(&self, request: &HttpRequest<'_>) -> reqwest::RequestBuilder {
        let mut builder = self
            .http
            .post(request.url)
            .timeout(request.timeout)
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(request.body.to_owned());
        if let Some(bearer) = request.bearer {
            builder = builder.bearer_auth(bearer);
        }
        builder
    }
}

impl HttpTransport for AiTransport {
    fn post_json(&self, request: HttpRequest<'_>) -> Result<HttpResponse, TransportFailed> {
        block_on(&self.handle, async {
            // Sent from inside the runtime: a request with a timeout starts its timer on `send`,
            // and a timer needs the runtime's clock.
            let response = self
                .build(&request)
                .send()
                .await
                .map_err(|_| TransportFailed)?;
            let status = response.status().as_u16();
            let body = response.text().await.map_err(|_| TransportFailed)?;
            Ok(HttpResponse { status, body })
        })
    }
}

#[cfg(test)]
#[path = "ai_transport_tests.rs"]
mod tests;
