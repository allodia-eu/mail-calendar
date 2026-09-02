//! How far a **failed** token request got: the one question a caller must answer before
//! retrying a refresh.
//!
//! Its own module because the answer is a small, self-contained classification over
//! [`OAuthError`](crate::OAuthError) with consequences out of all proportion to its size: get it
//! wrong in one direction and an account waits a minute it did not need to; wrong in the other
//! and a replayed refresh token costs the user their whole grant.

use crate::OAuthError;

/// How far a **failed** token request got: whether the refresh token it presented may
/// already have been consumed by the server.
///
/// This is the one question a caller must answer before retrying a refresh, and no single
/// error message answers it. A server that rotates the refresh token on every refresh
/// invalidates the presented one the moment it answers: so a retry after a failure that
/// *did* reach the server presents a spent token. That is a replay, and a server that
/// treats a replay as theft (Fastmail: `invalid_grant; ratchet`) revokes the whole grant,
/// which costs the user a full re-authentication. A retry after a failure that provably
/// never left the device costs nothing.
///
/// The mapping is deliberately **asymmetric**: [`TokenRequestReach::NotSent`] is returned
/// only where it can be proven, and everything unclear is
/// [`TokenRequestReach::MaybeProcessed`]. Being needlessly cautious costs a delay; being
/// wrong in the other direction costs the account.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenRequestReach {
    /// The request provably never left this device: no connection was established, so no
    /// server saw the refresh token. Presenting it again is not a replay.
    NotSent,
    /// The request may have been processed. Either it was written to an established
    /// connection and the outcome is unknown, or the server demonstrably answered and we
    /// could not read the answer. The presented refresh token may be spent.
    MaybeProcessed,
}

impl OAuthError {
    /// Whether the failed request could have been processed by the server; see
    /// [`TokenRequestReach`]. Callers use this to decide whether re-presenting the same
    /// refresh token is safe or is a replay.
    #[must_use]
    pub fn reach(&self) -> TokenRequestReach {
        match self {
            // No request was ever built, so none was sent.
            Self::Tls(_) | Self::Callback(_) => TokenRequestReach::NotSent,
            // `is_connect` covers the failures that happen *before* a byte of the request
            // is written: DNS (the backgrounded-Android `EAI_NODATA` storm), a refused or
            // unreachable peer, and a failed TLS handshake. `is_builder` never reached the
            // network at all. Everything else on the send path: a write that failed
            // mid-flight, a read timeout waiting for the status line, is unknowable from
            // here, so it is not claimed as safe.
            Self::Transport(err) => {
                if err.is_connect() || err.is_builder() {
                    TokenRequestReach::NotSent
                } else {
                    TokenRequestReach::MaybeProcessed
                }
            }
            // The server answered in every one of these: `ResponseLost` and `Decode` had a
            // body (a `Decode` on a 2xx means the rotation was in a body we could not
            // parse), and `Endpoint` is the server's own words. A 4xx rejection probably
            // did not consume the token and a 5xx may well have, but both arrive here as
            // `Endpoint`, and guessing between them is exactly what this type refuses to
            // do. `invalid_grant` is the one case a caller handles before asking.
            Self::ResponseLost(_) | Self::Decode(_) | Self::Endpoint { .. } => {
                TokenRequestReach::MaybeProcessed
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;

    use crate::{AuthStyle, OAuthClient, OAuthError, OAuthProviderConfig, TokenRequestReach};

    /// A client whose token endpoint is `endpoint`, for the reach tests below.
    fn client_for(endpoint: &str) -> OAuthClient {
        OAuthClient::new(OAuthProviderConfig {
            authorize_endpoint: "https://example/authorize".to_owned(),
            token_endpoint: endpoint.to_owned(),
            client_id: "client-abc".to_owned(),
            client_secret: None,
            redirect_uri: "eu.allodia.mailcal://oauth".to_owned(),
            scopes: vec!["offline_access".to_owned()],
            resource: None,
            expected_issuer: None,
            style: AuthStyle::Microsoft,
        })
        .unwrap()
    }

    /// The reach tests below drive a **real** `reqwest` rather than hand-building errors,
    /// because the property under test is exactly how this version of `reqwest` reports
    /// each failure. `reach()` reading `is_connect()` is an assumption about a dependency,
    /// and an assumption a test cannot falsify is a comment.
    ///
    /// This is the case that matters on Android: a backgrounded app's uid loses network
    /// access without the device ever losing its network, so `getaddrinfo` fails with
    /// `EAI_NODATA`. It happened 227 times in five days on one production device, and every
    /// one of them must be freely retryable; if these were classified as possibly-processed
    /// the account would park itself for hours over failures that never left the phone.
    #[tokio::test]
    async fn a_dns_failure_provably_never_left_the_device() {
        // `.invalid` is reserved by RFC 2606 and must never resolve.
        let client = client_for("https://allodia-token-endpoint.invalid/token");
        let err = client
            .refresh("refresh-abc", OffsetDateTime::UNIX_EPOCH)
            .await
            .unwrap_err();
        assert_eq!(
            err.reach(),
            TokenRequestReach::NotSent,
            "a name that cannot resolve reached no server: {err}",
        );
    }

    #[tokio::test]
    async fn a_refused_connection_provably_never_left_the_device() {
        // Bind to claim a port, then drop the listener so nothing is accepting on it.
        let addr = {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            listener.local_addr().unwrap()
        };
        let client = client_for(&format!("http://{addr}/token"));
        let err = client
            .refresh("refresh-abc", OffsetDateTime::UNIX_EPOCH)
            .await
            .unwrap_err();
        assert_eq!(
            err.reach(),
            TokenRequestReach::NotSent,
            "a refused connection reached no server: {err}",
        );
    }

    /// The dangerous one, and the reason `ResponseLost` exists: the server answered, so its
    /// handler ran and rotated the refresh token, and then the body died on the way back.
    /// Nothing in the error text distinguishes this from a failure to connect, and the
    /// consequences are opposite.
    #[tokio::test]
    async fn a_response_that_dies_mid_body_may_already_have_been_processed() {
        use std::io::{Read, Write};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 8192];
                let _ = stream.read(&mut buf);
                // Promise 500 bytes, send 4, hang up. The status line arrives, the body
                // never does.
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 500\r\n\r\n{\"a\"",
                );
            }
        });
        let client = client_for(&format!("http://{addr}/token"));
        let err = client
            .refresh("refresh-abc", OffsetDateTime::UNIX_EPOCH)
            .await
            .unwrap_err();
        assert!(
            matches!(err, OAuthError::ResponseLost(_)),
            "a truncated body must not be reported as a transport failure: {err:?}",
        );
        assert_eq!(err.reach(), TokenRequestReach::MaybeProcessed);
    }

    #[test]
    fn a_server_side_error_may_already_have_been_processed() {
        // A 5xx arrives as `Endpoint` (there is no OAuth error body to parse), and a server
        // that failed *after* rotating looks exactly like one that failed before.
        let err = OAuthError::Endpoint {
            error: "http 500".to_owned(),
            description: None,
        };
        assert_eq!(err.reach(), TokenRequestReach::MaybeProcessed);
        // A 2xx body we could not parse held the rotation we needed.
        assert_eq!(
            OAuthError::Decode("expected value".to_owned()).reach(),
            TokenRequestReach::MaybeProcessed,
        );
    }
}
