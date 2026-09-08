// SPDX-FileCopyrightText: 2026 Allodia
// SPDX-License-Identifier: LicenseRef-Allodia-1.0

//! Turning a stored grant back into an access token.
//!
//! Every call this crate makes needs a live access token, and what a device keeps between launches
//! is a refresh token. This is the piece in between, and it is deliberately **not** a [`SignIn`]:
//! that type can start a browser flow, and a caller holding one only to refresh could reach for
//! `begin` with the empty redirect URI a refresh grant has no use for. A type that cannot do the
//! thing it must not do beats a comment saying not to.
//!
//! **One per process, not one per refresh.** Discovery is two requests, and its answer does not
//! change between them; re-reading it on every refresh would put two more ways to fail in front of
//! the token a person is waiting on.
//!
//! [`SignIn`]: crate::SignIn

use mailcal_oauth::{OAuthClient, TokenSet};
use time::OffsetDateTime;

use crate::signin::{SignInError, discovered_client};

/// A grant that can be refreshed, and nothing else.
#[derive(Debug)]
pub struct Refresher {
    client: OAuthClient,
    end_session_endpoint: Option<String>,
}

impl Refresher {
    /// Read the service's metadata and build the client every later refresh runs on.
    ///
    /// # Errors
    /// [`SignInError::Unavailable`] when the build carries no registration;
    /// [`SignInError::Discovery`] when the service's metadata cannot be read.
    pub async fn discover() -> Result<Self, SignInError> {
        // A refresh grant carries no redirect URI (RFC 6749 §6), and this type has no path that
        // would put one on a request.
        let (client, metadata) = discovered_client("").await?;
        Ok(Self {
            client,
            end_session_endpoint: metadata.end_session_endpoint,
        })
    }

    /// Where to end the browser session, as discovery reported it just now.
    ///
    /// Kept rather than dropped, because a sign-out reads this from the **stored grant** so it can
    /// erase first and touch no network. That copy is written once, at sign-in, and the module note
    /// above only holds *within* a process: across launches the answer can move, and when the
    /// account service changed host it moved for everybody at once. Handing it out here is what
    /// lets a caller refresh the stored copy from an answer it already had to fetch anyway.
    ///
    /// `None` for a service advertising no endpoint, which is an answer rather than a missing one.
    #[must_use]
    pub fn end_session_endpoint(&self) -> Option<&str> {
        self.end_session_endpoint.as_deref()
    }

    /// What a sign-in against this service asks for.
    ///
    /// A refresh names no scope (RFC 6749 §6: an omitted one means the original grant), so a
    /// response that names none granted what was *originally* asked for. That set is this one for
    /// any grant made by this build, and the caller compares the two.
    #[must_use]
    pub fn requested_scopes(&self) -> &[String] {
        self.client.requested_scopes()
    }

    /// Exchange a refresh token for a fresh access token.
    ///
    /// The answer may carry a **rotated** refresh token, and a caller that does not store it has a
    /// grant the service has already moved past, which the next launch presents, and which a
    /// server that detects replay treats as theft.
    ///
    /// # Errors
    /// [`SignInError::OAuth`] when the service refuses the grant. A caller treats that as signed
    /// out rather than as an outage: an expired or revoked grant does not come back by waiting.
    pub async fn refresh(
        &self,
        refresh_token: &str,
        now: OffsetDateTime,
    ) -> Result<TokenSet, SignInError> {
        Ok(self.client.refresh(refresh_token, now).await?)
    }
}
