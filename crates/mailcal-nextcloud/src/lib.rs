//! Nextcloud's own sign-in for native clients, [Login Flow v2], and what follows from the app
//! password it yields.
//!
//! Nextcloud signs in no client through OAuth that an installed app could use (its `oauth2` app
//! takes confidential clients only, without PKCE or registration), so this is the one route that
//! spares a person a hand-made app password, and the only one that works behind an
//! organisation's single sign-on. The flow:
//!
//! 1. [`begin`] asks the server for a login page and a poll token.
//! 2. The person signs in on that page in their browser, the way they always do.
//! 3. [`poll`] asks until the server hands back a login name and an app password, **once**.
//!
//! The app password has no expiry and nothing to refresh. It stops working when the person
//! revokes it in Nextcloud, when it goes unused for a year, when the account is disabled or its
//! stored password stops verifying, or when the person wipes this device; each of those is a DAV
//! `401`, answered by signing in again. [`revoke`] withdraws it when the account is removed, and
//! [`wipe_requested`] asks whether a refusal was a wipe.
//!
//! This crate is the protocol only. It knows nothing of accounts, storage or which surface asked.
//!
//! [Login Flow v2]: https://docs.nextcloud.com/server/latest/developer_manual/client_apis/LoginFlow/index.html#login-flow-v2

mod flow;
mod ocs;
mod root;
mod secret;
mod sign;
mod status;
mod wipe;

#[cfg(test)]
mod test_server;

use std::time::Duration;

pub use flow::{
    AppCredentials, FLOW_LIFETIME, FlowStart, POLL_INTERVAL, PendingFlow, Poll, begin, poll,
};
pub use ocs::{User, current_user, revoke};
pub use root::Root;
pub use secret::Secret;
pub use sign::{WELL_KNOWN_PATH, answers_as_nextcloud, nextcloud_root, serves_php};
pub use status::{STATUS_PATH, STATUS_TIMEOUT, is_nextcloud, is_nextcloud_status};
pub use wipe::{wipe_finished, wipe_requested};

/// How long one request of the flow, or of the calls that follow it, may take.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Why a request to the server did not do what it was for.
#[derive(Debug, thiserror::Error)]
pub enum NextcloudError {
    /// The TLS configuration could not be built.
    #[error("the TLS configuration could not be built")]
    Tls(#[source] engine_tls::TlsError),
    /// The request did not complete: no connection, a timeout, a certificate that did not verify.
    #[error("the request to the server did not complete")]
    Transport(#[source] reqwest::Error),
    /// The server answered with a status the call does not expect.
    #[error("the server answered {0}")]
    Status(u16),
    /// The server's answer was not the document the call expects.
    #[error("the server's answer could not be read")]
    Malformed,
    /// The server named an address on another server, or a scheme other than its own.
    #[error("the server named an address that is not its own")]
    ForeignUrl,
}

/// The HTTP client every call in this crate runs on, over `tls`: redirects off, because a
/// redirect would move a credential to a URL this crate did not choose.
///
/// # Errors
///
/// [`NextcloudError::Transport`] when the client cannot be built.
pub fn client(tls: &engine_tls::TlsClientConfig) -> Result<reqwest::Client, NextcloudError> {
    tls.reqwest_builder()
        .user_agent(mailcal_user_agent::HEADER)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(NextcloudError::Transport)
}

/// The most of a response body this crate reads. Every document it expects is a few hundred
/// bytes; a server sending more is not answering the question.
const MAX_BODY: usize = 64 * 1024;

/// `response`'s body, up to [`MAX_BODY`].
pub(crate) async fn read_body(mut response: reqwest::Response) -> Result<Vec<u8>, NextcloudError> {
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(NextcloudError::Transport)? {
        if body.len() + chunk.len() > MAX_BODY {
            return Err(NextcloudError::Malformed);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// [`client`] over `policy`, with no certificate exceptions: a sign-in opens the server's page in
/// a browser, which would meet the same certificate, so a server whose certificate needs an
/// exception is offered the password instead.
///
/// # Errors
///
/// [`NextcloudError::Tls`] when the policy cannot be built, else as [`client`].
pub fn client_for(policy: &engine_tls::TlsPolicy) -> Result<reqwest::Client, NextcloudError> {
    client(&engine_tls::client_config(policy).map_err(NextcloudError::Tls)?)
}
