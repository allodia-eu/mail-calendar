//! Whether a Microsoft or Google account is a personal one or an organisation's: asked through
//! the engine once, and stored with the account
//! ([`MicrosoftConfig::affiliation`](crate::MicrosoftConfig::affiliation),
//! [`GoogleConfig::affiliation`](crate::GoogleConfig::affiliation)).

use engine_api::{Affiliation, RetryConfig};
use provider_google::GoogleClient;
use provider_graph::GraphClient;

use super::GraphTokenSource;
use crate::{AccountError, log_handle::account_log_handle, tls::tls_with};

/// Asks Graph who the account `access_token` signs in as belongs to.
///
/// # Errors
///
/// Returns [`AccountError::Graph`] when Graph gave neither answer: an expired token, a throttled
/// or failed request. The account is then asked again at its next connect.
pub async fn graph_affiliation(
    access_token: String,
    retry: &RetryConfig,
) -> Result<Affiliation, AccountError> {
    let tls = tls_with(&[])?;
    let client = GraphClient::connect(access_token, &tls, retry)
        .map_err(|err| AccountError::Graph(err.to_string()))?;
    client
        .affiliation()
        .await
        .map_err(|err| AccountError::Graph(err.to_string()))
}

/// Asks Google who the account `access_token` signs in as belongs to. The token must hold
/// `userinfo.email`
/// ([`GoogleConfig::can_ask_affiliation`](crate::GoogleConfig::can_ask_affiliation)).
///
/// # Errors
///
/// Returns [`AccountError::Google`] when Google gave no answer.
pub async fn google_affiliation(
    access_token: String,
    retry: &RetryConfig,
) -> Result<Affiliation, AccountError> {
    let tls = tls_with(&[])?;
    let client = GoogleClient::connect(access_token, &tls, retry)
        .map_err(|err| AccountError::Google(err.to_string()))?;
    client
        .affiliation()
        .await
        .map_err(|err| AccountError::Google(err.to_string()))
}

impl GraphTokenSource {
    /// Asks the provider who this account belongs to and reports the answer to the host's sink,
    /// which stores it. For a Microsoft or Google account stored before its affiliation was
    /// recorded; a failure is logged and leaves it unknown, to be asked at the next connect.
    pub async fn detect_affiliation(&self) {
        let handle = account_log_handle(self.account.as_str());
        let answer = match self.access_token().await {
            Ok(token) if self.provider == "google" => {
                google_affiliation(token, &self.retry()).await
            }
            Ok(token) => graph_affiliation(token, &self.retry()).await,
            Err(err) => Err(err),
        };
        let provider = self.provider;
        match answer {
            Ok(affiliation) => {
                log::info!(
                    "{provider}: [{handle}] the account is {}",
                    match affiliation {
                        Affiliation::Personal => "a personal one",
                        Affiliation::Organization(_) => "an organisation's",
                    }
                );
                if let Some(sink) = &self.sink {
                    sink.affiliation_found(&self.account, &affiliation).await;
                }
            }
            Err(err) => log::warn!(
                "{provider}: [{handle}] could not ask whether the account is a personal one \
                 ({err}); asking again at the next connect"
            ),
        }
    }
}
