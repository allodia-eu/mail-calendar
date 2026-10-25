//! Binding an account's contact-source adapters at boot, add, and reconnect.
//!
//! Every helper answers with the providers it bound or the failure it met, and the dial decides
//! what that failure costs (`account_registry::dial_parts`): beside working mail
//! it only empties the Contacts list, while for an account without mail it may be the only thing
//! that can say the server is unreachable or the sign-in has expired.
//!
//! # Why every helper carries a deadline
//!
//! Discovery runs on the path that produces the user's **mailbox**, so its worst case is the
//! mailbox's worst case. Without a bound, a CalDAV host that accepts the connection and then
//! blackholes the address-book `PROPFIND` holds up mail: for a feature the account may not
//! even serve. The deadline is what makes the *latency* of a failure bounded too.
//!
//! # Why every exit logs, including the boring ones
//!
//! Because "Contacts is empty" has five causes here and only one of them is an error: the
//! account has no CalDAV endpoint to derive contacts from, the JMAP session does not advertise
//! contacts, discovery failed, discovery timed out, or it succeeded and the account genuinely
//! has no address book. Each exit names itself, and the success path says how many sources it
//! bound; a count that is the difference between "we found nothing" and "we found books that are
//! empty".

use std::{
    future::Future,
    sync::Arc,
    time::{Duration, Instant},
};

use engine_api::{AccountId, ContactsProvider, Provider};
use mailcal_account::{AccountConfig, AccountError, GraphTokenSource, JmapAccountConfig};

use crate::account_registry::ConnectFailure;

/// How long contact-source discovery may hold up an account's connect before it is abandoned.
///
/// Generous enough for a real `PROPFIND` over a slow mobile link, short enough that a
/// blackholed host costs the user a pause rather than a launch.
const DISCOVERY_DEADLINE: Duration = Duration::from_secs(10);

/// The same bound for an account not used for mail, which has no mailbox to hold up: its contacts
/// are one of the only things it connects, so a slow server is given time to answer rather than
/// failing the account. Still bounded, because a dial holds one of a few slots at launch.
const NO_MAILBOX_DEADLINE: Duration = Duration::from_secs(30);

/// What a contacts binding answers: the sources it bound, or why it bound none.
type Bound = Result<Vec<Box<dyn ContactsProvider>>, ConnectFailure>;

/// Runs one family's discovery under [`DISCOVERY_DEADLINE`], logging how it ended under `family`.
async fn bounded(
    family: &str,
    discovery: impl Future<Output = Result<Vec<Box<dyn ContactsProvider>>, AccountError>>,
) -> Bound {
    bounded_by(DISCOVERY_DEADLINE, family, discovery).await
}

/// How long an account's contacts discovery may take: [`DISCOVERY_DEADLINE`] when a mailbox waits
/// on the same connect, [`NO_MAILBOX_DEADLINE`] when none does.
pub(crate) const fn discovery_deadline(used_for_mail: bool) -> Duration {
    if used_for_mail {
        DISCOVERY_DEADLINE
    } else {
        NO_MAILBOX_DEADLINE
    }
}

/// [`bounded`] under `deadline`.
async fn bounded_by(
    deadline: Duration,
    family: &str,
    discovery: impl Future<Output = Result<Vec<Box<dyn ContactsProvider>>, AccountError>>,
) -> Bound {
    let started = Instant::now();
    match tokio::time::timeout(deadline, discovery).await {
        Ok(Ok(providers)) => {
            log::info!(
                "{family}: bound {} contact source(s) in {}ms",
                providers.len(),
                started.elapsed().as_millis(),
            );
            Ok(providers)
        }
        Ok(Err(err)) => {
            log::warn!(
                "{family}: contacts connect failed after {}ms: {err}",
                started.elapsed().as_millis(),
            );
            Err(ConnectFailure::from(err))
        }
        Err(_) => {
            log::warn!(
                "{family}: contacts discovery timed out after {}s",
                deadline.as_secs(),
            );
            Err(ConnectFailure::unreachable(format!(
                "{family}: contacts discovery timed out after {}s",
                deadline.as_secs(),
            )))
        }
    }
}

