//! Whether a Microsoft account is a personal one or an organisation's: asked of Graph once,
//! through the engine, and stored with the account
//! ([`MicrosoftConfig::affiliation`](crate::MicrosoftConfig::affiliation)).

use engine_api::{Affiliation, RetryConfig};
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

impl GraphTokenSource {
    /// Asks Graph who this account belongs to and reports the answer to the host's sink, which
    /// stores it. For an account stored before its affiliation was recorded; a failure is logged
    /// and leaves it unknown, to be asked at the next connect.
    pub async fn detect_affiliation(&self) {
        let handle = account_log_handle(self.account.as_str());
        let answer = match self.access_token().await {
            Ok(token) => graph_affiliation(token, &self.retry()).await,
            Err(err) => Err(err),
        };
        match answer {
            Ok(affiliation) => {
                log::info!(
                    "graph: [{handle}] the account is {}",
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
                "graph: [{handle}] could not ask whether the account is a personal one ({err}); \
                 asking again at the next connect"
            ),
        }
    }
}
