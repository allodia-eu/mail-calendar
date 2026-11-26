//! An OAuth account's token source as a calendar or address book sees it: the account's own
//! source, confined to the origins its grant's issuer vouches for (`docs/mail-oauth.md` rule 10).

use std::sync::Arc;

use async_trait::async_trait;
use engine_provider::{
    Credential, CredentialRequest, CredentialSource, CredentialSubject, Origin, ProviderError,
};

use crate::GraphTokenSource;

/// The account's token source, adding only the issuer's rule as [`permits`]: the engine asks it
/// for the configured server before anything is sent, and for every origin discovery moves to.
///
/// Everything else goes to the account's one source, so a token the calendar refuses is replaced
/// through the same single-flight refresh as one the mailbox refuses, and the account's other
/// adapters, which ask that source directly, are not narrowed by a rule that is about DAV.
///
/// [`permits`]: CredentialSource::permits
pub(crate) struct IssuerBound {
    pub(super) tokens: Arc<GraphTokenSource>,
    pub(super) issuer: String,
    pub(super) protected_resources: Vec<String>,
}

/// Names the account by its log handle ([`GraphTokenSource`]'s own `Debug`) and the issuer's list
/// by its length: a calendar's config prints its source, and a support log may carry a config.
impl core::fmt::Debug for IssuerBound {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("IssuerBound")
            .field("tokens", &self.tokens)
            .field("protected_resources", &self.protected_resources.len())
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl CredentialSource for IssuerBound {
    fn subject(&self) -> &CredentialSubject {
        self.tokens.subject()
    }

    /// Whether the issuer's tokens are for `origin` (§3.6): an entry of its
    /// `protected_resources` with the same scheme, host and port, or, when it lists none, a host
    /// on the issuer's registrable domain. The path is not part of an origin, so the configured
    /// URL's is held to the list before the first request (`grant_reaches`).
    fn permits(&self, origin: &Origin) -> bool {
        mailcal_oauth::token_may_reach_origin(
            &self.issuer,
            &self.protected_resources,
            &origin_url(origin),
        )
    }

    async fn credential(
        &self,
        request: &CredentialRequest<'_>,
    ) -> Result<Credential, ProviderError> {
        self.tokens.credential(request).await
    }

    async fn replace(
        &self,
        refused: &Credential,
        request: &CredentialRequest<'_>,
    ) -> Result<Option<Credential>, ProviderError> {
        self.tokens.replace(refused, request).await
    }
}

/// `origin` as the URL `scheme://host:port`, an IPv6 host in the brackets a URL puts around it.
fn origin_url(origin: &Origin) -> String {
    let host = origin.host();
    if host.contains(':') {
        format!("{}://[{host}]:{}", origin.scheme(), origin.port())
    } else {
        format!("{}://{host}:{}", origin.scheme(), origin.port())
    }
}

#[cfg(test)]
#[path = "issuer_bound_tests.rs"]
mod tests;
