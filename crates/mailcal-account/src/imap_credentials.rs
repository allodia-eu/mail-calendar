//! What an IMAP account presents to its servers: a stored password, or an access token minted
//! from the account's OAuth grant for each connection.
//!
//! The engine asks a [`CredentialSource`] once per IMAP connection it dials and once per SMTP
//! submission, and once more after a refusal. A password account answers with the same
//! [`FixedCredential`] every time; an OAuth account answers through its [`GraphTokenSource`],
//! the one source every adapter of the account shares, so a connection dialled an hour after the
//! account connected presents a token that is valid then.

use std::sync::Arc;

use engine_provider::{CredentialSource, CredentialSubject, FixedCredential};

use crate::{AccountConfig, AccountError, GraphTokenSource};

/// The token source an **OAuth** account's adapters ask, its mailbox, calendar, address book and
/// JMAP alike; `None` for an account that signs in with a password.
///
/// Deliberately not folded into [`AccountConfig`]. The source holds live process state (a
/// cached access token, and the refresh single-flight every provider of this account shares)
/// while the config is inert data a host reads out of its keystore. One is rebuilt each
/// launch; the other is not.
pub type AccountTokens<'a> = Option<&'a Arc<GraphTokenSource>>;

/// What [`AccountError::MissingCredential`] says when an OAuth account arrives without its
/// token source.
pub(crate) const NO_TOKEN_SOURCE: &str =
    "the account signs in with OAuth but was connected without its token source";

/// The credential source for `account`'s IMAP and SMTP connections: its password, or `tokens`
/// itself when it signs in with OAuth.
///
/// # Errors
///
/// Returns [`AccountError::MissingCredential`] when the account signs in with OAuth but no
/// token source was given, or stores neither a password nor a grant.
pub fn imap_credential_source(
    account: &AccountConfig,
    tokens: AccountTokens<'_>,
) -> Result<Arc<dyn CredentialSource>, AccountError> {
    if account.is_oauth() {
        let tokens = tokens.ok_or(AccountError::MissingCredential(NO_TOKEN_SOURCE))?;
        return Ok(Arc::clone(tokens) as Arc<dyn CredentialSource>);
    }
    let password = account
        .imap_password_credentials()
        .ok_or(AccountError::MissingCredential(
            "the account stores neither a password nor a grant",
        ))?;
    Ok(Arc::new(FixedCredential::new(
        account.credential_subject()?,
        password,
    )))
}

impl AccountConfig {
    /// Who the account signs in as, to the engine: its id, the subject an OAuth account's
    /// [`GraphTokenSource`] names too, so everything the engine keys by subject is the account's.
    ///
    /// # Errors
    ///
    /// Returns [`AccountError::MissingCredential`] when no id derives from the account.
    pub(crate) fn credential_subject(&self) -> Result<CredentialSubject, AccountError> {
        self.account_id()
            .map(|id| CredentialSubject::new(id.as_str()))
            .map_err(|_| AccountError::MissingCredential("the account has no id to sign in as"))
    }
}

#[cfg(test)]
#[path = "imap_credentials_tests.rs"]
mod tests;
