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
//! the API pass, which can only start again from the top. Two answers are both:
//!
//! - A refused login: Gmail also refuses one for too many simultaneous connections, so only the
//!   same refusal [`REFUSALS_BEFORE_API`] times running moves the account.
//! - A `NO` while opening All Mail: a hidden All Mail and a server that cannot answer for now (RFC
//!   5530 `[UNAVAILABLE]`) reach this code as the same failure class, so the read asks again
//!   ([`ASKS_BEFORE_API`] times, a pause apart) and believes only an answer that does not change.
//!
//! A server without Gmail's extension is the one answer believed at once: it is a fact about the
//! session, not a reply that may differ next time.
//!
//! One slot serves every provider of an account's sign-in ([`GmailImapSlot::shared`]): each holds
//! a pool of connections, and Gmail counts an account's connections across all of its clients.

use std::{
    collections::HashMap,
    sync::{Arc, LazyLock, Weak},
};

use engine_core::error::FailureClass;
use engine_provider::ProviderResult;
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

/// How many times one read asks for All Mail before a `NO` moves the account to the API.
const ASKS_BEFORE_API: u32 = 3;

/// The pause before asking for All Mail again, grown by each ask.
const ASK_AGAIN_AFTER: std::time::Duration = std::time::Duration::from_secs(2);

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
    /// The slot of the account at `address` signed in with `tokens`: the one its other providers
    /// already hold, or a new one when none does.
    ///
    /// A sign-in made again gets a slot of its own even while the old one is alive. The account's
    /// old providers are replaced only once the new ones have connected, so they still hold the
    /// old slot then, and that slot dials every new IMAP connection with the grant it was built
    /// with: the one the person has just replaced, and usually the one that was refused.
    pub(super) fn shared(
        address: &str,
        tokens: &Arc<GraphTokenSource>,
        tls: &TlsClientConfig,
    ) -> Arc<Self> {
        let mut slots = SLOTS.lock().expect("gmail slots mutex poisoned");
        slots.retain(|_, slot| slot.strong_count() > 0);
        if let Some(slot) = slots.get(address).and_then(Weak::upgrade)
            && Arc::ptr_eq(&slot.tokens, tokens)
        {
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
        let result = settled(
            || open(&self.address, &self.tokens, &self.tls),
            ASK_AGAIN_AFTER,
        )
        .await;
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
    opened: ProviderResult<Opened>,
    refused: u8,
) -> (Side, ProviderResult<Option<GmailImap>>) {
    match opened {
        Ok(Opened::Reader(reader)) => (Side::Imap(reader.clone()), Ok(Some(reader))),
        Ok(Opened::NotGmail) => (Side::Api, Ok(None)),
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

/// What opening the IMAP side found.
enum Opened {
    /// A reader, with All Mail shown to IMAP.
    Reader(GmailImap),
    /// A session without Gmail's extension, which no second ask changes.
    NotGmail,
}

/// `open`'s answer, asked again while it is a `NO` (`InvalidState`) that may pass, up to
/// [`ASKS_BEFORE_API`] times with a pause that grows by `pause` each time. Every other answer is
/// returned as it came: one that may pass fails this read and is tried on the next.
async fn settled<T, F, Fut>(mut open: F, pause: std::time::Duration) -> ProviderResult<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = ProviderResult<T>>,
{
    let mut asked = 1;
    loop {
        match open().await {
            Err(err) if err.class() == FailureClass::InvalidState && asked < ASKS_BEFORE_API => {
                log::warn!(
                    "gmail: the IMAP side did not open All Mail ({asked} of {ASKS_BEFORE_API} \
                     asks); asking again"
                );
                tokio::time::sleep(pause * asked).await;
                asked += 1;
            }
            answer => return answer,
        }
    }
}

/// Logs in, and proves All Mail is shown to IMAP.
async fn open(
    address: &str,
    tokens: &Arc<GraphTokenSource>,
    tls: &TlsClientConfig,
) -> ProviderResult<Opened> {
    let credentials = Arc::new(OAuthCredentialSource::new(
        address.to_owned(),
        Arc::clone(tokens),
    ));
    let config =
        ImapConfig::from_credential_source(format!("{GMAIL_IMAP}:993"), GMAIL_IMAP, credentials)
            .with_connect_observer(connect_logger("imap"));
    let account = ImapAccount::connect(&config, tls.connector()).await?;
    let Some(reader) = account.gmail() else {
        return Ok(Opened::NotGmail);
    };
    reader.open(GmailFolder::All).await?;
    Ok(Opened::Reader(reader))
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, time::Duration};

    use engine_core::error::FailureClass;
    use engine_provider::ProviderError;

    use super::{
        ASKS_BEFORE_API, GmailImapSlot, Opened, REFUSALS_BEFORE_API, Side, decide, settled,
    };

    fn failed(class: FailureClass) -> engine_provider::ProviderResult<Opened> {
        Err(ProviderError::new(class, "no"))
    }

    #[test]
    fn a_no_that_outlasts_every_ask_moves_the_account_to_the_api() {
        let (side, answer) = decide(failed(FailureClass::InvalidState), 0);
        assert!(matches!(side, Side::Api));
        assert!(matches!(answer, Ok(None)));
    }

    #[test]
    fn a_server_without_the_gmail_extension_moves_it_at_once() {
        let (side, answer) = decide(Ok(Opened::NotGmail), 0);
        assert!(matches!(side, Side::Api));
        assert!(matches!(answer, Ok(None)));
    }

    /// `open`, answering each ask with the next of `answers`, and counting the asks.
    async fn ask(answers: &[Result<u8, FailureClass>]) -> (Result<u8, FailureClass>, usize) {
        let asked = Cell::new(0);
        let answer = settled(
            || {
                let next = answers[asked.get()];
                asked.set(asked.get() + 1);
                async move { next.map_err(|class| ProviderError::new(class, "no")) }
            },
            Duration::ZERO,
        )
        .await;
        (answer.map_err(|err| err.class()), asked.get())
    }

    #[tokio::test]
    async fn a_no_that_passes_is_asked_again_and_opens_the_imap_side() {
        let (answer, asked) = ask(&[Err(FailureClass::InvalidState), Ok(7)]).await;
        assert_eq!((answer, asked), (Ok(7), 2));
    }

    #[tokio::test]
    async fn a_no_that_does_not_change_is_believed_after_every_ask() {
        let answers = [Err(FailureClass::InvalidState); ASKS_BEFORE_API as usize];
        let (answer, asked) = ask(&answers).await;
        assert_eq!(answer, Err(FailureClass::InvalidState));
        assert_eq!(asked, answers.len());
    }

    #[tokio::test]
    async fn any_other_failure_fails_the_read_without_asking_again() {
        for class in [
            FailureClass::Authentication,
            FailureClass::Retryable,
            FailureClass::RateLimited,
            FailureClass::Permanent,
        ] {
            let (answer, asked) = ask(&[Err(class), Ok(7)]).await;
            assert_eq!((answer, asked), (Err(class), 1), "{class:?}");
        }
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

    /// A token source of its own, as each sign-in builds: signing in again builds a new one
    /// beside the old one, which stays alive until the account's old providers are replaced.
    fn sign_in() -> std::sync::Arc<crate::GraphTokenSource> {
        use mailcal_oauth::{GOOGLE_SCOPES, OAuthClient, OAuthProviderConfig};

        let provider = OAuthProviderConfig::google(
            "google-client",
            None,
            "com.googleusercontent.apps.google-client:/oauth2redirect",
            GOOGLE_SCOPES,
        );
        crate::GraphTokenSource::from_parts(
            OAuthClient::new(provider).unwrap(),
            engine_core::ids::AccountId::try_from("alice@gmail.com@mail.google.com").unwrap(),
            "refresh".to_owned(),
            None,
            "google",
            crate::CredentialOrigin::FreshSignIn,
        )
    }

    /// Signing in again replaces the account's providers only after the new ones have connected,
    /// so the old ones, and the slot they hold, are still alive when the new ones ask for theirs.
    /// A slot that kept the old sign-in would dial every new IMAP connection with a grant the
    /// user just replaced, which is the grant that was refused.
    #[test]
    fn signing_in_again_gives_the_account_a_slot_with_the_new_sign_in() {
        let tls = crate::tls::tls_with(&[]).unwrap();
        let address = "signed-in-again@gmail.com";
        let old = sign_in();
        let replaced = GmailImapSlot::shared(address, &old, &tls);

        let new = sign_in();
        let slot = GmailImapSlot::shared(address, &new, &tls);
        assert!(
            std::sync::Arc::ptr_eq(&slot.tokens, &new),
            "the slot dials IMAP with the sign-in it was given, not the one it replaced",
        );
        assert!(std::sync::Arc::ptr_eq(&replaced.tokens, &old));

        // The account's other providers, built from the same sign-in, share the new slot.
        let again = GmailImapSlot::shared(address, &new, &tls);
        assert!(std::sync::Arc::ptr_eq(&slot, &again));
    }
}
