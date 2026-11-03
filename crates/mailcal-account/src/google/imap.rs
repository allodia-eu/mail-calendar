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
//! refuses one for too many simultaneous connections, so only the [`REFUSALS_BEFORE_API`]th
//! refused login before the side opens moves the account. A failure of another kind in between
//! neither counts nor clears the count.
//!
//! A lasting `NO` while opening All Mail, and a server without Gmail's extension, are believed at
//! once: each is a fact about the account, not a reply that may differ next time. A refusal Gmail
//! calls temporary reaches this code as a failure that may pass, and fails the read.
//!
//! One slot serves every provider of an account's sign-in ([`GmailImapSlot::shared`]): each holds
//! a pool of connections, and Gmail counts an account's connections across all of its clients.

use std::{
    collections::HashMap,
    sync::{
        Arc, LazyLock, Weak,
        atomic::{AtomicU8, Ordering},
    },
};

use engine_core::error::FailureClass;
use engine_provider::{ProviderError, ProviderResult};
use engine_tls::TlsClientConfig;
use provider_imap::{GmailFolder, GmailMail, ImapAccount, ImapConfig, ImapError};
use tokio::{net::TcpStream, sync::Mutex};
use tokio_rustls::client::TlsStream;

use crate::{
    GraphTokenSource, connect_log::connect_logger, imap_credentials::OAuthCredentialSource,
};

/// Gmail's IMAP server, for every Gmail and Workspace account alike.
const GMAIL_IMAP: &str = "imap.gmail.com";

/// How many refused logins, before the IMAP side has opened, move an account to the API.
const REFUSALS_BEFORE_API: u8 = 3;

/// Every live slot, by account address, so the providers of one account share one.
static SLOTS: LazyLock<std::sync::Mutex<HashMap<String, Weak<GmailImapSlot>>>> =
    LazyLock::new(Default::default);

/// What hears that an account's route changed; it answers whether it still wants to hear.
type RouteListener = Box<dyn Fn() -> bool + Send + Sync>;

/// Called whenever an account's route changes, so Settings can draw it again: one per core in
/// the process, since each draws its own Settings.
static ROUTE_LISTENERS: LazyLock<std::sync::Mutex<Vec<RouteListener>>> =
    LazyLock::new(Default::default);

/// Which way a Google account's mail is read in this launch, as Settings shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GmailMailRoute {
    /// Not settled: the account has not been read in this launch, or its IMAP side is being tried
    /// again after a failure that may pass.
    Undecided,
    /// Over IMAP.
    Imap,
    /// Over the API, because Gmail refused this account IMAP.
    Api,
    /// Not at all: two mail reads in a row could not reach Gmail's IMAP side,
    /// while the account is not on the API. A network that lets HTTPS through and blocks IMAP
    /// looks exactly like this, and the account's API calls keep succeeding meanwhile, so nothing
    /// else would say that its mail has stopped.
    Unreachable,
}

/// How many mail reads in a row that could not reach Gmail's IMAP side make an account
/// [`GmailMailRoute::Unreachable`]. One is a blip a laptop has on every wake.
const UNREACHED_READS: u8 = 2;

/// Which way the Google account at `address` reads its mail in this launch.
#[must_use]
pub fn gmail_mail_route(address: &str) -> GmailMailRoute {
    let slot = SLOTS
        .lock()
        .expect("gmail slots mutex poisoned")
        .get(address)
        .and_then(Weak::upgrade);
    slot.map_or(GmailMailRoute::Undecided, |slot| slot.route())
}

/// Calls `listener` whenever a Google account's route changes, until it answers `false`.
pub fn on_gmail_mail_route_change(listener: impl Fn() -> bool + Send + Sync + 'static) {
    ROUTE_LISTENERS
        .lock()
        .expect("gmail route listeners mutex poisoned")
        .push(Box::new(listener));
}

/// Tells every listener that a route changed, and forgets the ones that no longer want to hear.
fn route_changed() {
    ROUTE_LISTENERS
        .lock()
        .expect("gmail route listeners mutex poisoned")
        .retain(|listener| listener());
}

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
    /// Which side `state` has settled on, as [`GmailMailRoute`]'s first three, kept beside it
    /// because that lock is held across a sign-in and Settings may not wait for one.
    route: AtomicU8,
    /// Mail reads in a row that could not reach the IMAP side ([`Self::note_read`]).
    unreached: AtomicU8,
}

/// Which side the account's mail is read over.
#[derive(Debug, Clone)]
enum Side {
    /// Not asked yet, or last asked during a failure that may pass, after this many refused
    /// logins.
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
            route: AtomicU8::new(0),
            unreached: AtomicU8::new(0),
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
        let settled = match next {
            Side::Unknown(_) => None,
            Side::Imap(_) => Some(GmailMailRoute::Imap),
            Side::Api => Some(GmailMailRoute::Api),
        };
        *state = next;
        if let Some(route) = settled {
            let before = self.route();
            self.route.store(route as u8, Ordering::Release);
            if self.route() != before {
                route_changed();
            }
        }
        answer
    }

    /// Records how a mail read ended. A read that could not reach the server ([`unreached`])
    /// counts towards [`GmailMailRoute::Unreachable`]; anything else ends the run, since the
    /// server answered. Reads over the API count for nothing here: their outage is the account's
    /// own.
    pub(super) fn note_read(&self, ended: Result<(), &ProviderError>) {
        let before = self.route();
        if before == GmailMailRoute::Api {
            return;
        }
        match ended {
            Err(err) if unreached(err) => {
                let _ = self
                    .unreached
                    .try_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                        Some(n.saturating_add(1))
                    });
            }
            _ => self.unreached.store(0, Ordering::Release),
        }
        if self.route() != before {
            route_changed();
        }
    }

    fn route(&self) -> GmailMailRoute {
        let settled = match self.route.load(Ordering::Acquire) {
            1 => GmailMailRoute::Imap,
            2 => GmailMailRoute::Api,
            _ => GmailMailRoute::Undecided,
        };
        if settled != GmailMailRoute::Api
            && self.unreached.load(Ordering::Acquire) >= UNREACHED_READS
        {
            GmailMailRoute::Unreachable
        } else {
            settled
        }
    }
}

/// Whether a read that failed with `err` never reached the server: a failure that may pass, with
/// no IMAP `NO` behind it. A refusal the server calls temporary may pass as well, but the server
/// answered it.
fn unreached(err: &ProviderError) -> bool {
    err.class() == FailureClass::Retryable
        && !std::iter::successors(std::error::Error::source(err), |cause| cause.source())
            .any(|cause| matches!(cause.downcast_ref(), Some(ImapError::No(_))))
}

/// What an attempt to open the IMAP side leaves the slot at, after `refused` refused logins, and
/// what the caller is told. The session lacking what the reader needs (no Gmail
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
#[path = "imap_tests.rs"]
mod tests;
