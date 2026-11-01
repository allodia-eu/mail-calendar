//! A Google account's mail read over IMAP rather than the Gmail API.
//!
//! The Gmail API bills each message's metadata and body against a per-user quota that cannot be
//! raised, which holds a first sync to about 270 messages a minute; IMAP has no such quota, and
//! reads the same messages under the same keys (`provider_google::GmailProvider::with_imap`). The
//! account's OAuth grant already covers it: `https://mail.google.com/` is the scope Gmail's IMAP
//! and SMTP accept for XOAUTH2.
//!
//! The API stays the fallback for an account whose IMAP side Gmail refuses: a Workspace domain
//! can turn IMAP off, which Gmail answers at login, and a user can hide All Mail from it. Any
//! other failure (the network, a throttle, a TLS failure behind a captive portal) is **not**
//! that: it fails the call, to be tried again, because falling back would hand an IMAP cursor to
//! the API pass, which can only start again from the top.

use std::sync::Arc;

use engine_core::error::FailureClass;
use engine_provider::{ProviderError, ProviderResult};
use engine_tls::TlsClientConfig;
use provider_imap::{GmailFolder, GmailMail, ImapAccount, ImapConfig};
use tokio::{net::TcpStream, sync::Mutex};
use tokio_rustls::client::TlsStream;

use crate::{GraphTokenSource, imap_credentials::OAuthCredentialSource};

/// Gmail's IMAP server, for every Gmail and Workspace account alike.
const GMAIL_IMAP: &str = "imap.gmail.com";

/// The reader a live account hands the Gmail provider.
pub(crate) type GmailImap = GmailMail<TlsStream<TcpStream>>;

/// The account's IMAP side, opened on first use and kept for the provider's life.
#[derive(Debug)]
pub(super) struct GmailImapSlot {
    /// The account's address, which XOAUTH2 names beside the token.
    address: String,
    tokens: Arc<GraphTokenSource>,
    tls: TlsClientConfig,
    state: Mutex<Side>,
}

/// Which side the account's mail is read over.
#[derive(Debug, Clone)]
enum Side {
    /// Not asked yet, or last asked during a failure that may pass.
    Unknown,
    /// IMAP, through this reader.
    Imap(GmailImap),
    /// The API: Gmail refused this account IMAP.
    Api,
}

impl GmailImapSlot {
    pub(super) fn new(address: String, tokens: Arc<GraphTokenSource>, tls: TlsClientConfig) -> Self {
        Self {
            address,
            tokens,
            tls,
            state: Mutex::new(Side::Unknown),
        }
    }

    /// The account's IMAP reader, opening it the first time; `None` when Gmail refuses this
    /// account IMAP, which leaves its mail on the API.
    ///
    /// # Errors
    ///
    /// A failure that may pass: nothing is decided, and the next call tries again.
    pub(super) async fn reader(&self) -> ProviderResult<Option<GmailImap>> {
        let mut state = self.state.lock().await;
        match &*state {
            Side::Imap(reader) => return Ok(Some(reader.clone())),
            Side::Api => return Ok(None),
            Side::Unknown => {}
        }
        match open(&self.address, &self.tokens, &self.tls).await {
            Ok(reader) => {
                log::info!("gmail: reading mail over IMAP");
                *state = Side::Imap(reader.clone());
                Ok(Some(reader))
            }
            Err(err) if !refuses_imap(&err) => Err(err),
            Err(err) => {
                log::info!(
                    "gmail: reading mail over the API, the IMAP side refused ({:?})",
                    err.class()
                );
                *state = Side::Api;
                Ok(None)
            }
        }
    }
}

/// Whether `err` says Gmail will not serve this account over IMAP: the login refused (IMAP
/// turned off for the domain, after the API took the same token), or the session lacking what
/// the reader needs (no Gmail extension, All Mail hidden from IMAP).
fn refuses_imap(err: &ProviderError) -> bool {
    matches!(
        err.class(),
        FailureClass::Authentication | FailureClass::InvalidState
    )
}

/// Logs in, and proves All Mail is shown to IMAP.
async fn open(
    address: &str,
    tokens: &Arc<GraphTokenSource>,
    tls: &TlsClientConfig,
) -> ProviderResult<GmailImap> {
    let credentials = Arc::new(OAuthCredentialSource::new(address.to_owned(), Arc::clone(tokens)));
    let config = ImapConfig::from_credential_source(format!("{GMAIL_IMAP}:993"), GMAIL_IMAP, credentials);
    let account = ImapAccount::connect(&config, tls.connector()).await?;
    let reader = account
        .gmail()
        .ok_or_else(|| ProviderError::invalid_state("the server offers no Gmail extension"))?;
    reader.open(GmailFolder::All).await?;
    Ok(reader)
}

#[cfg(test)]
mod tests {
    use engine_core::error::FailureClass;
    use engine_provider::ProviderError;

    use super::refuses_imap;

    #[test]
    fn only_a_refusal_of_imap_itself_moves_the_account_to_the_api() {
        for class in [FailureClass::Authentication, FailureClass::InvalidState] {
            assert!(refuses_imap(&ProviderError::new(class, "refused")), "{class:?}");
        }
        // Each of these may pass, and falling back on one would restart the sync from the top.
        for class in [
            FailureClass::Retryable,
            FailureClass::RateLimited,
            FailureClass::Permanent,
            FailureClass::Conflict,
            FailureClass::NeedsResync,
        ] {
            assert!(!refuses_imap(&ProviderError::new(class, "passing")), "{class:?}");
        }
    }
}
