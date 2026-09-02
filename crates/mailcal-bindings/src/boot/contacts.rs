//! Binding an account's contact-source adapters at boot, add, and reconnect.
//!
//! Both helpers follow the same rule the calendar ones do: **a contacts failure is never fatal
//! to the account.** Mail is why the user opened the app; an unreachable address book costs
//! them an empty Contacts list, not their inbox. So every path here logs and yields an empty
//! vector rather than propagating.
//!
//! Only CardDAV and JMAP are wired. Microsoft Graph and Google People both need OAuth scopes
//! this build does not request; adding them would force a re-consent prompt on every already
//! connected account: so those accounts sync no contacts yet (`docs/contacts.md`, Known gaps).
//!
//! # Why both helpers carry a deadline
//!
//! Discovery runs on the path that produces the user's **mailbox**, so its worst case is the
//! mailbox's worst case. Without a bound, a CalDAV host that accepts the connection and then
//! blackholes the address-book `PROPFIND` holds up mail: for a feature the account may not
//! even serve. The failure is already non-fatal; the deadline is what makes the *latency*
//! non-fatal too.
//!
//! # Why every exit logs, including the boring ones
//!
//! Because "Contacts is empty" has five causes here and only one of them is an error: the
//! account has no CalDAV endpoint to derive contacts from, the JMAP session does not advertise
//! contacts, discovery failed, discovery timed out, or it succeeded and the account genuinely
//! has no address book. Four of those used to return an empty vector in silence, so the log a
//! user attached to a support report said nothing at all about the surface they were reporting
//! on. Each exit now names itself, and the success path says how many sources it bound; a
//! count that is the difference between "we found nothing" and "we found books that are empty".

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use engine_api::{ContactsProvider, Provider};
use mailcal_account::{AccountConfig, GraphTokenSource, JmapAccountConfig};

/// How long contact-source discovery may hold up an account's connect before it is abandoned.
///
/// Generous enough for a real `PROPFIND` over a slow mobile link, short enough that a
/// blackholed host costs the user a pause rather than a launch.
const DISCOVERY_DEADLINE: Duration = Duration::from_secs(10);

/// Binds the CardDAV contact adapters for an IMAP/CalDAV account; one per discovered address
/// book, or none when the account has no `[caldav]` section to derive the endpoint from.
///
/// Contacts reuse the CalDAV origin and credentials (see `mailcal_account::contacts`), so an
/// account set up before this feature existed gains contacts with no re-entry of anything.
pub(crate) async fn connect_caldav_contacts(
    config: &AccountConfig,
    tokens: mailcal_account::ImapTokens<'_>,
) -> Vec<Box<dyn ContactsProvider>> {
    if config.caldav.is_none() {
        log::info!("carddav: contacts skipped; account has no caldav endpoint to derive one from");
        return Vec::new();
    }
    let started = Instant::now();
    match tokio::time::timeout(
        DISCOVERY_DEADLINE,
        mailcal_account::connect_carddav_contact_providers(config, tokens),
    )
    .await
    {
        Ok(Ok(providers)) => {
            log::info!(
                "carddav: bound {} contact source(s) in {}ms",
                providers.len(),
                started.elapsed().as_millis(),
            );
            providers
        }
        Ok(Err(err)) => {
            log::warn!(
                "carddav: contacts connect failed after {}ms, mail only: {err}",
                started.elapsed().as_millis(),
            );
            Vec::new()
        }
        Err(_) => {
            log::warn!(
                "carddav: contacts discovery timed out after {}s, mail only",
                DISCOVERY_DEADLINE.as_secs(),
            );
            Vec::new()
        }
    }
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
) -> Vec<Box<dyn ContactsProvider>> {
    if !providers
        .first()
        .is_some_and(|provider| provider.connection_info().capabilities.contacts())
    {
        log::info!("jmap: contacts skipped; session advertises no contacts capability");
        return Vec::new();
    }
    let started = Instant::now();
    match tokio::time::timeout(
        DISCOVERY_DEADLINE,
        mailcal_account::connect_jmap_contact_providers(config, tokens),
    )
    .await
    {
        Ok(Ok(providers)) => {
            log::info!(
                "jmap: bound {} contact source(s) in {}ms",
                providers.len(),
                started.elapsed().as_millis(),
            );
            providers
        }
        Ok(Err(err)) => {
            log::warn!(
                "jmap: contacts connect failed after {}ms, mail only: {err}",
                started.elapsed().as_millis(),
            );
            Vec::new()
        }
        Err(_) => {
            log::warn!(
                "jmap: contacts discovery timed out after {}s, mail only",
                DISCOVERY_DEADLINE.as_secs(),
            );
            Vec::new()
        }
    }
}