/// Binds the CardDAV contact adapters for a standards account; one per discovered address
/// book, or none when the account has neither a `[carddav]` section nor a `[caldav]` one to
/// derive the endpoint from.
///
/// Without a section of their own, contacts reuse the CalDAV origin and credentials (see
/// `mailcal_account::contacts`), so an account set up before this feature existed gains contacts
/// with no re-entry of anything.
/// `used_for_mail` decides how long discovery may take ([`discovery_deadline`]), and `learned`
/// receives what the connect said about where the next one starts.
pub(crate) async fn connect_caldav_contacts(
    config: &AccountConfig,
    tokens: mailcal_account::ImapTokens<'_>,
    used_for_mail: bool,
    learned: &std::sync::Mutex<mailcal_account::Learned>,
) -> Bound {
    if config.caldav.is_none() && config.carddav.is_none() {
        log::info!("carddav: contacts skipped; account has no address book server");
        return Ok(Vec::new());
    }
    bounded_by(discovery_deadline(used_for_mail), "carddav", async {
        mailcal_account::connect_carddav_learning(config, tokens)
            .await
            .map(|connected| {
                *learned.lock().expect("learned mutex poisoned") =
                    mailcal_account::Learned::connected(connected.principal);
                connected.providers
            })
    })
    .await
}

/// Binds a Google account's People contact adapters, one per source the app reads; the Workspace
/// directory only when `directory` says the person chose colleagues from their organisation.
///
/// There is nothing to discover first, unlike the two helpers either side of this one: the
/// source set is fixed and the token is already in hand, so this connects unconditionally and
/// lets a source the account does not have (the two Workspace-only ones, on a personal
/// account) report itself unavailable on its first sync. The deadline still applies, because
/// the token refresh it begins with is a network call on the mailbox's path.
pub(crate) async fn connect_google_contacts(
    tokens: Arc<GraphTokenSource>,
    directory: bool,
) -> Bound {
    bounded(
        "google",
        mailcal_account::connect_google_contact_providers(tokens, directory),
    )
    .await
}

/// Binds a Microsoft account's Graph contact adapters: one per personal contacts folder, then
/// the tenant directory when `directory` says the person chose colleagues from their
/// organisation.
///
/// The folder listing is the one network call before binding, and it is what an account whose
/// grant lacks the contact scopes fails on, so that account connects with mail and calendar and
/// an empty Contacts list until it signs in again.
pub(crate) async fn connect_graph_contacts(
    id: &AccountId,
    tokens: Arc<GraphTokenSource>,
    directory: bool,
) -> Bound {
    bounded(
        "graph",
        mailcal_account::connect_graph_contact_providers(id, tokens, directory),
    )
    .await
}

/// Binds the JMAP contacts adapter when the account's session advertises contact support.
///
/// `providers` is the account's already-connected mail provider set: its session capabilities
/// are what says whether this server has contacts at all, so checking them here avoids a
/// second connect to a server that would only refuse.
pub(crate) async fn connect_jmap_contacts(
    config: &JmapAccountConfig,
    tokens: Option<&Arc<GraphTokenSource>>,
    providers: &[Box<dyn Provider>],
) -> Bound {
    if !providers
        .first()
        .is_some_and(|provider| provider.connection_info().capabilities.contacts())
    {
        log::info!("jmap: contacts skipped; session advertises no contacts capability");
        return Ok(Vec::new());
    }
    bounded(
        "jmap",
        mailcal_account::connect_jmap_contact_providers(config, tokens),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::{DISCOVERY_DEADLINE, NO_MAILBOX_DEADLINE, discovery_deadline};

    #[test]
    fn an_account_without_mail_gives_a_slow_contacts_server_longer() {
        assert_eq!(discovery_deadline(true), DISCOVERY_DEADLINE);
        assert_eq!(discovery_deadline(false), NO_MAILBOX_DEADLINE);
        assert!(NO_MAILBOX_DEADLINE > DISCOVERY_DEADLINE);
    }
}
