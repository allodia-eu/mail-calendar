//! [`GraphTokenSource`] as the account's [`CredentialSource`]: the one source every adapter of an
//! OAuth account asks, on every dial, submission and request.
//!
//! It answers the account's current access token under the name the account signs in as, which
//! IMAP and SMTP SASL present beside it and HTTP ignores. A token a server refused is replaced
//! through the source's single-flight refresh, so the adapters refusing one token share one
//! refresh. Where a token may go is decided by each adapter's own host rules, and on a calendar
//! or address book by the account's grant (`crate::config_dav`), so `permits` keeps its default.

use async_trait::async_trait;
use engine_core::error::FailureClass;
use engine_provider::{
    Credential, CredentialRequest, CredentialSource, CredentialSubject, ProviderError,
};

use super::GraphTokenSource;
use crate::AccountError;

#[async_trait]
impl CredentialSource for GraphTokenSource {
    fn subject(&self) -> &CredentialSubject {
        &self.subject
    }

    async fn credential(
        &self,
        _request: &CredentialRequest<'_>,
    ) -> Result<Credential, ProviderError> {
        let token = self.access_token().await.map_err(|err| failure(&err))?;
        Ok(Credential::bearer_for(self.username.clone(), token))
    }

    /// Refreshes even when the cached token has not reached its stated expiry, because the
    /// server has just refused it. Anything but a token is never replaced: this source never
    /// answers one. A refresh that answers the refused token again is no replacement, since
    /// presenting it again would only be refused again.
    async fn replace(
        &self,
        refused: &Credential,
        _request: &CredentialRequest<'_>,
    ) -> Result<Option<Credential>, ProviderError> {
        let Credential::Bearer { token: refused, .. } = refused else {
            return Ok(None);
        };
        let token = self
            .access_token_replacing(refused.expose())
            .await
            .map_err(|err| failure(&err))?;
        if token == refused.expose() {
            return Ok(None);
        }
        Ok(Some(Credential::bearer_for(self.username.clone(), token)))
    }
}

/// Reports a token that could not be minted as a provider failure of the right class, with a
/// fixed detail: the token endpoint's own text stays in the refresh log line.
///
/// The class decides what the app does next. A revoked or expired grant is
/// [`Authentication`](FailureClass::Authentication), which becomes "sign in again"; a refresh
/// that could not reach the token endpoint is [`Retryable`](FailureClass::Retryable) and must
/// not prompt anybody, because nothing about the account has changed. The engine marks it as a
/// credential that could not be had (`ProviderError::is_credential_failure`), which a wrapper
/// tells apart from a transport failure (`crate::shared_client::should_reconnect`).
fn failure(err: &AccountError) -> ProviderError {
    match err {
        AccountError::SigninRejected(_) => ProviderError::new(
            FailureClass::Authentication,
            "the account's sign-in was refused when its access token was renewed",
        ),
        _ => ProviderError::new(
            FailureClass::Retryable,
            "the account's access token could not be renewed",
        ),
    }
}

#[cfg(test)]
#[path = "credential_tests.rs"]
mod tests;
