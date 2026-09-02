//! What an IMAP account presents to its servers: a stored password, or an access token minted
//! from the account's OAuth grant for each connection.
//!
//! The engine asks a [`CredentialSource`] once per IMAP connection it dials and once per SMTP
//! submission, and once more after a refusal. A password account answers with the same
//! [`Credentials`] every time; an OAuth account answers through [`OAuthCredentialSource`], so a
//! connection dialled an hour after the account connected presents a token that is valid then.

use std::sync::Arc;

use async_trait::async_trait;
use engine_core::error::FailureClass;
use engine_provider::ProviderError;
use provider_imap::{CredentialSource, Credentials};

use crate::{AccountConfig, AccountError, GraphTokenSource};

/// The token source an **OAuth** IMAP account dials with; `None` for a password account.
///
/// Deliberately not folded into [`AccountConfig`]. The source holds live process state (a
/// cached access token, and the refresh single-flight every provider of this account shares)
/// while the config is inert data a host reads out of its keystore. One is rebuilt each
/// launch; the other is not.
pub type ImapTokens<'a> = Option<&'a Arc<GraphTokenSource>>;

/// What [`AccountError::MissingCredential`] says when an OAuth account arrives without its
/// token source.
pub(crate) const NO_TOKEN_SOURCE: &str =
    "the account signs in with OAuth but was connected without its token source";

/// The credential source for `account`'s IMAP and SMTP connections: its password, or an
/// [`OAuthCredentialSource`] over `tokens` when it signs in with OAuth.
///
/// # Errors
///
/// Returns [`AccountError::MissingCredential`] when the account signs in with OAuth but no
/// token source was given, or stores neither a password nor a grant.
pub fn imap_credential_source(
    account: &AccountConfig,
    tokens: ImapTokens<'_>,
) -> Result<Arc<dyn CredentialSource>, AccountError> {
    if account.is_oauth() {
        let tokens = tokens.ok_or(AccountError::MissingCredential(NO_TOKEN_SOURCE))?;
        return Ok(Arc::new(OAuthCredentialSource {
            username: account.imap.username.clone(),
            tokens: Arc::clone(tokens),
        }));
    }
    let password = account
        .imap_password_credentials()
        .ok_or(AccountError::MissingCredential(
            "the account stores neither a password nor a grant",
        ))?;
    Ok(Arc::new(password))
}

/// An OAuth account's IMAP and SMTP credential: the mailbox name plus an access token from the
/// account's shared token source, which every provider of the account refreshes through.
#[derive(Debug)]
pub struct OAuthCredentialSource {
    /// The address the token was issued for, carried as the SASL `authzid`.
    username: String,
    /// The account's one token source.
    tokens: Arc<GraphTokenSource>,
}

#[async_trait]
impl CredentialSource for OAuthCredentialSource {
    async fn credentials(&self) -> Result<Credentials, ProviderError> {
        let token = self
            .tokens
            .access_token()
            .await
            .map_err(|err| failure(&err))?;
        Ok(Credentials::oauth2(self.username.clone(), token))
    }

    /// Refreshes even when the cached token has not reached its stated expiry, because the
    /// server has just refused it. A password is never renewed: this source never holds one.
    async fn renew(&self, refused: &Credentials) -> Result<Option<Credentials>, ProviderError> {
        let Credentials::OAuth2 { access_token, .. } = refused else {
            return Ok(None);
        };
        let token = self
            .tokens
            .access_token_replacing(access_token)
            .await
            .map_err(|err| failure(&err))?;
        Ok(Some(Credentials::oauth2(self.username.clone(), token)))
    }
}

/// Reports a token that could not be minted as a provider failure of the right class.
///
/// The class decides what the app does next. A revoked or expired grant is
/// [`Authentication`](FailureClass::Authentication), which becomes "sign in again"; a refresh
/// that could not reach the token endpoint is [`Retryable`](FailureClass::Retryable) and must
/// not prompt anybody, because nothing about the account has changed.
fn failure(err: &AccountError) -> ProviderError {
    let class = match err {
        AccountError::SigninRejected(_) => FailureClass::Authentication,
        _ => FailureClass::Retryable,
    };
    ProviderError::new(class, err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::test_support::{
        dead_grant_token_endpoint, ratcheting_token_endpoint, source_at,
    };

    fn oauth_source(tokens: Arc<GraphTokenSource>) -> OAuthCredentialSource {
        OAuthCredentialSource {
            username: "you@example.net".to_owned(),
            tokens,
        }
    }

    fn token(credentials: &Credentials) -> &str {
        match credentials {
            Credentials::OAuth2 { access_token, .. } => access_token,
            _ => panic!("expected a token credential"),
        }
    }

    #[tokio::test]
    async fn a_dial_presents_the_current_token_under_the_mailbox_name() {
        let (endpoint, _) = ratcheting_token_endpoint("initial-refresh");
        let source = oauth_source(source_at(endpoint, None));

        let credentials = source.credentials().await.unwrap();
        assert_eq!(credentials.username(), "you@example.net");
        assert_eq!(token(&credentials), "AT-1");
    }

    /// The server's refusal is the one piece of news the cached expiry cannot know, so a
    /// renewal refreshes although the token has most of its hour left.
    #[tokio::test]
    async fn a_refused_token_is_renewed_with_a_fresh_one() {
        let (endpoint, hits) = ratcheting_token_endpoint("initial-refresh");
        let source = oauth_source(source_at(endpoint, None));

        let refused = source.credentials().await.unwrap();
        let renewed = source
            .renew(&refused)
            .await
            .unwrap()
            .expect("a replacement");
        assert_eq!(token(&renewed), "AT-2");
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn a_password_is_never_renewed() {
        let (endpoint, _) = ratcheting_token_endpoint("initial-refresh");
        let source = oauth_source(source_at(endpoint, None));

        let password = Credentials::password("you@example.net", "hunter2");
        assert!(source.renew(&password).await.unwrap().is_none());
    }

    /// A dead grant must become "sign in again", never a retry: retrying presents a refresh
    /// token the server has already refused.
    #[tokio::test]
    async fn a_dead_grant_is_an_authentication_failure() {
        let (endpoint, _) = dead_grant_token_endpoint();
        let source = oauth_source(source_at(endpoint, None));

        let err = source.credentials().await.expect_err("the grant is dead");
        assert_eq!(err.class(), FailureClass::Authentication);
    }

    #[test]
    fn an_oauth_account_without_its_token_source_is_refused() {
        let account: AccountConfig = toml::from_str(
            "[imap]\naddr=\"h:993\"\nserver_name=\"h\"\nusername=\"u\"\n\
             [oauth]\nclient_id=\"c\"\nrefresh_token=\"r\"\n\
             authorize_endpoint=\"https://h/a\"\ntoken_endpoint=\"https://h/t\"\n\
             redirect_uri=\"x://y\"\nscopes=[]\n",
        )
        .expect("valid config");
        assert!(matches!(
            imap_credential_source(&account, None),
            Err(AccountError::MissingCredential(NO_TOKEN_SOURCE))
        ));
    }
}
