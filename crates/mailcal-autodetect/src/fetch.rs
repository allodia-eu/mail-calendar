//! The HTTP fetcher shared by every lookup strategy.
//!
//! Redirects are **disabled in reqwest and followed by hand**, so each hop's scheme is
//! inspected: a result is trusted only when every hop was HTTPS. Requests are bounded
//! (per-request timeout, redirect-hop cap, response-size cap); a transport or TLS
//! failure is a silent skip, logged at debug without the local part. This fixes the
//! three sharp edges of Thunderbird's fetcher: no overall timeout, no size cap, and
//! automatic cross-scheme redirect following.

use reqwest::{
    Client,
    header::{LOCATION, WWW_AUTHENTICATE},
    redirect::Policy,
};
use url::Url;

use crate::{DetectConfig, types::DetectError};

/// A reusable HTTP client plus the per-run limits it enforces.
#[derive(Debug)]
pub(crate) struct Fetcher {
    client: Client,
    max_redirects: usize,
    max_body_bytes: usize,
}

/// The result of fetching one URL.
#[derive(Debug)]
pub(crate) enum FetchOutcome {
    /// A terminal (non-redirect) HTTP response was received; any status code.
    Response(FetchResponse),
    /// A response chain was reached but is unusable: too many redirects, a redirect
    /// with no `Location`, or a body over the size cap. Distinct from a transport
    /// failure, so it does not count toward the "everything failed → offline" signal.
    Miss,
    /// No HTTP response at all; DNS/connect/TLS failure or a timeout.
    NetworkError,
}

/// A terminal HTTP response, with just what the strategies need.
#[derive(Debug)]
pub(crate) struct FetchResponse {
    /// The final HTTP status code.
    pub status: u16,
    /// Whether the response carried a `WWW-Authenticate` header (the JMAP-probe signal).
    pub www_authenticate: bool,
    /// The response body, truncated at nothing; capped during read, never exceeding
    /// [`DetectConfig::max_body_bytes`].
    pub body: Vec<u8>,
    /// Whether every hop of the redirect chain was HTTPS.
    pub trusted: bool,
    /// The URL of this terminal response (after any redirects).
    pub final_url: Url,
}

impl FetchResponse {
    /// Whether the status is 2xx.
    pub(crate) fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

/// The fetch capability the strategies depend on. `Fetcher` is the real HTTP
/// implementation; tests inject a virtual-time fake so orchestration can be exercised
/// deterministically without real sockets.
#[async_trait::async_trait]
pub(crate) trait Fetch: Send + Sync {
    /// Fetches `url`, following redirects by hand, never erroring (see
    /// [`FetchOutcome`]).
    async fn get(&self, url: &Url) -> FetchOutcome;
}

#[async_trait::async_trait]
impl Fetch for Fetcher {
    async fn get(&self, url: &Url) -> FetchOutcome {
        Fetcher::get(self, url).await
    }
}

impl Fetcher {
    /// Builds the fetcher over the shared TLS trust store, with redirects disabled and
    /// both connect and total per-request timeouts set.
    ///
    /// # Errors
    ///
    /// Returns [`DetectError::Tls`] if the trust store or HTTP client cannot be built.
    pub(crate) fn new(config: &DetectConfig) -> Result<Self, DetectError> {
        let tls = engine_tls::client_config(&engine_tls::TlsPolicy::bundled_and_system())
            .map_err(|e| DetectError::Tls(e.to_string()))?;
        let client = tls
            .reqwest_builder()
            .redirect(Policy::none())
            .connect_timeout(config.http_timeout)
            .timeout(config.http_timeout)
            .build()
            .map_err(|e| DetectError::Tls(e.to_string()))?;
        Ok(Self {
            client,
            max_redirects: config.max_redirects,
            max_body_bytes: config.max_body_bytes,
        })
    }

    /// Fetches `url`, following up to `max_redirects` hops by hand and tracking whether
    /// every hop stayed on HTTPS. Never returns an error: a failure folds into
    /// [`FetchOutcome::NetworkError`] or [`FetchOutcome::Miss`].
    pub(crate) async fn get(&self, url: &Url) -> FetchOutcome {
        let mut current = url.clone();
        let mut trusted = true;

        for _hop in 0..=self.max_redirects {
            trusted &= current.scheme() == "https";
            let response = match self.client.get(current.clone()).send().await {
                Ok(response) => response,
                Err(err) => {
                    log::debug!("autodetect fetch failed for {current}: {err}");
                    return FetchOutcome::NetworkError;
                }
            };

            let status = response.status();
            if status.is_redirection() {
                let Some(next) = redirect_target(&current, &response) else {
                    log::debug!("autodetect redirect without a usable Location at {current}");
                    return FetchOutcome::Miss;
                };
                current = next;
                continue;
            }

            let www_authenticate = response.headers().contains_key(WWW_AUTHENTICATE);
            return match self.read_capped_body(response).await {
                Some(body) => FetchOutcome::Response(FetchResponse {
                    status: status.as_u16(),
                    www_authenticate,
                    body,
                    trusted,
                    final_url: current,
                }),
                None => FetchOutcome::Miss,
            };
        }

        log::debug!(
            "autodetect exceeded {} redirect hops for {url}",
            self.max_redirects
        );
        FetchOutcome::Miss
    }

    /// Reads the body, streaming until the size cap; returns `None` if the cap is
    /// exceeded or a chunk read fails.
    async fn read_capped_body(&self, mut response: reqwest::Response) -> Option<Vec<u8>> {
        let mut body = Vec::new();
        loop {
            match response.chunk().await {
                Ok(Some(chunk)) => {
                    if body.len() + chunk.len() > self.max_body_bytes {
                        log::debug!("autodetect response body exceeded the size cap");
                        return None;
                    }
                    body.extend_from_slice(&chunk);
                }
                Ok(None) => return Some(body),
                Err(err) => {
                    log::debug!("autodetect body read failed: {err}");
                    return None;
                }
            }
        }
    }
}

/// Resolves a redirect `response`'s `Location` against the `current` URL.
fn redirect_target(current: &Url, response: &reqwest::Response) -> Option<Url> {
    let location = response.headers().get(LOCATION)?.to_str().ok()?;
    current.join(location).ok()
}

#[cfg(test)]
#[path = "fetch_tests.rs"]
mod fetch_tests;
