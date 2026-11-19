//! The socket under the writing assistant: `mailcal-ai`'s [`HttpTransport`] over the app's own TLS
//! policy and runtime.
//!
//! Built from [`mailcal_oauth::discovery_client`] for the reason the Allodia account transport is:
//! `reqwest` is pinned to `rustls-no-provider` workspace-wide, so a client built any other way has
//! no crypto provider and fails at its first request. The port is synchronous and the client is
//! not; [`crate::runtime::drive`] polls each request on a thread of its own.
//!
//! Each request carries the timeout its purpose allows, so a model that never answers ends as no
//! answer rather than as a wait without end, and one whose work was stopped is abandoned at once,
//! which closes its connection.

use std::time::Duration;

use mailcal_ai::{HttpRequest, HttpResponse, HttpTransport, TransportFailed};
use mailcal_app::ai::AiCancel;

/// How often a request in flight looks whether its work was stopped.
const STOP_POLL: Duration = Duration::from_millis(100);

/// The assistant's requests over the app's own TLS policy and runtime.
pub(crate) struct AiTransport {
    http: reqwest::Client,
    handle: tokio::runtime::Handle,
    cancel: AiCancel,
}

impl std::fmt::Debug for AiTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AiTransport")
    }
}

impl AiTransport {
    /// One on `handle`'s runtime whose requests are abandoned once `cancel` is set.
    ///
    /// # Errors
    ///
    /// Returns the shared TLS policy's message when a client cannot be built.
    pub(crate) fn new(handle: tokio::runtime::Handle, cancel: AiCancel) -> Result<Self, String> {
        let http = mailcal_oauth::discovery_client().map_err(|error| error.to_string())?;
        Ok(Self {
            http,
            handle,
            cancel,
        })
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
        if self.cancel.is_set() {
            return Err(TransportFailed);
        }
        let builder = self.build(&request);
        let cancel = self.cancel.clone();
        crate::runtime::drive(&self.handle, move || async move {
            // Sent from inside the runtime: a request's timeout starts on `send`, and its timer
            // needs the runtime's clock.
            let answer = async {
                let response = builder.send().await.map_err(|_| TransportFailed)?;
                let status = response.status().as_u16();
                let body = response.text().await.map_err(|_| TransportFailed)?;
                Ok(HttpResponse { status, body })
            };
            let stopped = async {
                while !cancel.is_set() {
                    tokio::time::sleep(STOP_POLL).await;
                }
            };
            match futures::future::select(std::pin::pin!(answer), std::pin::pin!(stopped)).await {
                futures::future::Either::Left((answer, _)) => answer,
                futures::future::Either::Right(((), _)) => Err(TransportFailed),
            }
        })
    }
}

#[cfg(test)]
#[path = "ai_transport_tests.rs"]
mod tests;
