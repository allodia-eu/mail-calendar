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
//! that: it fails the read, to be tried again, because falling back would hand an IMAP cursor to
//! the API pass, which can only start again from the top. A refused login is both: Gmail also
//! refuses one for too many simultaneous connections, so only the same refusal
//! [`REFUSALS_BEFORE_API`] times running moves the account.
//!
//! One slot serves every provider of an account ([`GmailImapSlot::shared`]): each holds a pool of
//! connections, and Gmail counts an account's connections across all of its clients.

use std::{
    collections::HashMap,
    sync::{Arc, LazyLock, Weak},
};

use engine_core::error::FailureClass;
use engine_provider::{ProviderError, ProviderResult};
use engine_tls::TlsClientConfig;
use provider_imap::{GmailFolder, GmailMail, ImapAccount, ImapConfig};
use tokio::{net::TcpStream, sync::Mutex};
use tokio_rustls::client::TlsStream;

use crate::{
    GraphTokenSource, connect_log::connect_logger, imap_credentials::OAuthCredentialSource,
};

/// Gmail's IMAP server, for every Gmail and Workspace account alike.
const GMAIL_IMAP: &str = "imap.gmail.com";

/// How many refused logins in a row move an account to the API.
const REFUSALS_BEFORE_API: u8 = 3;

/// Every live slot, by account address, so the providers of one account share one.
static SLOTS: LazyLock<std::sync::Mutex<HashMap<String, Weak<GmailImapSlot>>>> =
    LazyLock::new(Default::default);

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
    /// Not asked yet, or last asked during a failure that may pass, after this many refused
    /// logins in a row.
    Unknown(u8),
    /// IMAP, through this reader.
    Imap(GmailImap),
    /// The API: Gmail refused this account IMAP.
    Api,
}

impl GmailImapSlot {
    /// The slot of the account at `address`: the one its other providers already hold, or a new
    /// one when none does.
    pub(super) fn shared(
        address: &str,
        tokens: &Arc<GraphTokenSource>,
        tls: &TlsClientConfig,
    ) -> Arc<Self> {
        let mut slots = SLOTS.lock().expect("gmail slots mutex poisoned");
        slots.retain(|_, slot| slot.strong_count() > 0);
        if let Some(slot) = slots.get(address).and_then(Weak::upgrade) {
            return slot;
        }
        let slot = Arc::new(Self {
            address: address.to_owned(),
            tokens: Arc::clone(tokens),
            tls: tls.clone(),
            state: Mutex::new(Side::Unknown(0)),
        });
        slots.insert(address.to_owned(), Arc::downgrade(&slot));
        slot
    }

    /// The account's IMAP reader, opening it the first time; `None` when Gmail refuses this
    /// account IMAP, which leaves its mail on the API.
    ///
    /// # Errors
    ///
    /// A failure that may pass: nothing is decided, and the next call tries again.
    pub(super) async fn reader(&self) -> ProviderResult<Option<GmailImap>> {
        let mut state = self.state.lock().await;
        let refused = match &*state {
            Side::Imap(reader) => return Ok(Some(reader.clone())),
            Side::Api => return Ok(None),
            Side::Unknown(refused) => *refused,
        };
        let result = open(&self.address, &self.tokens, &self.tls).await;
        let (next, answer) = decide(result, refused);
        match (&next, &answer) {
            (Side::Imap(_), _) => log::info!("gmail: reading mail over IMAP"),
            (Side::Api, _) => {
                log::info!("gmail: reading mail over the API, the IMAP side refused it");
            }
            (Side::Unknown(refusals), Err(err)) => log::warn!(
                "gmail: the IMAP side did not open ({:?}, {refusals} of {REFUSALS_BEFORE_API} \
                 refused sign-ins); the next read tries again",
                err.class(),
            ),
            (Side::Unknown(_), Ok(_)) => {}
        }
        *state = next;
        answer
    }
}

/// What an attempt to open the IMAP side leaves the slot at, after `refused` refused logins in a
/// row, and what the caller is told. The session lacking what the reader needs (no Gmail
/// extension, All Mail hidden) is final; a refused login is final only once it has repeated
/// (IMAP turned off for the domain, after the API took the same token); anything else passes.
fn decide(
    opened: ProviderResult<GmailImap>,
    refused: u8,
) -> (Side, ProviderResult<Option<GmailImap>>) {
    match opened {
        Ok(reader) => (Side::Imap(reader.clone()), Ok(Some(reader))),
        Err(err) => match err.class() {
            FailureClass::InvalidState => (Side::Api, Ok(None)),
            FailureClass::Authentication if refused + 1 >= REFUSALS_BEFORE_API => {
                (Side::Api, Ok(None))
            }
            FailureClass::Authentication => (Side::Unknown(refused + 1), Err(err)),
            _ => (Side::Unknown(refused), Err(err)),
        },
    }
}

/// Logs in, and proves All Mail is shown to IMAP.
async fn open(
    address: &str,
    tokens: &Arc<GraphTokenSource>,
    tls: &TlsClientConfig,
) -> ProviderResult<GmailImap> {
    let credentials = Arc::new(OAuthCredentialSource::new(
        address.to_owned(),
        Arc::clone(tokens),
    ));
    let config =
        ImapConfig::from_credential_source(format!("{GMAIL_IMAP}:993"), GMAIL_IMAP, credentials)
            .with_connect_observer(connect_logger("imap"));
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

    use super::{REFUSALS_BEFORE_API, Side, decide};

    fn failed(class: FailureClass) -> engine_provider::ProviderResult<super::GmailImap> {
        Err(ProviderError::new(class, "no"))
    }

    #[test]
    fn a_hidden_all_mail_moves_the_account_to_the_api_at_once() {
        let (side, answer) = decide(failed(FailureClass::InvalidState), 0);
        assert!(matches!(side, Side::Api));
        assert!(matches!(answer, Ok(None)));
    }

    #[test]
    fn a_refused_login_moves_it_only_once_the_refusal_has_repeated() {
        let mut refused = 0;
        for _ in 1..REFUSALS_BEFORE_API {
            let (side, answer) = decide(failed(FailureClass::Authentication), refused);
            let Side::Unknown(count) = side else {
                panic!("one refusal may be a connection limit, not IMAP turned off");
            };
            assert_eq!(answer.unwrap_err().class(), FailureClass::Authentication);
            refused = count;
        }
        let (side, answer) = decide(failed(FailureClass::Authentication), refused);
        assert!(matches!(side, Side::Api));
        assert!(matches!(answer, Ok(None)));
    }

    #[test]
    fn a_failure_that_may_pass_is_tried_again_and_counts_no_refusal() {
        for class in [
            FailureClass::Retryable,
            FailureClass::RateLimited,
            FailureClass::Permanent,
            FailureClass::Conflict,
        ] {
            let (side, answer) = decide(failed(class), 1);
            assert!(matches!(side, Side::Unknown(1)), "{class:?}");
            assert_eq!(answer.unwrap_err().class(), class);
        }
    }
}
